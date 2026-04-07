//! # Grep 工具
//!
//! 在文件内容中搜索正则表达式，支持目录递归搜索、
//! glob 模式过滤、大小写敏感控制和上下文行显示。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;
use tracing::debug;
use walkdir::WalkDir;

/// Grep 工具 — 在文件内容中搜索正则表达式
#[derive(Debug)]
pub struct GrepTool;

impl GrepTool {
    /// 创建新的 Grep 工具实例
    pub fn new() -> Self {
        Self
    }

    /// 检查文件是否为二进制文件（前 8KB 中是否包含 null 字节）
    fn is_binary(content: &[u8]) -> bool {
        let check_len = content.len().min(8192);
        content[..check_len].contains(&0)
    }

    /// 检查路径是否匹配 glob 模式
    fn matches_glob(path: &str, pattern: &str) -> bool {
        let glob_pattern = glob::Pattern::new(pattern);
        match glob_pattern {
            Ok(p) => p.matches(path) || p.matches(Path::new(path).file_name().unwrap_or_default().to_str().unwrap_or("")),
            Err(_) => false,
        }
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "在文件内容中搜索正则表达式，返回匹配的行及其位置"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "正则表达式搜索模式"
                },
                "path": {
                    "type": "string",
                    "description": "搜索的文件或目录路径（默认当前目录）",
                    "default": "."
                },
                "include": {
                    "type": "string",
                    "description": "glob 模式过滤文件（例如 \"*.rs\"）"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "是否忽略大小写（默认 false）",
                    "default": false
                },
                "context_lines": {
                    "type": "number",
                    "description": "匹配行前后显示的上下文行数（默认 0）",
                    "default": 0,
                    "minimum": 0
                },
                "max_results": {
                    "type": "number",
                    "description": "最大返回结果数（默认 100）",
                    "default": 100,
                    "minimum": 1
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取搜索模式
        let pattern_str = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: pattern".to_string()))?;

        if pattern_str.is_empty() {
            return Err(ToolError::InvalidParams("pattern 参数不能为空".to_string()));
        }

        // 解析参数
        let search_path = params
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let include_pattern = params
            .get("include")
            .and_then(|v| v.as_str());

        let case_insensitive = params
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let context_lines = params
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        // 编译正则表达式
        let regex_pattern = if case_insensitive {
            format!("(?i){}", pattern_str)
        } else {
            pattern_str.to_string()
        };

        let regex = Regex::new(&regex_pattern).map_err(|e| {
            ToolError::InvalidParams(format!("无效的正则表达式: {}", e))
        })?;

        let path = Path::new(search_path);
        if !path.exists() {
            return Err(ToolError::ExecutionError(format!(
                "路径不存在: {}",
                search_path
            )));
        }

        debug!("Grep 搜索: pattern={}, path={}", pattern_str, search_path);

        let mut results = Vec::new();
        let mut total_matches = 0;

        // 收集要搜索的文件列表
        let files: Vec<_> = if path.is_file() {
            vec![path.to_path_buf()]
        } else {
            WalkDir::new(path)
                .into_iter()
                .filter_entry(|e| {
                    // 跳过隐藏目录
                    let name = e.file_name().to_str().unwrap_or("");
                    !name.starts_with('.') || e.depth() == 0
                })
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.into_path())
                .collect()
        };

        'outer: for file_path in &files {
            // 应用 include 过滤
            if let Some(pattern) = include_pattern {
                let file_name = file_path.to_str().unwrap_or("");
                if !Self::matches_glob(file_name, pattern) {
                    continue;
                }
            }

            // 读取文件内容
            let content_bytes = match std::fs::read(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // 跳过二进制文件
            if Self::is_binary(&content_bytes) {
                continue;
            }

            let content = match String::from_utf8(content_bytes) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();

            // 查找匹配行
            let mut match_indices: Vec<usize> = Vec::new();
            for (idx, line) in lines.iter().enumerate() {
                if regex.is_match(line) {
                    match_indices.push(idx);
                }
            }

            // 生成带上下文的输出
            let display_path = file_path.to_str().unwrap_or("?");

            for &match_idx in &match_indices {
                if total_matches >= max_results {
                    break 'outer;
                }

                let start = match_idx.saturating_sub(context_lines);
                let end = (match_idx + context_lines + 1).min(lines.len());

                // 如果有上下文行且不是第一个结果，添加分隔符
                if context_lines > 0 && !results.is_empty() {
                    results.push("--".to_string());
                }

                for i in start..end {
                    let line_num = i + 1;
                    results.push(format!("{}:{}:{}", display_path, line_num, lines[i]));
                }

                total_matches += 1;
            }
        }

