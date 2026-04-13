//! # Tool Permission Invariant Tests
//!
//! INV-TOOL-01: 高危工具执行前必须权限检查
//! INV-TOOL-02: Deny 决策必须阻止执行
//! INV-TOOL-03: 输入验证必须在执行前完成

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chengcoding_tools::permissions::{PermissionContext, PermissionDecision};
use chengcoding_tools::{Tool, ToolError, ToolMetadata, ToolResult};
use serde_json::{json, Value};

// =========================================================================
// Mock tool that tracks call order
// =========================================================================

/// A mock destructive tool that records which methods are called and in what order.
#[derive(Debug)]
struct MockDestructiveTool {
    validate_called: AtomicBool,
    permissions_called: AtomicBool,
    execute_called: AtomicBool,
    call_order: Arc<std::sync::Mutex<Vec<&'static str>>>,
    force_permission: std::sync::Mutex<Option<PermissionDecision>>,
    force_validate_err: AtomicBool,
}

impl MockDestructiveTool {
    fn new() -> Self {
        Self {
            validate_called: AtomicBool::new(false),
            permissions_called: AtomicBool::new(false),
            execute_called: AtomicBool::new(false),
            call_order: Arc::new(std::sync::Mutex::new(Vec::new())),
            force_permission: std::sync::Mutex::new(None),
            force_validate_err: AtomicBool::new(false),
        }
    }

    fn with_permission(self, decision: PermissionDecision) -> Self {
        *self.force_permission.lock().unwrap() = Some(decision);
        self
    }

    fn with_validate_error(self) -> Self {
        self.force_validate_err.store(true, Ordering::SeqCst);
        self
    }

    fn was_execute_called(&self) -> bool {
        self.execute_called.load(Ordering::SeqCst)
    }

    fn was_permissions_called(&self) -> bool {
        self.permissions_called.load(Ordering::SeqCst)
    }

    fn call_sequence(&self) -> Vec<&'static str> {
        self.call_order.lock().unwrap().clone()
    }
}

#[async_trait]
impl Tool for MockDestructiveTool {
    fn name(&self) -> &str {
        "mock_destructive"
    }

    fn description(&self) -> &str {
        "A mock destructive tool for testing"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": { "type": "string" }
            },
            "required": ["target"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::destructive()
    }

    fn validate_input(&self, _params: &Value) -> ToolResult<()> {
        self.validate_called.store(true, Ordering::SeqCst);
        self.call_order.lock().unwrap().push("validate_input");
        if self.force_validate_err.load(Ordering::SeqCst) {
            return Err(ToolError::InvalidParams("forced validation error".into()));
        }
        Ok(())
    }

    fn check_permissions(&self, _params: &Value, _ctx: &PermissionContext) -> PermissionDecision {
        self.permissions_called.store(true, Ordering::SeqCst);
        self.call_order.lock().unwrap().push("check_permissions");
        self.force_permission
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(PermissionDecision::Allow)
    }

    async fn execute(&self, _params: Value) -> ToolResult<String> {
        self.execute_called.store(true, Ordering::SeqCst);
        self.call_order.lock().unwrap().push("execute");
        Ok("executed".into())
    }
}

/// Simulates the expected execution pipeline: validate → permissions → execute.
/// This is the contract that all tool executors must follow.
async fn run_tool_pipeline(
    tool: &dyn Tool,
    params: Value,
    ctx: &PermissionContext,
) -> ToolResult<String> {
    // Step 1: Validate input
    tool.validate_input(&params)?;

    // Step 2: Check permissions (required for destructive tools)
    if tool.metadata().is_destructive {
        let decision = tool.check_permissions(&params, ctx);
        match decision {
            PermissionDecision::Allow => {}
            PermissionDecision::Deny(reason) => {
                return Err(ToolError::SecurityViolation(reason));
            }
            PermissionDecision::Ask(_) => {
                // In real code this would pause for user approval
                return Err(ToolError::SecurityViolation("approval required".into()));
            }
        }
    }

    // Step 3: Execute
    tool.execute(params).await
}

