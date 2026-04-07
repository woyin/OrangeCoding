//! # 工具注册表管理
//!
//! 提供线程安全的工具注册表 `ToolRegistry`，用于动态注册、查找和管理工具。
//! 使用 `DashMap` 实现高性能的并发读写访问。

use crate::bash_tool::BashTool;
use crate::file_tools::{
    DeleteFileTool, EditFileTool, ListDirectoryTool, ReadFileTool, SearchFilesTool, WriteFileTool,
};
use crate::find_tool::FindTool;
use crate::grep_tool::GrepTool;
use crate::{Tool, ToolError, ToolResult};
use dashmap::DashMap;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn};

// ============================================================
// 工具注册表
// ============================================================

/// 工具注册表
///
/// 线程安全的工具管理容器，支持并发注册和查找。
/// 内部使用 `DashMap` 存储工具实例，键为工具名称，值为 `Arc<dyn Tool>`。
///
/// # 使用示例
/// ```no_run
/// use ceair_tools::{ToolRegistry, ReadFileTool};
/// use std::sync::Arc;
///
/// let registry = ToolRegistry::new();
/// registry.register(Arc::new(ReadFileTool));
/// ```
#[derive(Debug)]
pub struct ToolRegistry {
    /// 工具存储映射表（名称 -> 工具实例）
    tools: DashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// 创建一个空的工具注册表
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
        }
    }

    /// 注册一个工具到注册表
    ///
    /// 如果已存在同名工具，会被新工具替换，并打印警告日志。
    ///
    /// # 参数
    /// - `tool`: 实现了 `Tool` 特征的工具实例（包裹在 Arc 中以支持共享所有权）
    pub fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            warn!("工具 '{}' 已存在，将被覆盖注册", name);
        }
        debug!("注册工具: {}", name);
        self.tools.insert(name, tool);
    }

    /// 从注册表中移除指定名称的工具
    ///
    /// # 参数
    /// - `name`: 要移除的工具名称
    ///
    /// # 返回值
    /// 如果工具存在并被成功移除则返回 `true`，否则返回 `false`
    pub fn unregister(&self, name: &str) -> bool {
        let removed = self.tools.remove(name).is_some();
        if removed {
            debug!("已移除工具: {}", name);
        } else {
            warn!("尝试移除不存在的工具: {}", name);
        }
        removed
    }

    /// 根据名称获取工具实例
    ///
    /// # 参数
    /// - `name`: 工具名称
    ///
    /// # 返回值
    /// 如果工具存在则返回其 `Arc` 引用，否则返回 `None`
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).map(|entry| Arc::clone(entry.value()))
    }

    /// 列出所有已注册工具的名称
    ///
    /// # 返回值
    /// 按字母排序的工具名称列表
    pub fn list_tools(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.iter().map(|entry| entry.key().clone()).collect();
        // 按字母排序，确保返回顺序的确定性
        names.sort();
        names
    }

    /// 获取所有已注册工具的 JSON Schema 数组
    ///
    /// 返回格式兼容 OpenAI 函数调用规范的工具定义数组，
    /// 可直接用于 AI 模型的 function calling 功能。
    ///
    /// # 返回值
    /// JSON 数组，每个元素包含工具的 `name`、`description` 和 `parameters`
    pub fn get_schemas(&self) -> Value {
        let mut schemas: Vec<Value> = self
            .tools
            .iter()
            .map(|entry| {
                let tool = entry.value();
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters_schema()
                    }
                })
            })
            .collect();

        // 按工具名称排序，确保输出顺序稳定
        schemas.sort_by(|a, b| {
            let name_a = a["function"]["name"].as_str().unwrap_or("");
            let name_b = b["function"]["name"].as_str().unwrap_or("");
            name_a.cmp(name_b)
        });

        Value::Array(schemas)
    }

    /// 获取注册表中的工具总数
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 检查注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 通过名称执行指定工具
    ///
    /// 便捷方法，自动查找工具并调用其 `execute` 方法。
    ///
    /// # 参数
    /// - `name`: 工具名称
    /// - `params`: 调用参数（JSON 格式）
    ///
    /// # 返回值
    /// 工具执行结果或错误
    pub async fn execute(&self, name: &str, params: Value) -> ToolResult<String> {
        let tool = self
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(params).await
    }
}

impl Default for ToolRegistry {
    /// 创建默认的空注册表
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 默认注册表工厂函数
// ============================================================

/// 创建包含所有内置文件工具的默认注册表
///
/// 注册以下工具：
/// - `read_file`: 读取文件内容
/// - `write_file`: 写入文件内容
/// - `edit_file`: 编辑文件（搜索替换）
/// - `list_directory`: 列出目录内容
/// - `search_files`: 搜索文件
/// - `delete_file`: 删除文件
pub fn create_default_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();

    // 注册所有内置文件操作工具
    info!("正在创建默认工具注册表...");

    registry.register(Arc::new(ReadFileTool));
    registry.register(Arc::new(WriteFileTool));
    registry.register(Arc::new(EditFileTool));
    registry.register(Arc::new(ListDirectoryTool));
    registry.register(Arc::new(SearchFilesTool));
    registry.register(Arc::new(DeleteFileTool));
    registry.register(Arc::new(BashTool::new()));
    registry.register(Arc::new(GrepTool::new()));
    registry.register(Arc::new(FindTool::new()));