        if results.is_empty() {
            Ok("未找到匹配结果".to_string())
        } else {
            let header = if total_matches >= max_results {
                format!("找到 {} 个匹配（已达上限）：\n", total_matches)
            } else {
                format!("找到 {} 个匹配：\n", total_matches)
            };
            Ok(format!("{}{}", header, results.join("\n")))
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
    use std::fs;

    /// 辅助函数：创建测试目录结构
    fn setup_test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();

        // 创建测试文件
        fs::write(dir.path().join("hello.rs"), "fn main() {\n    println!(\"Hello, world!\");\n}\n").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n").unwrap();
        fs::write(dir.path().join("readme.txt"), "This is a README file.\nIt contains some text.\n").unwrap();

        // 创建子目录和文件
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.rs"), "fn nested() {\n    // nested function\n}\n").unwrap();

        dir
    }

    /// 测试搜索字面量字符串
    #[tokio::test]
    async fn test_search_literal_string() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let params = json!({
            "pattern": "Hello",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("Hello"));
        assert!(result.contains("hello.rs"));
    }

    /// 测试正则表达式搜索
    #[tokio::test]
    async fn test_regex_pattern_search() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let params = json!({
            "pattern": r"fn \w+\(",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("fn main("));
        assert!(result.contains("fn add("));
    }

    /// 测试大小写不敏感搜索
    #[tokio::test]
    async fn test_case_insensitive_search() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let params = json!({
            "pattern": "hello",
            "path": dir.path().to_str().unwrap(),
            "case_insensitive": true
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("Hello"));
    }

    /// 测试上下文行显示
    #[tokio::test]
    async fn test_context_lines() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let params = json!({
            "pattern": "println",
            "path": dir.path().to_str().unwrap(),
            "context_lines": 1
        });

        let result = tool.execute(params).await.unwrap();
        // 应包含 println 行及其前后各一行
        assert!(result.contains("fn main()"));
        assert!(result.contains("println"));
    }

    /// 测试 include 文件过滤
    #[tokio::test]
    async fn test_file_pattern_filtering() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let params = json!({
            "pattern": "fn",
            "path": dir.path().to_str().unwrap(),
            "include": "*.rs"
        });

        let result = tool.execute(params).await.unwrap();
        // 应只包含 .rs 文件的结果
        assert!(result.contains(".rs"));
        assert!(!result.contains("readme.txt"));
    }

    /// 测试最大结果限制
    #[tokio::test]
    async fn test_max_results_limiting() {
        let dir = setup_test_dir();

        // 创建包含多个匹配行的文件
        let many_lines: String = (0..50).map(|i| format!("match_line_{}\n", i)).collect();
        fs::write(dir.path().join("many.txt"), &many_lines).unwrap();

        let tool = GrepTool::new();
        let params = json!({
            "pattern": "match_line",
            "path": dir.path().to_str().unwrap(),
            "max_results": 5
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("已达上限"));
    }

    /// 测试无匹配结果
    #[tokio::test]
    async fn test_no_matches() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let params = json!({
            "pattern": "nonexistent_string_xyz",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("未找到"));
    }

    /// 测试无效正则表达式
    #[tokio::test]
    async fn test_invalid_regex() {
        let tool = GrepTool::new();
        let params = json!({
            "pattern": "[invalid",
            "path": "."
        });

        let result = tool.execute(params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("正则"));
            }
            other => panic!("期望参数错误，得到: {:?}", other),
        }
    }

    /// 测试二进制文件跳过
    #[tokio::test]
    async fn test_binary_file_skipping() {
        let dir = setup_test_dir();

        // 创建包含 null 字节的二进制文件
        let mut binary_content = b"match_this\x00binary data".to_vec();
        fs::write(dir.path().join("binary.bin"), &binary_content).unwrap();

        let tool = GrepTool::new();
        let params = json!({
            "pattern": "match_this",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        // 二进制文件不应出现在结果中
        assert!(!result.contains("binary.bin"));
    }

    /// 测试嵌套目录搜索
    #[tokio::test]
    async fn test_nested_directory_search() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let params = json!({
            "pattern": "nested",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("nested"));
        assert!(result.contains("sub"));
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameters_schema() {
        let tool = GrepTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["include"].is_object());
        assert!(schema["properties"]["case_insensitive"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("pattern")));
    }

    /// 测试工具名称和描述
    #[test]
    fn test_name_and_description() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), "grep");
        assert!(!tool.description().is_empty());
    }

    /// 测试空 pattern
    #[tokio::test]
    async fn test_empty_pattern() {
        let tool = GrepTool::new();
        let params = json!({"pattern": ""});
        let result = tool.execute(params).await;
        assert!(result.is_err());
    }

    /// 测试路径不存在
    #[tokio::test]
    async fn test_nonexistent_path() {
        let tool = GrepTool::new();
        let params = json!({
            "pattern": "test",
            "path": "/nonexistent_dir_12345"
        });
        let result = tool.execute(params).await;
        assert!(result.is_err());
    }
}
