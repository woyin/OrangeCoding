//! # 自定义工具系统
//!
//! 支持从 JSON 定义文件加载自定义工具，包括 Shell 脚本、可执行文件和 MCP 服务器工具。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 自定义工具定义
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomToolDef {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 参数 JSON Schema
    pub parameters: serde_json::Value,
    /// 处理器类型
    pub handler: ToolHandlerType,
    /// 定义文件来源路径
    pub source: PathBuf,
}

/// 工具处理器类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToolHandlerType {
    /// Shell 脚本
    Shell(String),
    /// 可执行文件
    Executable(PathBuf),
    /// MCP 服务器工具
    Mcp {
        /// MCP 服务器名称
        server: String,
        /// 工具名称
        tool: String,
    },
}

/// 自定义工具注册表
pub struct CustomToolRegistry {
    tools: Vec<CustomToolDef>,
}

impl CustomToolRegistry {
    /// 创建空的工具注册表
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// 注册自定义工具（同名则覆盖）
    pub fn register(&mut self, tool: CustomToolDef) {
        self.tools.retain(|t| t.name != tool.name);
        self.tools.push(tool);
    }

    /// 按名称获取工具定义
    pub fn get(&self, name: &str) -> Option<&CustomToolDef> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// 列出所有已注册工具
    pub fn list(&self) -> Vec<&CustomToolDef> {
        self.tools.iter().collect()
    }

    /// 从目录发现工具定义
    ///
    /// 读取 `<dir>/*.json` 文件，每个文件包含一个 `CustomToolDef` 的 JSON 定义。
    pub fn discover_from_dir(&mut self, dir: &Path) -> Result<usize, std::io::Error> {
        let mut count = 0;
        if !dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(mut tool) = serde_json::from_str::<CustomToolDef>(&content) {
                    tool.source = path;
                    self.register(tool);
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// 返回已注册工具数量
    pub fn count(&self) -> usize {
        self.tools.len()
    }
}

impl Default for CustomToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 辅助函数：构造自定义工具
    fn make_tool(name: &str) -> CustomToolDef {
        CustomToolDef {
            name: name.to_string(),
            description: format!("{name} tool"),
            parameters: serde_json::json!({"type": "object"}),
            handler: ToolHandlerType::Shell(format!("echo {name}")),
            source: PathBuf::from("test"),
        }
    }

    #[test]
    fn test_register_tool() {
        let mut reg = CustomToolRegistry::new();
        reg.register(make_tool("t1"));
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn test_get_tool() {
        let mut reg = CustomToolRegistry::new();
        reg.register(make_tool("alpha"));
        assert!(reg.get("alpha").is_some());
        assert_eq!(reg.get("alpha").unwrap().description, "alpha tool");
        assert!(reg.get("beta").is_none());
    }

    #[test]
    fn test_list_tools() {
        let mut reg = CustomToolRegistry::new();
        reg.register(make_tool("a"));
        reg.register(make_tool("b"));
        assert_eq!(reg.list().len(), 2);
    }

    #[test]
    fn test_discover_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let tool_json = serde_json::json!({
            "name": "my-tool",
            "description": "A custom tool",
            "parameters": {"type": "object"},
            "handler": {"Shell": "echo hello"},
            "source": "placeholder"
        });
        fs::write(
            dir.path().join("my-tool.json"),
            serde_json::to_string_pretty(&tool_json).unwrap(),
        )
        .unwrap();

        let mut reg = CustomToolRegistry::new();
        let count = reg.discover_from_dir(dir.path()).unwrap();
        assert_eq!(count, 1);
        assert!(reg.get("my-tool").is_some());
        assert_eq!(reg.get("my-tool").unwrap().description, "A custom tool");
    }

    #[test]
    fn test_handler_types() {
        // Shell 类型
        let shell_tool = make_tool("shell");
        assert!(matches!(&shell_tool.handler, ToolHandlerType::Shell(_)));

        // Executable 类型
        let exec_tool = CustomToolDef {
            name: "exec".to_string(),
            description: "exec tool".to_string(),
            parameters: serde_json::json!({}),
            handler: ToolHandlerType::Executable(PathBuf::from("/usr/bin/test")),
            source: PathBuf::from("test"),
        };
        assert!(matches!(&exec_tool.handler, ToolHandlerType::Executable(_)));

        // MCP 类型
        let mcp_tool = CustomToolDef {
            name: "mcp".to_string(),
            description: "mcp tool".to_string(),
            parameters: serde_json::json!({}),
            handler: ToolHandlerType::Mcp {
                server: "srv".to_string(),
                tool: "t".to_string(),
            },
            source: PathBuf::from("test"),
        };
        assert!(matches!(&mcp_tool.handler, ToolHandlerType::Mcp { .. }));
    }

    #[test]
    fn test_count() {
        let mut reg = CustomToolRegistry::new();
        assert_eq!(reg.count(), 0);
        reg.register(make_tool("a"));
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn test_empty() {
        let reg = CustomToolRegistry::new();
        assert_eq!(reg.count(), 0);
        assert!(reg.list().is_empty());
        assert!(reg.get("x").is_none());
    }
}
