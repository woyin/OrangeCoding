//! # AST 工具
//!
//! 基于语法树的代码搜索和编辑工具，封装 ast-grep 命令行工具。
//! 支持多语言的结构化代码搜索和重构操作。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;

// ============================================================
// 数据类型定义
// ============================================================

/// AST 搜索请求
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AstSearchRequest {
    /// 搜索模式（ast-grep 模式语法）
    pub pattern: String,
    /// 目标语言
    pub language: String,
    /// 搜索路径
    pub path: String,
    /// 最大结果数量
    pub max_results: Option<usize>,
}

/// AST 匹配结果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AstMatch {
    /// 文件路径
    pub file: String,
    /// 行号
    pub line: usize,
    /// 列号
    pub column: usize,
    /// 匹配的文本
    pub matched_text: String,
    /// 上下文行
    pub context: String,
}

/// AST 编辑请求
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AstEditRequest {
    /// 搜索模式
    pub search_pattern: String,
    /// 替换模式
    pub replace_pattern: String,
    /// 目标语言
    pub language: String,
    /// 编辑路径
    pub path: String,
}

// ============================================================
// AstTool — AST 搜索/编辑工具
// ============================================================

/// AST 工具 — 基于语法树的代码搜索和编辑
#[derive(Debug)]
pub struct AstTool;

/// 支持的编程语言列表
const SUPPORTED_LANGUAGES: &[&str] = &[
    "rust",
    "python",
    "javascript",
    "typescript",
    "go",
    "java",
    "c",
    "cpp",
    "kotlin",
    "swift",
    "ruby",
    "html",
    "css",
    "json",
    "yaml",
    "toml",
];

impl AstTool {
    /// 构建 ast-grep 搜索命令
    pub fn build_search_command(req: &AstSearchRequest) -> Vec<String> {
        let mut args = vec![
            "sg".to_string(),
            "--pattern".to_string(),
            req.pattern.clone(),
            "--lang".to_string(),
            req.language.clone(),
            "--json".to_string(),
        ];

        if let Some(max) = req.max_results {
            args.push("--max-count".to_string());
            args.push(max.to_string());
        }

        args.push(req.path.clone());
        args
    }

    /// 构建 ast-grep 编辑命令
    pub fn build_edit_command(req: &AstEditRequest) -> Vec<String> {
        vec![
            "sg".to_string(),
            "--pattern".to_string(),
            req.search_pattern.clone(),
            "--rewrite".to_string(),
            req.replace_pattern.clone(),
            "--lang".to_string(),
            req.language.clone(),
            "--update-all".to_string(),
            req.path.clone(),
        ]
    }

    /// 解析 ast-grep JSON 输出
    pub fn parse_results(output: &str) -> Result<Vec<AstMatch>, String> {
        if output.trim().is_empty() {
            return Ok(Vec::new());
        }

        let raw: Value =
            serde_json::from_str(output).map_err(|e| format!("JSON 解析失败: {}", e))?;

        let arr = match raw {
            Value::Array(arr) => arr,
            // ast-grep 可能返回单个对象
            Value::Object(_) => vec![raw],
            _ => return Err("意外的 JSON 格式".to_string()),
        };

        let mut results = Vec::new();
        for item in &arr {
            let file = item
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let range = item.get("range").and_then(|v| v.get("start"));
            let line = range
                .and_then(|v| v.get("line"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let column = range
                .and_then(|v| v.get("column"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            let matched_text = item
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let context = item
                .get("lines")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            results.push(AstMatch {
                file,
                line,
                column,
                matched_text,
                context,
            });
        }

        Ok(results)
    }

    /// 检测 ast-grep 是否可用
    pub fn is_available() -> bool {
        std::process::Command::new("sg")
            .arg("--version")
            .output()
            .is_ok()
    }

    /// 返回支持的语言列表
    pub fn supported_languages() -> Vec<&'static str> {
        SUPPORTED_LANGUAGES.to_vec()
    }
}

#[async_trait]
impl Tool for AstTool {
    fn name(&self) -> &str {
        "ast_grep"
    }

    fn description(&self) -> &str {
        "基于语法树的代码搜索和编辑，使用 ast-grep 进行结构化匹配"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "操作类型: search, edit",
                    "enum": ["search", "edit"]
                },
                "pattern": {
                    "type": "string",
                    "description": "搜索模式（ast-grep 模式语法）"
                },
                "replace": {
                    "type": "string",
                    "description": "替换模式（编辑模式下使用）"
                },
                "language": {
                    "type": "string",
                    "description": "目标编程语言（如 rust, python, javascript）"
                },
                "path": {
                    "type": "string",
                    "description": "搜索/编辑路径"
                },
                "max_results": {
                    "type": "number",
                    "description": "最大结果数量（可选）"
                }
            },
            "required": ["action", "pattern", "language", "path"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: action".to_string()))?;

        let pattern = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: pattern".to_string()))?;

        let language = params
            .get("language")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: language".to_string()))?;

        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: path".to_string()))?;

        debug!("AST {} 操作: 模式={}, 语言={}", action, pattern, language);

        match action {
            "search" => {
                let max_results = params
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);

                let req = AstSearchRequest {
                    pattern: pattern.to_string(),
                    language: language.to_string(),
                    path: path.to_string(),
                    max_results,
                };

                let args = Self::build_search_command(&req);

                let mut cmd = tokio::process::Command::new(&args[0]);
                for arg in &args[1..] {
                    cmd.arg(arg);
                }
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());

                match cmd.output().await {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        if stdout.trim().is_empty() {
                            return Ok("未找到匹配结果".to_string());
                        }

                        match Self::parse_results(&stdout) {
                            Ok(matches) => {
                                let mut result = format!("找到 {} 个匹配:\n", matches.len());
                                for m in &matches {
                                    result.push_str(&format!(
                                        "  {}:{}:{} - {}\n",
                                        m.file, m.line, m.column, m.matched_text
                                    ));
                                }
                                Ok(result)
                            }
                            Err(_e) => {
                                // 非 JSON 输出，直接返回原始结果
                                Ok(stdout)
                            }
                        }
                    }
                    Err(e) => Err(ToolError::ExecutionError(format!(
                        "启动 ast-grep 失败: {}",
                        e
                    ))),
                }
            }
            "edit" => {
                let replace = params
                    .get("replace")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidParams("edit 操作需要 replace 参数".to_string())
                    })?;

