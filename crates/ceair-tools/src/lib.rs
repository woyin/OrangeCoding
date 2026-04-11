//! # CEAIR 工具系统
//!
//! 本模块提供了 CEAIR AI 编程助手的工具系统框架，包括：
//! - 工具特征（Tool trait）定义
//! - 文件操作工具集
//! - 工具注册表管理
//! - 安全策略与路径验证

pub mod ask_tool;
pub mod ast_tool;
pub mod bash_tool;
pub mod browser_tool;
pub mod calc_tool;
pub mod edit_tool;
pub mod fetch_tool;
pub mod file_tools;
pub mod find_tool;
pub mod grep_tool;
pub mod lsp_tool;
pub mod notebook_tool;
pub mod permissions;
pub mod python_tool;
pub mod registry;
pub mod security;
pub mod ssh_tool;
pub mod task_tool;
pub mod todo_tool;
pub mod web_search_tool;
pub mod session_tools;
pub mod task_management;
pub mod tool_hooks;

use async_trait::async_trait;
use serde_json::Value;
use std::fmt;

// ============================================================
// 公共重导出
// ============================================================

pub use ask_tool::{AskResponse, AskTool, Choice, Question};
pub use ast_tool::{AstEditRequest, AstMatch, AstSearchRequest, AstTool};
pub use bash_tool::BashTool;
pub use browser_tool::{BrowserAction, BrowserResult, BrowserTool};
pub use calc_tool::CalcTool;
pub use edit_tool::{EditOperation, EditTool};
pub use fetch_tool::FetchTool;
pub use file_tools::{
    DeleteFileTool, EditFileTool, ListDirectoryTool, ReadFileTool, SearchFilesTool, WriteFileTool,
};
pub use find_tool::FindTool;
pub use grep_tool::GrepTool;
pub use lsp_tool::{LspRequest, LspResponse, LspResultItem, LspTool};
pub use notebook_tool::{
    CellOutput, CellType, Notebook, NotebookCell, NotebookMetadata, NotebookTool, OutputType,
};
pub use python_tool::{PythonMode, PythonResult, PythonTool};
pub use registry::{create_default_registry, ToolRegistry};
pub use security::{FileOperationGuard, PathValidator, SecurityPolicy};
pub use ssh_tool::{SshAuth, SshConnection, SshResult, SshTool};
pub use task_tool::{TaskRequest, TaskResult, TaskState, TaskTool};
pub use todo_tool::{Phase, TaskStatus, TodoState, TodoTask, TodoTool};
pub use web_search_tool::{SearchProvider, SearchResult, WebSearchTool};

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
// 工具元数据
// ============================================================

/// 工具元数据 — 描述工具的行为特性，用于执行调度和安全决策
///
/// 采用 TOOL_DEFAULTS 模式：所有字段提供安全的默认值，
/// 工具实现者只需覆盖需要修改的字段。
///
/// # 设计思想
/// 参考 reference 中 buildTool() 的 TOOL_DEFAULTS 模式：
/// - 默认不允许并发（防止竞态）
/// - 默认非只读（保守安全策略）
/// - 默认非破坏性
/// - 默认启用
/// 工具实现者通过覆盖特定字段来声明实际行为。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolMetadata {
    /// 工具是否为只读操作（不修改任何文件或状态）
    /// 只读工具可以安全地并发执行和在沙箱中运行
    pub is_read_only: bool,

    /// 工具是否可以安全地与其他工具并发执行
    /// 为 true 时，批量执行器会将其放入并发组
    pub is_concurrency_safe: bool,

    /// 工具是否具有破坏性（如删除文件、执行危险命令）
    /// 破坏性工具在执行前需要额外的权限确认
    pub is_destructive: bool,

    /// 工具是否当前可用
    /// 为 false 时工具不会出现在可用工具列表中
    pub is_enabled: bool,
}

/// 安全的默认值：保守策略，最小权限原则
impl Default for ToolMetadata {
    fn default() -> Self {
        Self {
            is_read_only: false,
            is_concurrency_safe: false,
            is_destructive: false,
            is_enabled: true,
        }
    }
}

impl ToolMetadata {
    /// 创建只读工具的元数据（自动标记为并发安全）
    ///
    /// 只读工具天然可以并发执行，因此同时设置 is_concurrency_safe
    pub fn read_only() -> Self {
        Self {
            is_read_only: true,
            is_concurrency_safe: true,
            ..Default::default()
        }
    }