// =========================================================================
// INV-TOOL-01: 高危工具执行前必须权限检查
// =========================================================================

#[tokio::test]
async fn inv_tool_01_destructive_tool_checks_permissions_before_execute() {
    let tool = MockDestructiveTool::new();
    let ctx = PermissionContext::default();
    let params = json!({"target": "/some/path"});

    let result = run_tool_pipeline(&tool, params, &ctx).await;
    assert!(result.is_ok());

    assert!(
        tool.was_permissions_called(),
        "破坏性工具必须在 execute 前调用 check_permissions"
    );

    let seq = tool.call_sequence();
    let perm_idx = seq.iter().position(|&s| s == "check_permissions").unwrap();
    let exec_idx = seq.iter().position(|&s| s == "execute").unwrap();
    assert!(
        perm_idx < exec_idx,
        "check_permissions 必须在 execute 之前: {:?}",
        seq
    );
}

#[tokio::test]
async fn inv_tool_01_full_pipeline_order_is_validate_permissions_execute() {
    let tool = MockDestructiveTool::new();
    let ctx = PermissionContext::default();
    let params = json!({"target": "test"});

    let _ = run_tool_pipeline(&tool, params, &ctx).await;

    assert_eq!(
        tool.call_sequence(),
        vec!["validate_input", "check_permissions", "execute"],
        "执行顺序必须为: validate → permissions → execute"
    );
}

// =========================================================================
// INV-TOOL-02: Deny 决策必须阻止执行
// =========================================================================

#[tokio::test]
async fn inv_tool_02_deny_prevents_execution() {
    let tool =
        MockDestructiveTool::new().with_permission(PermissionDecision::Deny("forbidden".into()));
    let ctx = PermissionContext::default();
    let params = json!({"target": "secret"});

    let result = run_tool_pipeline(&tool, params, &ctx).await;
    assert!(result.is_err(), "Deny 决策必须返回错误");

    match result.unwrap_err() {
        ToolError::SecurityViolation(reason) => {
            assert_eq!(reason, "forbidden");
        }
        other => panic!("应返回 SecurityViolation，实际为: {:?}", other),
    }

    assert!(!tool.was_execute_called(), "Deny 后 execute 不得被调用");
}

#[tokio::test]
async fn inv_tool_02_allow_permits_execution() {
    let tool = MockDestructiveTool::new().with_permission(PermissionDecision::Allow);
    let ctx = PermissionContext::default();
    let params = json!({"target": "safe"});

    let result = run_tool_pipeline(&tool, params, &ctx).await;
    assert!(result.is_ok(), "Allow 决策应允许执行");
    assert!(tool.was_execute_called(), "Allow 后 execute 应被调用");
}

// =========================================================================
// INV-TOOL-03: 输入验证必须在执行前完成
// =========================================================================

#[tokio::test]
async fn inv_tool_03_validate_error_prevents_execution() {
    let tool = MockDestructiveTool::new().with_validate_error();
    let ctx = PermissionContext::default();
    let params = json!({"target": "bad-input"});

    let result = run_tool_pipeline(&tool, params, &ctx).await;
    assert!(result.is_err(), "验证失败时必须返回错误");

    assert!(
        !tool.was_execute_called(),
        "validate_input 失败后 execute 不得被调用"
    );
    assert!(
        !tool.was_permissions_called(),
        "validate_input 失败后 check_permissions 不得被调用"
    );
}

#[tokio::test]
async fn inv_tool_03_valid_input_proceeds_to_execute() {
    let tool = MockDestructiveTool::new();
    let ctx = PermissionContext::default();
    let params = json!({"target": "good-input"});

    let result = run_tool_pipeline(&tool, params, &ctx).await;
    assert!(result.is_ok(), "有效输入应成功执行");
    assert!(tool.was_execute_called(), "有效输入后 execute 应被调用");
}