    info!("默认工具注册表已创建，共 {} 个工具", registry.len());

    registry
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tool;
    use async_trait::async_trait;
    use serde_json::json;

    /// 用于测试的简单模拟工具
    #[derive(Debug)]
    struct TestTool {
        /// 工具名称
        tool_name: String,
    }

    impl TestTool {
        /// 创建新的测试工具实例
        fn new(name: &str) -> Self {
            Self {
                tool_name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.tool_name
        }

        fn description(&self) -> &str {
            "测试工具"
        }

        fn parameters_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {}
            })
        }

        async fn execute(&self, _params: Value) -> ToolResult<String> {
            Ok(format!("执行了工具: {}", self.tool_name))
        }
    }

    /// 测试注册和获取工具
    #[test]
    fn test_registry_register_and_get() {
        let registry = ToolRegistry::new();

        // 注册工具
        registry.register(Arc::new(TestTool::new("tool_a")));
        registry.register(Arc::new(TestTool::new("tool_b")));

        // 验证工具已注册
        assert!(registry.get("tool_a").is_some());
        assert!(registry.get("tool_b").is_some());

        // 验证不存在的工具返回 None
        assert!(registry.get("tool_c").is_none());

        // 验证工具总数
        assert_eq!(registry.len(), 2);
    }

    /// 测试移除工具
    #[test]
    fn test_registry_unregister() {
        let registry = ToolRegistry::new();

        registry.register(Arc::new(TestTool::new("to_remove")));
        assert_eq!(registry.len(), 1);

        // 成功移除
        assert!(registry.unregister("to_remove"));
        assert_eq!(registry.len(), 0);
        assert!(registry.get("to_remove").is_none());

        // 移除不存在的工具应返回 false
        assert!(!registry.unregister("nonexistent"));
    }

    /// 测试列出工具名称
    #[test]
    fn test_registry_list_tools() {
        let registry = ToolRegistry::new();

        registry.register(Arc::new(TestTool::new("charlie")));
        registry.register(Arc::new(TestTool::new("alpha")));
        registry.register(Arc::new(TestTool::new("bravo")));

        let names = registry.list_tools();

        // 验证按字母排序
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    /// 测试获取工具 JSON Schema 数组
    #[test]
    fn test_registry_get_schemas() {
        let registry = ToolRegistry::new();

        registry.register(Arc::new(TestTool::new("tool_b")));
        registry.register(Arc::new(TestTool::new("tool_a")));

        let schemas = registry.get_schemas();

        // 验证返回的是数组
        assert!(schemas.is_array());
        let arr = schemas.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        // 验证按名称排序（tool_a 在 tool_b 之前）
        assert_eq!(arr[0]["function"]["name"], "tool_a");
        assert_eq!(arr[1]["function"]["name"], "tool_b");

        // 验证 Schema 结构
        assert_eq!(arr[0]["type"], "function");
        assert!(arr[0]["function"]["parameters"].is_object());
    }

    /// 测试覆盖注册（同名工具重复注册）
    #[test]
    fn test_registry_override() {
        let registry = ToolRegistry::new();

        registry.register(Arc::new(TestTool::new("duplicate")));
        registry.register(Arc::new(TestTool::new("duplicate")));

        // 工具数量不应增加
        assert_eq!(registry.len(), 1);
    }

    /// 测试通过注册表执行工具
    #[tokio::test]
    async fn test_registry_execute() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(TestTool::new("my_tool")));

        // 执行已注册的工具
        let result = registry.execute("my_tool", json!({})).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("my_tool"));

        // 执行不存在的工具应返回 NotFound 错误
        let result = registry.execute("unknown", json!({})).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::NotFound(name) => assert_eq!(name, "unknown"),
            other => panic!("期望 NotFound 错误，得到: {:?}", other),
        }
    }

    /// 测试默认注册表包含所有内置工具
    #[test]
    fn test_create_default_registry() {
        let registry = create_default_registry();

        // 验证所有内置工具已注册
        assert_eq!(registry.len(), 9);

        // 验证每个内置工具都存在
        let expected_tools = vec![
            "read_file",
            "write_file",
            "edit_file",
            "list_directory",
            "search_files",
            "delete_file",
            "bash",
            "grep",
            "find",
        ];

        for tool_name in expected_tools {
            assert!(
                registry.get(tool_name).is_some(),
                "内置工具 '{}' 未注册",
                tool_name
            );
        }
    }

    /// 测试默认注册表的 Schema 输出
    #[test]
    fn test_default_registry_schemas() {
        let registry = create_default_registry();
        let schemas = registry.get_schemas();

        let arr = schemas.as_array().unwrap();
        assert_eq!(arr.len(), 9);

        // 验证每个 Schema 都包含必要字段
        for schema in arr {
            assert_eq!(schema["type"], "function");
            assert!(schema["function"]["name"].is_string());
            assert!(schema["function"]["description"].is_string());
            assert!(schema["function"]["parameters"].is_object());
        }
    }

    /// 测试空注册表
    #[test]
    fn test_empty_registry() {
        let registry = ToolRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.list_tools().is_empty());
        assert_eq!(registry.get_schemas(), Value::Array(vec![]));
    }
}
