//! # 编辑工具
//!
//! 精确的文件内容替换工具，基于字符串匹配实现编辑操作。
//! 支持单次编辑、批量编辑、唯一性验证和差异预览。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;
use tracing::debug;

// ============================================================
// 数据类型定义
// ============================================================

/// 编辑操作 — 描述一次精确的文本替换
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditOperation {
    /// 文件路径
    pub path: String,
    /// 要替换的原始文本
    pub old_text: String,
    /// 替换后的新文本
    pub new_text: String,
}

// ============================================================
// EditTool — 精确文件内容替换工具
// ============================================================

/// 编辑工具 — 精确的文件内容替换
///
/// 与 `EditFileTool` 不同，`EditTool` 提供纯函数式的内容操作方法，
/// 支持批量编辑和差异预览功能。
#[derive(Debug)]
pub struct EditTool;

impl EditTool {
    /// 验证旧文本在内容中只出现一次
    ///
    /// 如果匹配次数不为 1，返回相应的错误信息。
    pub fn validate_unique(content: &str, old_text: &str) -> Result<(), ToolError> {
        let count = content.matches(old_text).count();
        if count == 0 {
            return Err(ToolError::ExecutionError(format!(
                "未找到要替换的文本: {:?}",
                old_text
            )));
        }
        if count > 1 {
            return Err(ToolError::ExecutionError(format!(
                "找到 {} 处匹配，要求恰好匹配一处: {:?}",
                count, old_text
            )));
        }
        Ok(())
    }

    /// 应用单个编辑操作
    ///
    /// 先验证唯一性，再执行替换。
    pub fn apply_edit(content: &str, edit: &EditOperation) -> Result<String, ToolError> {
        Self::validate_unique(content, &edit.old_text)?;
        Ok(content.replacen(&edit.old_text, &edit.new_text, 1))
    }

    /// 应用多个编辑操作（按序执行）
    ///
    /// 每个编辑操作基于前一个操作的结果执行。
    pub fn apply_edits(content: &str, edits: &[EditOperation]) -> Result<String, ToolError> {
        let mut result = content.to_string();
        for edit in edits {
            result = Self::apply_edit(&result, edit)?;
        }
        Ok(result)
    }

    /// 生成编辑差异预览
    ///
    /// 简单的逐行对比，标记增删行。
    pub fn diff_preview(old_content: &str, new_content: &str) -> String {
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        let mut output = Vec::new();
        let max_len = old_lines.len().max(new_lines.len());

        // 使用简单的逐行比较生成差异
        let mut i = 0;
        let mut j = 0;
        while i < old_lines.len() || j < new_lines.len() {
            if i < old_lines.len() && j < new_lines.len() {
                if old_lines[i] == new_lines[j] {
                    output.push(format!(" {}", old_lines[i]));
                    i += 1;
                    j += 1;
                } else {
                    // 查找旧行是否在新内容后续出现
                    let mut found_in_new = false;
                    for k in (j + 1)..new_lines.len().min(j + max_len) {
                        if old_lines[i] == new_lines[k] {
                            // 新增的行
                            for m in j..k {
                                output.push(format!("+{}", new_lines[m]));
                            }
                            j = k;
                            found_in_new = true;
                            break;
                        }
                    }
                    if !found_in_new {
                        output.push(format!("-{}", old_lines[i]));
                        i += 1;
                        // 检查新行是否是替换
                        if j < new_lines.len() {
                            output.push(format!("+{}", new_lines[j]));
                            j += 1;
                        }
                    }
                }
            } else if i < old_lines.len() {
                output.push(format!("-{}", old_lines[i]));
                i += 1;
            } else {
                output.push(format!("+{}", new_lines[j]));
                j += 1;
            }
        }

        output.join("\n")
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "精确的文件内容替换，基于字符串匹配进行编辑"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要编辑的文件路径"
                },
                "old_text": {
                    "type": "string",
                    "description": "要替换的原始文本（必须精确匹配且唯一）"
                },
                "new_text": {
                    "type": "string",
                    "description": "替换后的新文本"
                }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: path".to_string()))?;

