//! # Find 工具
//!
//! 按名称模式查找文件和目录。
//! 支持 glob 模式、类型过滤、深度限制等功能。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tracing::debug;
use walkdir::WalkDir;

/// Find 工具 — 按名称模式查找文件
#[derive(Debug)]
pub struct FindTool;

impl FindTool {
    /// 创建新的 Find 工具实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for FindTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FindTool {
    fn name(&self) -> &str {
        "find"
    }

    fn description(&self) -> &str {
        "按名称模式查找文件和目录"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "glob 模式（例如 \"*.rs\"、\"**/*.toml\"）"
                },
                "path": {
                    "type": "string",
                    "description": "起始搜索目录（默认当前目录）",
                    "default": "."
                },
                "type": {
                    "type": "string",
                    "description": "过滤类型：file、directory 或 any（默认 any）",
                    "enum": ["file", "directory", "any"],
                    "default": "any"
                },
                "max_depth": {
                    "type": "number",
                    "description": "最大搜索深度（可选）",
                    "minimum": 0
                },
                "max_results": {
                    "type": "number",
                    "description": "最大返回结果数（默认 200）",
                    "default": 200,
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
        let search_path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let type_filter = params.get("type").and_then(|v| v.as_str()).unwrap_or("any");

        let max_depth = params
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(200) as usize;

        // 验证搜索路径
        let path = Path::new(search_path);
        if !path.exists() {
            return Err(ToolError::ExecutionError(format!(
                "路径不存在: {}",
                search_path
            )));
        }

        // 编译 glob 模式
        let glob_pattern = glob::Pattern::new(pattern_str)
            .map_err(|e| ToolError::InvalidParams(format!("无效的 glob 模式: {}", e)))?;

        // 判断模式是否搜索隐藏文件
        let search_hidden = pattern_str.starts_with('.');

        debug!(
            "Find 搜索: pattern={}, path={}, type={}",
            pattern_str, search_path, type_filter
        );

        // 构建目录遍历器
        let mut walker = WalkDir::new(path);
        if let Some(depth) = max_depth {
            walker = walker.max_depth(depth);
        }

        let mut matches: Vec<String> = Vec::new();

        for entry in walker.into_iter().filter_entry(|e| {
            // 跳过隐藏目录（除非搜索隐藏文件）
            if !search_hidden && e.depth() > 0 {
                let name = e.file_name().to_str().unwrap_or("");
                if name.starts_with('.') {
                    return false;
                }
            }
            true
        }) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            // 跳过根目录自身
            if entry.depth() == 0 {
                continue;
            }

            // 类型过滤
            let is_file = entry.file_type().is_file();
            let is_dir = entry.file_type().is_dir();

            match type_filter {
                "file" if !is_file => continue,
                "directory" if !is_dir => continue,
                _ => {}
            }

            // 匹配文件名（对于非递归模式）或完整路径
            let file_name = entry.file_name().to_str().unwrap_or("");
            let relative_path = entry
                .path()
                .strip_prefix(path)
                .unwrap_or(entry.path())
                .to_str()
                .unwrap_or("");

            let matched = glob_pattern.matches(file_name) || glob_pattern.matches(relative_path);

            if matched {
                matches.push(relative_path.to_string());
                if matches.len() >= max_results {
                    break;
                }
            }
        }

        // 按字母排序
        matches.sort();

        if matches.is_empty() {
            Ok("未找到匹配的文件".to_string())
        } else {
            let count = matches.len();
            let truncated = if count >= max_results {
                format!("（已达上限 {}）", max_results)
            } else {
                String::new()
            };
            Ok(format!(
                "找到 {} 个匹配{}：\n{}",
                count,
                truncated,
                matches.join("\n")
            ))
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

        // 创建文件
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub mod lib;").unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        fs::write(dir.path().join("README.md"), "# README").unwrap();

        // 创建子目录
        let sub = dir.path().join("src");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("utils.rs"), "pub fn util() {}").unwrap();
        fs::write(sub.join("config.toml"), "key = \"value\"").unwrap();

        // 创建深层子目录
        let deep = sub.join("deep");
        fs::create_dir(&deep).unwrap();
        fs::write(deep.join("inner.rs"), "fn inner() {}").unwrap();

