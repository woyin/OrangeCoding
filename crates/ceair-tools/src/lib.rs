//! # CEAIR 工具系统
//!
//! 本模块提供了 CEAIR AI 编程助手的工具系统框架，包括：
//! - 工具特征（Tool trait）定义
//! - 文件操作工具集
//! - 工具注册表管理
//! - 安全策略与路径验证

pub mod file_tools;
pub mod registry;
pub mod security;

use async_trait::async_trait;
use serde_json::Value;
use std::fmt;

// ============================================================
// 公共重导出
// ============================================================

pub use file_tools::{
    DeleteFileTool, EditFileTool, ListDirectoryTool, ReadFileTool, SearchFilesTool, WriteFileTool,
};
pub use registry::{create_default_registry, ToolRegistry};
pub use security::{FileOperationGuard, PathValidator, SecurityPolicy};

// ============================================================
// 工具错误类型
// ============================================================

/// 工具执行过程中可能出现的错误
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// 参数验证失败（缺少必要参数或参数格式不正确）
    #[error("参数错误: {0}")]
    InvalidParams(String),

    /// 工具执行过程中的运行时错误
    #[error("执行错误: {0}")]
    ExecutionError(String),

    /// 文件系统 IO 操作错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 安全策略拦截（路径不允许访问等）
    #[error("安全检查失败: {0}")]
    SecurityViolation(String),

    /// 工具未找到
    #[error("工具未找到: {0}")]
    NotFound(String),
}

/// 工具执行的统一结果类型
pub type ToolResult<T> = Result<T, ToolError>;

// ============================================================
// 工具特征定义
// ============================================================

/// 工具特征（Tool Trait）
///
/// 所有可供 AI 智能体调用的工具都必须实现此特征。
/// 每个工具需要提供：
/// - 唯一名称（用于注册和调用）
/// - 功能描述（供 AI 理解工具用途）
/// - 参数模式（JSON Schema 格式，用于 AI 函数调用）
/// - 异步执行方法（接收 JSON 参数，返回字符串结果）
#[async_trait]
pub trait Tool: Send + Sync + fmt::Debug {
    /// 返回工具的唯一标识名称
    fn name(&self) -> &str;

    /// 返回工具的功能描述（供 AI 模型理解用途）
    fn description(&self) -> &str;

    /// 返回工具参数的 JSON Schema 定义
    ///
    /// 该 Schema 遵循 JSON Schema 规范，用于 AI 函数调用时的参数校验。
    /// 返回值应包含 `type`、`properties`、`required` 等字段。
    fn parameters_schema(&self) -> Value;

    /// 异步执行工具操作
    ///
    /// # 参数
    /// - `params`: JSON 格式的调用参数，需符合 `parameters_schema` 定义的格式
    ///
    /// # 返回值
    /// - 成功时返回工具执行结果的字符串表示
    /// - 失败时返回 `ToolError`
    async fn execute(&self, params: Value) -> ToolResult<String>;
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 用于测试的模拟工具
    #[derive(Debug)]
    struct MockTool;

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            "mock_tool"
        }

        fn description(&self) -> &str {
            "这是一个用于单元测试的模拟工具"
        }

        fn parameters_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "测试消息"
                    }
                },
                "required": ["message"]
            })
        }

        async fn execute(&self, params: Value) -> ToolResult<String> {
            // 从参数中提取消息字段
            let message = params
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidParams("缺少 message 参数".to_string()))?;
            Ok(format!("模拟执行: {}", message))
        }
    }

    /// 测试 Tool 特征的基本方法
    #[tokio::test]
    async fn test_tool_trait_basic_methods() {
        let tool = MockTool;

        // 验证工具名称
        assert_eq!(tool.name(), "mock_tool");

        // 验证工具描述
        assert_eq!(tool.description(), "这是一个用于单元测试的模拟工具");

        // 验证参数模式包含必要字段
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["message"].is_object());
    }

    /// 测试工具执行成功的场景
    #[tokio::test]
    async fn test_tool_execute_success() {
        let tool = MockTool;
        let params = json!({"message": "你好世界"});

        let result = tool.execute(params).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "模拟执行: 你好世界");
    }

    /// 测试工具执行参数缺失的场景
    #[tokio::test]
    async fn test_tool_execute_missing_params() {
        let tool = MockTool;
        let params = json!({});

        let result = tool.execute(params).await;
        assert!(result.is_err());

        // 验证返回的是参数错误
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("message"));
            }
            other => panic!("期望 InvalidParams 错误，实际得到: {:?}", other),
        }
    }

    /// 测试 ToolError 的显示格式
    #[test]
    fn test_tool_error_display() {
        let err = ToolError::InvalidParams("测试参数错误".to_string());
        assert_eq!(format!("{}", err), "参数错误: 测试参数错误");

        let err = ToolError::SecurityViolation("路径被禁止".to_string());
        assert_eq!(format!("{}", err), "安全检查失败: 路径被禁止");

        let err = ToolError::NotFound("unknown_tool".to_string());
        assert_eq!(format!("{}", err), "工具未找到: unknown_tool");
    }
}