        let old_text = params
            .get("old_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: old_text".to_string()))?;

        let new_text = params
            .get("new_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: new_text".to_string()))?;

        debug!("编辑文件: {}", path);

        // 检查文件存在
        if !Path::new(path).exists() {
            return Err(ToolError::ExecutionError(format!("文件不存在: {}", path)));
        }

        // 读取内容并应用编辑
        let content = fs::read_to_string(path).await?;
        let edit = EditOperation {
            path: path.to_string(),
            old_text: old_text.to_string(),
            new_text: new_text.to_string(),
        };

        let new_content = Self::apply_edit(&content, &edit)?;

        // 生成差异预览
        let preview = Self::diff_preview(&content, &new_content);

        // 写回文件
        fs::write(path, &new_content).await?;

        Ok(format!(
            "已成功编辑文件: {}\n\n差异预览:\n{}",
            path, preview
        ))
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试应用单个编辑操作
    #[test]
    fn test_apply_single_edit() {
        let content = "fn hello() {\n    println!(\"hello\");\n}\n";
        let edit = EditOperation {
            path: "test.rs".to_string(),
            old_text: "hello".to_string(),
            new_text: "world".to_string(),
        };

        // old_text 出现多次，应报错
        let result = EditTool::apply_edit(content, &edit);
        assert!(result.is_err());

        // 使用唯一匹配
        let edit2 = EditOperation {
            path: "test.rs".to_string(),
            old_text: "println!(\"hello\")".to_string(),
            new_text: "println!(\"world\")".to_string(),
        };
        let result = EditTool::apply_edit(content, &edit2).unwrap();
        assert!(result.contains("println!(\"world\")"));
        assert!(!result.contains("println!(\"hello\")"));
    }

    /// 测试应用多个编辑操作
    #[test]
    fn test_apply_multiple_edits() {
        let content = "let a = 1;\nlet b = 2;\nlet c = 3;\n";
        let edits = vec![
            EditOperation {
                path: "test.rs".to_string(),
                old_text: "let a = 1;".to_string(),
                new_text: "let a = 10;".to_string(),
            },
            EditOperation {
                path: "test.rs".to_string(),
                old_text: "let b = 2;".to_string(),
                new_text: "let b = 20;".to_string(),
            },
        ];

        let result = EditTool::apply_edits(content, &edits).unwrap();
        assert!(result.contains("let a = 10;"));
        assert!(result.contains("let b = 20;"));
        assert!(result.contains("let c = 3;"));
    }

    /// 测试验证唯一性 — 单次匹配通过
    #[test]
    fn test_validate_unique_single_match() {
        let content = "唯一的文本在这里";
        let result = EditTool::validate_unique(content, "唯一的文本");
        assert!(result.is_ok());
    }

    /// 测试验证唯一性 — 无匹配报错
    #[test]
    fn test_validate_unique_no_match() {
        let content = "这里没有要找的内容";
        let result = EditTool::validate_unique(content, "不存在的文本");
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionError(msg) => {
                assert!(msg.contains("未找到"));
            }
            other => panic!("期望执行错误，得到: {:?}", other),
        }
    }

    /// 测试验证唯一性 — 多次匹配报错
    #[test]
    fn test_validate_unique_multiple_matches() {
        let content = "重复 重复 重复";
        let result = EditTool::validate_unique(content, "重复");
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionError(msg) => {
                assert!(msg.contains("3"));
                assert!(msg.contains("匹配"));
            }
            other => panic!("期望执行错误，得到: {:?}", other),
        }
    }

    /// 测试差异预览生成
    #[test]
    fn test_diff_preview() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3";

        let preview = EditTool::diff_preview(old, new);
        assert!(preview.contains(" line1"));
        assert!(preview.contains("-line2"));
        assert!(preview.contains("+modified"));
        assert!(preview.contains(" line3"));
    }

    /// 测试多行编辑
    #[test]
    fn test_multiline_edit() {
        let content = "fn main() {\n    // 旧代码\n    let x = 1;\n}\n";
        let edit = EditOperation {
            path: "test.rs".to_string(),
            old_text: "    // 旧代码\n    let x = 1;".to_string(),
            new_text: "    // 新代码\n    let x = 42;\n    let y = 100;".to_string(),
        };

        let result = EditTool::apply_edit(content, &edit).unwrap();
        assert!(result.contains("// 新代码"));
        assert!(result.contains("let x = 42;"));
        assert!(result.contains("let y = 100;"));
    }

    /// 测试空新文本（删除操作）
    #[test]
    fn test_empty_new_text() {
        let content = "keep this\nremove this\nkeep this too\n";
        let edit = EditOperation {
            path: "test.rs".to_string(),
            old_text: "remove this\n".to_string(),
            new_text: "".to_string(),
        };

        let result = EditTool::apply_edit(content, &edit).unwrap();
        assert_eq!(result, "keep this\nkeep this too\n");
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameter_schema() {
        let tool = EditTool;
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["old_text"].is_object());
        assert!(schema["properties"]["new_text"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("path")));
        assert!(required.contains(&json!("old_text")));
        assert!(required.contains(&json!("new_text")));
    }

    /// 测试工具名称
    #[test]
    fn test_tool_name() {
        let tool = EditTool;
        assert_eq!(tool.name(), "edit");
        assert!(!tool.description().is_empty());
    }
}