    /// 创建破坏性工具的元数据
    pub fn destructive() -> Self {
        Self {
            is_destructive: true,
            ..Default::default()
        }
    }
}

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
/// - 可选的元数据方法（声明工具行为特性）
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

    /// 返回工具的行为元数据
    ///
    /// 默认返回安全的保守值。工具实现者可覆盖此方法
    /// 来声明工具的实际行为特性（如只读、可并发等）。
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }

    /// 异步执行工具操作
    ///
    /// # 参数
    /// - `params`: JSON 格式的调用参数，需符合 `parameters_schema` 定义的格式
    ///
    /// # 返回值
    /// - 成功时返回工具执行结果的字符串表示
    /// - 失败时返回 `ToolError`
    async fn execute(&self, params: Value) -> ToolResult<String>;

    /// 验证输入参数是否符合 parameters_schema
    ///
    /// 默认实现检查：
    /// 1. 参数是否为 JSON 对象
    /// 2. required 字段是否都存在
    /// 3. 已提供的字段类型是否匹配 schema
    ///
    /// 工具实现者可覆盖此方法添加自定义校验逻辑。
    fn validate_input(&self, params: &Value) -> ToolResult<()> {
        validate_params_against_schema(params, &self.parameters_schema())
    }

    /// 检查执行权限
    ///
    /// 在 validate_input 后、execute 前调用。
    /// 工具根据参数和权限上下文判断是否允许执行。
    ///
    /// 默认实现返回 Allow，工具实现者可覆盖以实现：
    /// - 路径级别的访问控制
    /// - 危险命令检测
    /// - 破坏性操作确认
    fn check_permissions(
        &self,
        _params: &Value,
        _ctx: &permissions::PermissionContext,
    ) -> permissions::PermissionDecision {
        permissions::PermissionDecision::Allow
    }
}

// ============================================================
// 输入验证
// ============================================================

/// 基于 JSON Schema 的参数验证
///
/// 为什么不使用完整的 JSON Schema 验证库：
/// - Agent 工具的 schema 简单（object + properties + required）
/// - 避免引入重量级依赖
/// - 保持验证逻辑可读可调试
fn validate_params_against_schema(params: &Value, schema: &Value) -> ToolResult<()> {
    // 1. 参数必须是对象
    let params_obj = params.as_object().ok_or_else(|| {
        ToolError::InvalidParams("参数必须是 JSON 对象".to_string())
    })?;

    // 2. 检查 required 字段
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        for field in required {
            if let Some(name) = field.as_str() {
                if !params_obj.contains_key(name) {
                    return Err(ToolError::InvalidParams(format!(
                        "缺少必需参数: {}",
                        name
                    )));
                }
            }
        }
    }

    // 3. 检查已提供字段的类型匹配
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (key, value) in params_obj {
            if let Some(prop_schema) = properties.get(key) {
                if let Some(expected_type) = prop_schema.get("type").and_then(|t| t.as_str()) {
                    if !value_matches_type(value, expected_type) {
                        return Err(ToolError::InvalidParams(format!(
                            "参数 '{}' 类型不匹配: 期望 {}, 实际 {}",
                            key,
                            expected_type,
                            json_type_name(value)
                        )));
                    }
                }
            }
        }
    }

    Ok(())
}

/// 检查 JSON 值是否匹配指定类型
fn value_matches_type(value: &Value, expected: &str) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true, // 未知类型默认通过
    }
}