                let req = AstEditRequest {
                    search_pattern: pattern.to_string(),
                    replace_pattern: replace.to_string(),
                    language: language.to_string(),
                    path: path.to_string(),
                };

                let args = Self::build_edit_command(&req);

                let mut cmd = tokio::process::Command::new(&args[0]);
                for arg in &args[1..] {
                    cmd.arg(arg);
                }
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());

                match cmd.output().await {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                        if output.status.success() {
                            Ok(format!("AST 编辑完成\n{}", stdout))
                        } else {
                            Err(ToolError::ExecutionError(format!(
                                "AST 编辑失败: {}",
                                stderr
                            )))
                        }
                    }
                    Err(e) => Err(ToolError::ExecutionError(format!(
                        "启动 ast-grep 失败: {}",
                        e
                    ))),
                }
            }
            other => Err(ToolError::InvalidParams(format!("未知操作: {}", other))),
        }
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试构建搜索命令
    #[test]
    fn test_build_search_command() {
        let req = AstSearchRequest {
            pattern: "fn $NAME($$$ARGS)".to_string(),
            language: "rust".to_string(),
            path: "src/".to_string(),
            max_results: Some(10),
        };

        let args = AstTool::build_search_command(&req);

        assert_eq!(args[0], "sg");
        assert!(args.contains(&"--pattern".to_string()));
        assert!(args.contains(&"fn $NAME($$$ARGS)".to_string()));
        assert!(args.contains(&"--lang".to_string()));
        assert!(args.contains(&"rust".to_string()));
        assert!(args.contains(&"--json".to_string()));
        assert!(args.contains(&"--max-count".to_string()));
        assert!(args.contains(&"10".to_string()));
        assert!(args.contains(&"src/".to_string()));
    }

    /// 测试构建编辑命令
    #[test]
    fn test_build_edit_command() {
        let req = AstEditRequest {
            search_pattern: "println!($$$ARGS)".to_string(),
            replace_pattern: "tracing::info!($$$ARGS)".to_string(),
            language: "rust".to_string(),
            path: "src/".to_string(),
        };

        let args = AstTool::build_edit_command(&req);

        assert_eq!(args[0], "sg");
        assert!(args.contains(&"--pattern".to_string()));
        assert!(args.contains(&"println!($$$ARGS)".to_string()));
        assert!(args.contains(&"--rewrite".to_string()));
        assert!(args.contains(&"tracing::info!($$$ARGS)".to_string()));
        assert!(args.contains(&"--lang".to_string()));
        assert!(args.contains(&"rust".to_string()));
        assert!(args.contains(&"--update-all".to_string()));
    }

    /// 测试解析 ast-grep JSON 输出
    #[test]
    fn test_parse_results() {
        let json_output = json!([
            {
                "file": "src/main.rs",
                "range": {
                    "start": { "line": 10, "column": 4 },
                    "end": { "line": 10, "column": 20 }
                },
                "text": "fn main()",
                "lines": "fn main() {"
            },
            {
                "file": "src/lib.rs",
                "range": {
                    "start": { "line": 5, "column": 0 },
                    "end": { "line": 5, "column": 15 }
                },
                "text": "fn helper()",
                "lines": "fn helper() {"
            }
        ]);

        let results = AstTool::parse_results(&json_output.to_string()).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].file, "src/main.rs");
        assert_eq!(results[0].line, 10);
        assert_eq!(results[0].column, 4);
        assert_eq!(results[0].matched_text, "fn main()");
        assert_eq!(results[1].file, "src/lib.rs");
        assert_eq!(results[1].line, 5);
    }

    /// 测试空输出解析
    #[test]
    fn test_parse_results_empty() {
        let results = AstTool::parse_results("").unwrap();
        assert!(results.is_empty());
    }

    /// 测试支持的语言列表
    #[test]
    fn test_supported_languages() {
        let langs = AstTool::supported_languages();

        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"python"));
        assert!(langs.contains(&"javascript"));
        assert!(langs.contains(&"typescript"));
        assert!(langs.contains(&"go"));
        assert!(langs.contains(&"java"));
        // 至少支持 10 种语言
        assert!(langs.len() >= 10);
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameter_schema() {
        let tool = AstTool;
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["action"].is_object());
        assert!(schema["properties"]["pattern"].is_object());
        assert!(schema["properties"]["language"].is_object());
        assert!(schema["properties"]["path"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("action")));
        assert!(required.contains(&json!("pattern")));
        assert!(required.contains(&json!("language")));
        assert!(required.contains(&json!("path")));
    }

    /// 测试工具名称和描述
    #[test]
    fn test_tool_name() {
        let tool = AstTool;
        assert_eq!(tool.name(), "ast_grep");
        assert!(!tool.description().is_empty());
    }
}