        // 创建普通目录（不是隐藏的）
        let data = dir.path().join("data");
        fs::create_dir(&data).unwrap();
        fs::write(data.join("test.txt"), "test data").unwrap();

        // 创建隐藏目录
        let hidden = dir.path().join(".hidden");
        fs::create_dir(&hidden).unwrap();
        fs::write(hidden.join("secret.rs"), "secret").unwrap();

        dir
    }

    /// 测试按扩展名查找文件
    #[tokio::test]
    async fn test_find_files_by_extension() {
        let dir = setup_test_dir();
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.rs",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("main.rs"));
        assert!(result.contains("lib.rs"));
        assert!(!result.contains("Cargo.toml"));
    }

    /// 测试仅查找目录
    #[tokio::test]
    async fn test_find_directories_only() {
        let dir = setup_test_dir();
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*",
            "path": dir.path().to_str().unwrap(),
            "type": "directory"
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("src"));
        assert!(result.contains("data"));
        // 不应包含文件
        assert!(!result.contains("main.rs"));
    }

    /// 测试递归 glob 模式
    #[tokio::test]
    async fn test_recursive_glob() {
        let dir = setup_test_dir();
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.rs",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        // 应找到所有层级的 .rs 文件
        assert!(result.contains("main.rs"));
        assert!(result.contains("utils.rs"));
        assert!(result.contains("inner.rs"));
    }

    /// 测试最大深度限制
    #[tokio::test]
    async fn test_max_depth_limiting() {
        let dir = setup_test_dir();
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.rs",
            "path": dir.path().to_str().unwrap(),
            "max_depth": 1
        });

        let result = tool.execute(params).await.unwrap();
        // depth=1 应只搜索直接子文件
        assert!(result.contains("main.rs"));
        // 子目录中的文件不应出现
        assert!(!result.contains("utils.rs"));
        assert!(!result.contains("inner.rs"));
    }

    /// 测试最大结果限制
    #[tokio::test]
    async fn test_max_results_limiting() {
        let dir = setup_test_dir();

        // 创建大量文件
        for i in 0..20 {
            fs::write(dir.path().join(format!("file_{}.txt", i)), "content").unwrap();
        }

        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.txt",
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
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.xyz",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("未找到"));
    }

    /// 测试在指定目录中查找
    #[tokio::test]
    async fn test_find_in_specific_directory() {
        let dir = setup_test_dir();
        let src_path = dir.path().join("src");
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.rs",
            "path": src_path.to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("utils.rs"));
        // 根目录的文件不应出现
        assert!(!result.contains("main.rs"));
    }

    /// 测试隐藏目录默认不搜索
    #[tokio::test]
    async fn test_hidden_directory_skipped() {
        let dir = setup_test_dir();
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.rs",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        // 隐藏目录中的文件不应出现
        assert!(!result.contains("secret.rs"));
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameters_schema() {
        let tool = FindTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["type"].is_object());
        assert!(schema["properties"]["max_depth"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("pattern")));
    }

    /// 测试工具名称和描述
    #[test]
    fn test_name_and_description() {
        let tool = FindTool::new();
        assert_eq!(tool.name(), "find");
        assert!(!tool.description().is_empty());
    }

    /// 测试空 pattern
    #[tokio::test]
    async fn test_empty_pattern() {
        let tool = FindTool::new();
        let params = json!({"pattern": ""});
        let result = tool.execute(params).await;
        assert!(result.is_err());
    }

    /// 测试路径不存在
    #[tokio::test]
    async fn test_nonexistent_path() {
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.rs",
            "path": "/nonexistent_dir_12345"
        });
        let result = tool.execute(params).await;
        assert!(result.is_err());
    }

    /// 测试结果按字母排序
    #[tokio::test]
    async fn test_results_sorted() {
        let dir = setup_test_dir();
        let tool = FindTool::new();
        let params = json!({
            "pattern": "*.rs",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(params).await.unwrap();
        let lines: Vec<&str> = result.lines().skip(1).collect(); // 跳过标题行
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted);
    }
}