/// 获取 JSON 值的类型名称
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
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

    // -----------------------------------------------------------------------
    // ToolMetadata 测试
    // -----------------------------------------------------------------------

    /// 测试 ToolMetadata 默认值 — 保守安全策略
    #[test]
    fn test_tool_metadata_defaults() {
        let meta = ToolMetadata::default();

        // 默认不允许并发、非只读、非破坏性、已启用
        assert!(!meta.is_read_only, "默认不应为只读");
        assert!(!meta.is_concurrency_safe, "默认不应允许并发");
        assert!(!meta.is_destructive, "默认不应为破坏性");
        assert!(meta.is_enabled, "默认应为启用");
    }

    /// 测试只读元数据快捷构造
    #[test]
    fn test_tool_metadata_read_only() {
        let meta = ToolMetadata::read_only();

        assert!(meta.is_read_only);
        // 只读工具天然可以并发执行
        assert!(meta.is_concurrency_safe);
        assert!(!meta.is_destructive);
        assert!(meta.is_enabled);
    }

    /// 测试破坏性元数据快捷构造
    #[test]
    fn test_tool_metadata_destructive() {
        let meta = ToolMetadata::destructive();

        assert!(meta.is_destructive);
        assert!(!meta.is_read_only);
        assert!(!meta.is_concurrency_safe);
        assert!(meta.is_enabled);
    }

    /// 测试自定义覆盖所有字段
    #[test]
    fn test_tool_metadata_custom_override() {
        let meta = ToolMetadata {
            is_read_only: true,
            is_concurrency_safe: true,
            is_destructive: false,
            is_enabled: false, // 显式禁用
        };

        assert!(meta.is_read_only);
        assert!(meta.is_concurrency_safe);
        assert!(!meta.is_destructive);
        assert!(!meta.is_enabled);
    }

    /// 测试 ToolMetadata 的 PartialEq 实现
    #[test]
    fn test_tool_metadata_equality() {
        let a = ToolMetadata::default();
        let b = ToolMetadata::default();
        assert_eq!(a, b);

        let c = ToolMetadata::read_only();
        assert_ne!(a, c);
    }

    /// 测试 Tool trait 默认的 metadata() 方法
    #[tokio::test]
    async fn test_tool_default_metadata() {
        let tool = MockTool;

        // MockTool 未覆盖 metadata()，应返回安全默认值
        let meta = tool.metadata();
        assert_eq!(meta, ToolMetadata::default());
    }

    /// 自定义元数据的模拟工具 — 验证覆盖生效
    #[derive(Debug)]
    struct ReadOnlyTool;

    #[async_trait]
    impl Tool for ReadOnlyTool {
        fn name(&self) -> &str {
            "read_only_tool"
        }
        fn description(&self) -> &str {
            "只读测试工具"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata::read_only()
        }
        async fn execute(&self, _params: Value) -> ToolResult<String> {
            Ok("只读结果".to_string())
        }
    }

    /// 测试覆盖 metadata() 方法后返回自定义值
    #[tokio::test]
    async fn test_tool_custom_metadata() {
        let tool = ReadOnlyTool;
        let meta = tool.metadata();

        assert!(meta.is_read_only);
        assert!(meta.is_concurrency_safe);
        assert!(!meta.is_destructive);
        assert!(meta.is_enabled);
    }

    /// 测试现有工具（MockTool）无需修改即可编译和使用
    #[tokio::test]
    async fn test_existing_tool_backward_compatible() {
        let tool = MockTool;

        // 所有原有方法依然可用
        assert_eq!(tool.name(), "mock_tool");
        assert_eq!(tool.description(), "这是一个用于单元测试的模拟工具");

        let result = tool.execute(json!({"message": "兼容性测试"})).await;
        assert!(result.is_ok());

        // metadata() 使用默认实现
        let meta = tool.metadata();
        assert_eq!(meta, ToolMetadata::default());
    }

    // ================================================================
    // validate_input 测试
    // ================================================================

    /// 测试合法参数通过验证
    #[test]
    fn test_validate_input_valid() {
        let tool = MockTool;
        let params = json!({"message": "hello"});
        assert!(tool.validate_input(&params).is_ok());
    }

    /// 测试缺少必需参数
    #[test]
    fn test_validate_input_missing_required() {
        let tool = MockTool;
        let params = json!({});
        let err = tool.validate_input(&params).unwrap_err();
        match err {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("message"), "应提示缺少 message: {}", msg);
            }
            other => panic!("期望 InvalidParams，得到 {:?}", other),
        }
    }

    /// 测试非对象参数被拒绝
    #[test]
    fn test_validate_input_not_object() {
        let tool = MockTool;
        let params = json!("string input");
        let err = tool.validate_input(&params).unwrap_err();
        match err {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("JSON 对象"), "应提示必须是对象: {}", msg);
            }
            other => panic!("期望 InvalidParams，得到 {:?}", other),
        }
    }

    /// 测试类型不匹配
    #[test]
    fn test_validate_input_type_mismatch() {
        let tool = MockTool;
        let params = json!({"message": 123}); // 应为 string
        let err = tool.validate_input(&params).unwrap_err();
        match err {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("类型不匹配"), "应提示类型不匹配: {}", msg);
            }
            other => panic!("期望 InvalidParams，得到 {:?}", other),
        }
    }

    /// 测试额外字段不影响验证（schema 无 additionalProperties 限制）
    #[test]
    fn test_validate_input_extra_fields() {
        let tool = MockTool;
        let params = json!({"message": "ok", "extra": 42});
        assert!(tool.validate_input(&params).is_ok());
    }

    /// 测试无 required 字段的 schema
    #[test]
    fn test_validate_input_no_required() {
        let schema = json!({"type": "object", "properties": {"x": {"type": "number"}}});
        let params = json!({});
        assert!(validate_params_against_schema(&params, &schema).is_ok());
    }

    /// 测试所有 JSON 类型匹配
    #[test]
    fn test_value_matches_type_all() {
        assert!(value_matches_type(&json!("hi"), "string"));
        assert!(value_matches_type(&json!(42), "number"));
        assert!(value_matches_type(&json!(42), "integer"));
        assert!(value_matches_type(&json!(true), "boolean"));
        assert!(value_matches_type(&json!([1, 2]), "array"));
        assert!(value_matches_type(&json!({}), "object"));
        assert!(value_matches_type(&json!(null), "null"));

        // 不匹配的情况
        assert!(!value_matches_type(&json!("hi"), "number"));
        assert!(!value_matches_type(&json!(42), "string"));
        assert!(!value_matches_type(&json!(true), "string"));
    }

    /// 测试未知类型默认通过
    #[test]
    fn test_value_matches_unknown_type() {
        assert!(value_matches_type(&json!("anything"), "custom_type"));
    }

    /// 测试 json_type_name 覆盖所有类型
    #[test]
    fn test_json_type_name_all() {
        assert_eq!(json_type_name(&json!(null)), "null");
        assert_eq!(json_type_name(&json!(true)), "boolean");
        assert_eq!(json_type_name(&json!(42)), "number");
        assert_eq!(json_type_name(&json!("hi")), "string");
        assert_eq!(json_type_name(&json!([1])), "array");
        assert_eq!(json_type_name(&json!({})), "object");
    }

    /// 测试空对象参数对无 required schema 通过
    #[test]
    fn test_validate_empty_params_no_required() {
        let schema = json!({"type": "object", "properties": {}});
        assert!(validate_params_against_schema(&json!({}), &schema).is_ok());
    }

    /// 测试 null 参数被拒绝
    #[test]
    fn test_validate_null_params() {
        let schema = json!({"type": "object"});
        assert!(validate_params_against_schema(&json!(null), &schema).is_err());
    }

    /// 测试 array 参数被拒绝
    #[test]
    fn test_validate_array_params() {
        let schema = json!({"type": "object"});
        assert!(validate_params_against_schema(&json!([1, 2, 3]), &schema).is_err());
    }

    // ======== check_permissions 默认实现测试 ========

    /// 测试 Tool trait 默认 check_permissions 返回 Allow
    #[test]
    fn test_default_check_permissions_returns_allow() {
        let tool = MockTool;
        let ctx = permissions::PermissionContext::new("/project");
        let params = json!({"message": "test"});
        let decision = tool.check_permissions(&params, &ctx);
        assert!(decision.is_allow());
    }

    /// 测试使用自定义权限检查的工具
    #[derive(Debug)]
    struct RestrictedTool;

    #[async_trait]
    impl Tool for RestrictedTool {
        fn name(&self) -> &str {
            "restricted"
        }

        fn description(&self) -> &str {
            "受限工具"
        }

        fn parameters_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            })
        }

        async fn execute(&self, _params: Value) -> ToolResult<String> {
            Ok("ok".to_string())
        }

        fn check_permissions(
            &self,
            params: &Value,
            ctx: &permissions::PermissionContext,
        ) -> permissions::PermissionDecision {
            // 检查路径参数是否在允许范围内
            if let Some(path_str) = params.get("path").and_then(|v| v.as_str()) {
                ctx.check_path(std::path::Path::new(path_str))
            } else {
                permissions::PermissionDecision::Allow
            }
        }
    }

    /// 测试自定义权限检查 — 允许工作目录内路径
    #[test]
    fn test_custom_check_permissions_allow() {
        let tool = RestrictedTool;
        let ctx = permissions::PermissionContext::new("/project");
        let params = json!({"path": "/project/src/main.rs"});
        let decision = tool.check_permissions(&params, &ctx);
        assert!(decision.is_allow());
    }

    /// 测试自定义权限检查 — 工作目录外路径需要确认
    #[test]
    fn test_custom_check_permissions_ask() {
        let tool = RestrictedTool;
        let ctx = permissions::PermissionContext::new("/project");
        let params = json!({"path": "/etc/passwd"});
        let decision = tool.check_permissions(&params, &ctx);
        assert!(decision.is_ask());
    }

    /// 测试自定义权限检查 — 黑名单路径被拒绝
    #[test]
    fn test_custom_check_permissions_deny() {
        let tool = RestrictedTool;
        let mut ctx = permissions::PermissionContext::new("/project");
        ctx.denied_patterns.push(".env".into());
        let params = json!({"path": "/project/.env"});
        let decision = tool.check_permissions(&params, &ctx);
        assert!(decision.is_deny());
    }
}
