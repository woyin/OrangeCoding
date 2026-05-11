//! # 工具执行器
//!
//! 本模块提供 `ToolExecutor`，负责接收 AI 模型返回的工具调用请求，
//! 通过 `ToolRegistry` 查找并执行对应的工具，支持：
//! - 单个工具调用执行
//! - 批量并行执行多个工具调用
//! - 单个工具执行的超时控制
//! - 执行错误的安全包装

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use orangecoding_core::message::{ToolCall, ToolResult};
use orangecoding_invariant::gate::{GateDecision, GateReport, PreCheckGate};
use orangecoding_invariant::runtime_guard::{
    GuardAction, RiskLevel, RuntimeGuard, ToolCallContext,
};
use orangecoding_tools::permissions::PermissionContext;
use orangecoding_tools::ToolRegistry;
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// 默认常量
// ---------------------------------------------------------------------------

/// 单个工具执行的默认超时时间（30 秒）
const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Pre-Check Gate 类型
// ---------------------------------------------------------------------------

/// Pre-check gate 配置，仅自主模式启用
#[derive(Debug, Clone)]
pub struct GateCheckConfig {
    /// 工作目录，用于运行 git diff
    pub working_directory: PathBuf,
}

/// Gate 检查结果
enum GateOutcome {
    Allow,
    Warn(GateReport),
    Block(GateReport),
}

/// Diff 获取抽象，便于测试替换
pub trait DiffProvider: Send + Sync {
    fn get_diff(&self, working_dir: &PathBuf) -> String;
}

/// 生产环境使用 git 命令获取 diff
pub struct GitDiffProvider;

impl DiffProvider for GitDiffProvider {
    fn get_diff(&self, working_dir: &PathBuf) -> String {
        let output = std::process::Command::new("git")
            .args(["diff", "--cached"])
            .current_dir(working_dir)
            .output();

        match output {
            Ok(o) if !o.stdout.is_empty() => String::from_utf8_lossy(&o.stdout).to_string(),
            Ok(_) => {
                let output2 = std::process::Command::new("git")
                    .args(["diff", "HEAD"])
                    .current_dir(working_dir)
                    .output();
                match output2 {
                    Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                    Err(_) => String::new(),
                }
            }
            Err(_) => String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// 工具执行器
// ---------------------------------------------------------------------------

/// 工具执行器 - 桥接 AI 工具调用与实际工具执行
///
/// 封装了 `ToolRegistry`，提供：
/// - 将 `ToolCall`（AI 请求格式）转换为实际工具调用
/// - 将工具执行结果转换为 `ToolResult`（可回传给 AI）
/// - 支持并行批量执行
/// - 每个工具调用的超时保护
pub struct ToolExecutor {
    /// 工具注册表，存储所有可用工具
    registry: Arc<ToolRegistry>,
    /// 单个工具执行的超时时间
    timeout: Duration,
    /// 权限上下文（用于 check_permissions）
    permission_ctx: Option<PermissionContext>,
    /// Pre-check gate 配置（仅自主模式启用）
    gate_config: Option<GateCheckConfig>,
    /// Diff 获取提供者（默认 GitDiffProvider，测试可替换）
    diff_provider: Arc<dyn DiffProvider>,
    /// 运行时守卫（仅自主模式启用）
    runtime_guard: Option<RuntimeGuard>,
}

impl ToolExecutor {
    /// 创建新的工具执行器
    ///
    /// # 参数
    /// - `registry`: 工具注册表的共享引用
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            timeout: DEFAULT_TOOL_TIMEOUT,
            permission_ctx: None,
            gate_config: None,
            diff_provider: Arc::new(GitDiffProvider),
            runtime_guard: None,
        }
    }

    /// 设置工具执行超时时间
    ///
    /// # 参数
    /// - `timeout`: 超时时长
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置权限上下文
    pub fn with_permission_context(mut self, ctx: PermissionContext) -> Self {
        self.permission_ctx = Some(ctx);
        self
    }

    /// 启用 pre-check gate（自主模式）
    pub fn with_gate_check(mut self, config: GateCheckConfig) -> Self {
        self.gate_config = Some(config);
        self
    }

    /// 替换 diff 提供者（测试用）
    pub fn with_diff_provider(mut self, provider: Arc<dyn DiffProvider>) -> Self {
        self.diff_provider = provider;
        self
    }

    /// 启用运行时守卫（自主模式）
    pub fn with_runtime_guard(mut self) -> Self {
        self.runtime_guard = Some(RuntimeGuard::new());
        self
    }

    /// 执行单个工具调用
    ///
    /// 根据 `ToolCall` 中的函数名称在注册表中查找工具并执行。
    /// 如果工具不存在、参数解析失败或执行超时，均返回错误类型的 `ToolResult`。
    ///
    /// # 参数
    /// - `tool_call`: AI 模型请求的工具调用
    ///
    /// # 返回值
    /// 工具执行结果（成功或错误均封装在 `ToolResult` 中）
    pub async fn execute_tool_call(&self, tool_call: &ToolCall) -> ToolResult {
        let tool_name = &tool_call.function_name;
        let tool_call_id = &tool_call.id;

        info!("开始执行工具调用: {} (ID: {})", tool_name, tool_call_id);
        debug!("工具参数: {}", tool_call.arguments);

        // 在注册表中查找工具
        let tool = match self.registry.get(tool_name) {
            Some(t) => t,
            None => {
                warn!("工具未找到: {}", tool_name);
                return ToolResult::error(
                    tool_call_id,
                    format!("工具 '{}' 未注册，请检查工具名称是否正确", tool_name),
                );
            }
        };

        if let Err(err) = tool.validate_input(&tool_call.arguments) {
            warn!(
                "工具 {} 参数验证失败: {} (ID: {})",
                tool_name, err, tool_call_id
            );
            return ToolResult::error(tool_call_id, format!("参数验证失败: {}", err));
        }

        // 权限检查
        if let Some(ref ctx) = self.permission_ctx {
            use orangecoding_tools::permissions::PermissionDecision;
            let params = &tool_call.arguments;
            match tool.check_permissions(params, ctx) {
                PermissionDecision::Allow => {}
                PermissionDecision::Deny(reason) => {
                    warn!(
                        "工具 {} 权限被拒绝: {} (ID: {})",
                        tool_name, reason, tool_call_id
                    );
                    return ToolResult::error(tool_call_id, format!("权限拒绝: {}", reason));
                }
                PermissionDecision::Ask(prompt) => {
                    warn!(
                        "工具 {} 需要用户确认: {} (ID: {})",
                        tool_name, prompt, tool_call_id
                    );
                    return ToolResult::error(tool_call_id, format!("需要用户确认: {}", prompt));
                }
            }
        }

        // Pre-check gate: 自主模式下拦截 git commit
        if let Some(ref gate_cfg) = self.gate_config {
            if is_git_commit_call(tool_name, &tool_call.arguments) {
                match self.run_gate_check(&gate_cfg.working_directory) {
                    GateOutcome::Block(report) => {
                        warn!("Pre-check gate 阻止提交: {}", report.reason);
                        return ToolResult::error(
                            tool_call_id,
                            format!(
                                "Pre-check gate BLOCKED commit:\n{}\n\n请先解决不变量违规后再提交。",
                                report.to_markdown()
                            ),
                        );
                    }
                    GateOutcome::Warn(report) => {
                        warn!("Pre-check gate 警告: {}", report.reason);
                    }
                    GateOutcome::Allow => {}
                }
            }
        }

        // Runtime guard: 自主模式下拦截高危工具调用
        if let Some(ref guard) = self.runtime_guard {
            let risk = determine_risk_level(tool_name, &tool_call.arguments);
            let ctx = ToolCallContext {
                tool_name: tool_name.to_string(),
                session_id: String::new(),
                authenticated: true,
                risk_level: risk,
            };
            match guard.check_tool_call(&ctx) {
                GuardAction::Deny(reason) => {
                    warn!("Runtime guard 拦截: {}", reason);
                    return ToolResult::error(tool_call_id, format!("Runtime guard 拒绝: {}", reason));
                }
                GuardAction::RequireApproval(reason) => {
                    warn!("Runtime guard 需要审批: {}", reason);
                    // 在自主模式中，审批需求被视为警告，但继续执行
                    // 因为自主模式没有交互式审批通道
                }
                GuardAction::Allow => {}
            }
        }

        // 使用 tokio::time::timeout 保护工具执行
        let params = tool_call.arguments.clone();
        let execution = tool.execute(params);

        match tokio::time::timeout(self.timeout, execution).await {
            // 执行成功
            Ok(Ok(content)) => {
                info!("工具 {} 执行成功 (ID: {})", tool_name, tool_call_id);
                debug!("工具返回内容长度: {} 字节", content.len());
                ToolResult::success(tool_call_id, content)
            }
            // 工具返回错误
            Ok(Err(tool_err)) => {
                error!(
                    "工具 {} 执行失败: {} (ID: {})",
                    tool_name, tool_err, tool_call_id
                );
                ToolResult::error(
                    tool_call_id,
                    format!("工具 '{}' 执行错误: {}", tool_name, tool_err),
                )
            }
            // 执行超时
            Err(_elapsed) => {
                error!(
                    "工具 {} 执行超时 (超过 {:?}) (ID: {})",
                    tool_name, self.timeout, tool_call_id
                );
                ToolResult::error(
                    tool_call_id,
                    format!(
                        "工具 '{}' 执行超时（限制 {} 秒）",
                        tool_name,
                        self.timeout.as_secs()
                    ),
                )
            }
        }
    }

    /// 批量并行执行多个工具调用
    ///
    /// 使用 `tokio::join!` 语义（通过 `futures::future::join_all`）并行执行所有工具调用，
    /// 结果顺序与输入顺序一致。
    ///
    /// # 参数
    /// - `tool_calls`: 要执行的工具调用切片
    ///
    /// # 返回值
    /// 所有工具的执行结果列表，与输入顺序对应
    pub async fn execute_batch(&self, tool_calls: &[ToolCall]) -> Vec<ToolResult> {
        if tool_calls.is_empty() {
            return Vec::new();
        }

        info!("开始批量执行 {} 个工具调用", tool_calls.len());

        use orangecoding_tools::batch_partition::{partition_tool_calls, ToolCallInfo};

        let call_infos: Vec<ToolCallInfo> = tool_calls
            .iter()
            .enumerate()
            .map(|(idx, tc)| {
                let is_concurrency_safe = self
                    .registry
                    .get(&tc.function_name)
                    .map(|t| t.metadata().is_concurrency_safe)
                    .unwrap_or(false);
                ToolCallInfo {
                    original_index: idx,
                    tool_name: tc.function_name.clone(),
                    call_id: tc.id.clone(),
                    is_concurrency_safe,
                }
            })
            .collect();

        let batches = partition_tool_calls(call_infos);

        let mut indexed_results: Vec<(usize, ToolResult)> = Vec::new();

        for batch in batches {
            if batch.concurrent {
                let futures: Vec<_> = batch
                    .calls
                    .iter()
                    .map(|info| {
                        let idx = info.original_index;
                        async move { (idx, self.execute_tool_call(&tool_calls[idx]).await) }
                    })
                    .collect();
                let batch_results = futures::future::join_all(futures).await;
                indexed_results.extend(batch_results);
            } else {
                for info in &batch.calls {
                    let idx = info.original_index;
                    let result = self.execute_tool_call(&tool_calls[idx]).await;
                    indexed_results.push((idx, result));
                }
            }
        }

        indexed_results.sort_by_key(|(idx, _)| *idx);
        let results: Vec<ToolResult> = indexed_results.into_iter().map(|(_, r)| r).collect();

        let success_count = results.iter().filter(|r| !r.is_error).count();
        let error_count = results.len() - success_count;

        info!(
            "批量执行完成: {} 个成功, {} 个失败",
            success_count, error_count
        );

        results
    }

    /// 执行 pre-check gate 分析
    fn run_gate_check(&self, working_dir: &PathBuf) -> GateOutcome {
        let diff = self.diff_provider.get_diff(working_dir);
        let gate = PreCheckGate::with_system_rules();
        let report = gate.analyze_diff(&diff);
        match report.decision {
            GateDecision::Block => GateOutcome::Block(report),
            GateDecision::Warn => GateOutcome::Warn(report),
            GateDecision::Allow => GateOutcome::Allow,
        }
    }

    /// 获取工具注册表的引用
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// 获取当前超时设置
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// 检测工具调用是否为 bash 中的 git commit 命令
fn is_git_commit_call(tool_name: &str, args: &serde_json::Value) -> bool {
    if tool_name != "bash" && tool_name != "Bash" {
        return false;
    }
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let lower = command.to_lowercase();
    lower.contains("git") && lower.contains("commit")
}

/// 根据工具名称和参数确定风险级别
fn determine_risk_level(tool_name: &str, args: &serde_json::Value) -> RiskLevel {
    let high_risk: &[&str] = &["bash", "edit", "delete", "ssh", "web_fetch"];
    if high_risk.contains(&tool_name) {
        if tool_name == "bash" {
            let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let lower = command.to_lowercase();
            if lower.contains("rm ") || lower.contains("format") || lower.contains("mkfs") {
                return RiskLevel::Critical;
            }
            if lower.contains("sudo") || lower.contains("chmod") || lower.contains("chown") {
                return RiskLevel::High;
            }
        }
        RiskLevel::High
    } else if tool_name == "read" || tool_name == "grep" || tool_name == "find" {
        RiskLevel::Safe
    } else {
        RiskLevel::Medium
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use orangecoding_tools::permissions;
    use orangecoding_tools::{Tool, ToolError};
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::time::Duration;

    // -----------------------------------------------------------------------
    // 测试用模拟工具
    // -----------------------------------------------------------------------

    /// 始终成功的模拟工具
    #[derive(Debug)]
    struct SuccessTool;

    #[async_trait]
    impl Tool for SuccessTool {
        fn name(&self) -> &str {
            "success_tool"
        }
        fn description(&self) -> &str {
            "始终返回成功的测试工具"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
            Ok("执行成功".to_string())
        }
    }

    /// 始终失败的模拟工具
    #[derive(Debug)]
    struct FailTool;

    #[async_trait]
    impl Tool for FailTool {
        fn name(&self) -> &str {
            "fail_tool"
        }
        fn description(&self) -> &str {
            "始终返回错误的测试工具"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
            Err(ToolError::ExecutionError("模拟执行失败".to_string()))
        }
    }

    /// 模拟耗时较长的工具（用于超时测试）
    #[derive(Debug)]
    struct SlowTool;

    #[async_trait]
    impl Tool for SlowTool {
        fn name(&self) -> &str {
            "slow_tool"
        }
        fn description(&self) -> &str {
            "模拟耗时较长的工具"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
            // 睡眠 5 秒模拟慢操作
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok("慢速执行完成".to_string())
        }
    }

    /// 辅助函数：创建包含测试工具的执行器
    fn create_test_executor() -> ToolExecutor {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(SuccessTool));
        registry.register(Arc::new(FailTool));
        registry.register(Arc::new(SlowTool));
        ToolExecutor::new(Arc::new(registry))
    }

    /// 辅助函数：创建工具调用
    fn make_tool_call(id: &str, name: &str) -> ToolCall {
        ToolCall::new(id, name, json!({}))
    }

    // -----------------------------------------------------------------------
    // 测试用例
    // -----------------------------------------------------------------------

    /// 测试成功执行工具调用
    #[tokio::test]
    async fn test_execute_tool_call_success() {
        let executor = create_test_executor();
        let call = make_tool_call("call_1", "success_tool");

        let result = executor.execute_tool_call(&call).await;

        assert!(!result.is_error);
        assert_eq!(result.tool_call_id, "call_1");
        assert!(result.content.contains("执行成功"));
    }

    /// 测试工具执行失败
    #[tokio::test]
    async fn test_execute_tool_call_failure() {
        let executor = create_test_executor();
        let call = make_tool_call("call_2", "fail_tool");

        let result = executor.execute_tool_call(&call).await;

        assert!(result.is_error);
        assert_eq!(result.tool_call_id, "call_2");
        assert!(result.content.contains("执行错误"));
    }

    /// 测试调用不存在的工具
    #[tokio::test]
    async fn test_execute_tool_call_not_found() {
        let executor = create_test_executor();
        let call = make_tool_call("call_3", "nonexistent_tool");

        let result = executor.execute_tool_call(&call).await;

        assert!(result.is_error);
        assert!(result.content.contains("未注册"));
    }

    #[tokio::test]
    async fn test_execute_tool_call_validates_input() {
        #[derive(Debug)]
        struct RequiredParamTool;

        #[async_trait]
        impl Tool for RequiredParamTool {
            fn name(&self) -> &str {
                "required_param"
            }
            fn description(&self) -> &str {
                "需要参数的测试工具"
            }
            fn parameters_schema(&self) -> Value {
                json!({
                    "type": "object",
                    "properties": {"message": {"type": "string"}},
                    "required": ["message"]
                })
            }
            async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
                Ok("不应执行".to_string())
            }
        }

        let registry = ToolRegistry::new();
        registry.register(Arc::new(RequiredParamTool));
        let executor = ToolExecutor::new(Arc::new(registry));

        let result = executor
            .execute_tool_call(&ToolCall::new("invalid", "required_param", json!({})))
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("参数验证失败"));
    }

    /// 测试工具执行超时
    #[tokio::test]
    async fn test_execute_tool_call_timeout() {
        let executor = create_test_executor().with_timeout(Duration::from_millis(100));
        let call = make_tool_call("call_4", "slow_tool");

        let result = executor.execute_tool_call(&call).await;

        assert!(result.is_error);
        assert!(result.content.contains("超时"));
    }

    /// 测试批量并行执行
    #[tokio::test]
    async fn test_execute_batch() {
        let executor = create_test_executor();
        let calls = vec![
            make_tool_call("batch_1", "success_tool"),
            make_tool_call("batch_2", "fail_tool"),
            make_tool_call("batch_3", "success_tool"),
        ];

        let results = executor.execute_batch(&calls).await;

        // 验证结果数量与输入一致
        assert_eq!(results.len(), 3);

        // 第一个和第三个应成功
        assert!(!results[0].is_error);
        assert_eq!(results[0].tool_call_id, "batch_1");

        // 第二个应失败
        assert!(results[1].is_error);
        assert_eq!(results[1].tool_call_id, "batch_2");

        // 第三个应成功
        assert!(!results[2].is_error);
        assert_eq!(results[2].tool_call_id, "batch_3");
    }

    /// 测试空批量执行
    #[tokio::test]
    async fn test_execute_batch_empty() {
        let executor = create_test_executor();
        let results = executor.execute_batch(&[]).await;
        assert!(results.is_empty());
    }

    /// 测试自定义超时设置
    #[test]
    fn test_with_timeout() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = ToolExecutor::new(registry).with_timeout(Duration::from_secs(120));

        assert_eq!(executor.timeout(), Duration::from_secs(120));
    }

    /// 测试默认超时值
    #[test]
    fn test_default_timeout() {
        let registry = Arc::new(ToolRegistry::new());
        let executor = ToolExecutor::new(registry);

        assert_eq!(executor.timeout(), DEFAULT_TOOL_TIMEOUT);
    }

    // --- BUG-001: check_permissions 强制执行测试 ---

    /// 覆盖 check_permissions 返回 Deny 的工具
    #[derive(Debug)]
    struct DenyTool {
        execute_count: AtomicUsize,
    }

    #[async_trait]
    impl Tool for DenyTool {
        fn name(&self) -> &str {
            "deny_tool"
        }
        fn description(&self) -> &str {
            "权限被拒绝的工具"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
            self.execute_count.fetch_add(1, AtomicOrdering::SeqCst);
            Ok("不应执行".to_string())
        }
        fn check_permissions(
            &self,
            _params: &Value,
            _ctx: &permissions::PermissionContext,
        ) -> permissions::PermissionDecision {
            permissions::PermissionDecision::Deny("权限被拒绝: 危险操作".to_string())
        }
    }

    /// 测试 check_permissions 返回 Deny 时工具不应执行
    ///
    /// 当前实现：execute_tool_call 忽略 check_permissions，工具照常执行。
    /// 修复后：应返回 is_error=true 的 ToolResult，且 execute 不被调用。
    #[tokio::test]
    async fn test_execute_tool_call_respects_deny_permission() {
        let registry = ToolRegistry::new();
        let deny_tool = DenyTool {
            execute_count: AtomicUsize::new(0),
        };
        registry.register(Arc::new(deny_tool));

        let executor = ToolExecutor::new(Arc::new(registry))
            .with_permission_context(permissions::PermissionContext::new("/project"));
        let call = make_tool_call("call_deny", "deny_tool");

        let result = executor.execute_tool_call(&call).await;

        assert!(result.is_error, "Deny 权限应导致错误结果");
        assert!(
            result.content.contains("拒绝") || result.content.contains("deny"),
            "错误信息应包含拒绝原因，实际: {}",
            result.content
        );
    }

    /// 覆盖 check_permissions 返回 Allow 的工具（应正常执行）
    #[derive(Debug)]
    struct AllowTool;

    #[async_trait]
    impl Tool for AllowTool {
        fn name(&self) -> &str {
            "allow_tool"
        }
        fn description(&self) -> &str {
            "权限允许的工具"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
            Ok("允许执行".to_string())
        }
        fn check_permissions(
            &self,
            _params: &Value,
            _ctx: &permissions::PermissionContext,
        ) -> permissions::PermissionDecision {
            permissions::PermissionDecision::Allow
        }
    }

    #[tokio::test]
    async fn test_execute_tool_call_allows_when_permitted() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(AllowTool));

        let executor = ToolExecutor::new(Arc::new(registry))
            .with_permission_context(permissions::PermissionContext::new("/project"));
        let call = make_tool_call("call_allow", "allow_tool");

        let result = executor.execute_tool_call(&call).await;

        assert!(!result.is_error, "Allow 权限应正常执行");
        assert!(result.content.contains("允许执行"));
    }

    // --- BUG-002: execute_batch 使用 partitioner 测试 ---

    /// 并发安全的只读工具（带执行计数器）
    #[derive(Debug)]
    struct SafeCountTool {
        count: AtomicUsize,
    }

    #[async_trait]
    impl Tool for SafeCountTool {
        fn name(&self) -> &str {
            "safe_count"
        }
        fn description(&self) -> &str {
            "并发安全的只读工具"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn metadata(&self) -> orangecoding_tools::ToolMetadata {
            orangecoding_tools::ToolMetadata::read_only()
        }
        async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
            self.count.fetch_add(1, AtomicOrdering::SeqCst);
            Ok("safe".to_string())
        }
    }

    /// 非并发安全的写入工具（带执行计数器）
    #[derive(Debug)]
    struct UnsafeCountTool {
        count: AtomicUsize,
    }

    #[async_trait]
    impl Tool for UnsafeCountTool {
        fn name(&self) -> &str {
            "unsafe_count"
        }
        fn description(&self) -> &str {
            "非并发安全的写入工具"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn metadata(&self) -> orangecoding_tools::ToolMetadata {
            orangecoding_tools::ToolMetadata::default()
        }
        async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
            self.count.fetch_add(1, AtomicOrdering::SeqCst);
            Ok("unsafe".to_string())
        }
    }

    /// 测试混合 safe/unsafe 工具的批量执行结果顺序一致
    ///
    /// 当前实现：join_all 全并发，unsafe 工具可能竞态。
    /// 修复后：unsafe 工具应串行执行，但结果顺序与输入一致。
    #[tokio::test]
    async fn test_execute_batch_mixed_safe_unsafe_order_preserved() {
        let safe = Arc::new(SafeCountTool {
            count: AtomicUsize::new(0),
        });
        let unsafe1 = Arc::new(UnsafeCountTool {
            count: AtomicUsize::new(0),
        });
        let unsafe2 = Arc::new(UnsafeCountTool {
            count: AtomicUsize::new(0),
        });

        let registry = ToolRegistry::new();
        registry.register(safe.clone());
        registry.register(unsafe1.clone());
        registry.register(unsafe2.clone());

        let executor = ToolExecutor::new(Arc::new(registry));

        let calls = vec![
            make_tool_call("b1", "safe_count"),
            make_tool_call("b2", "unsafe_count"),
            make_tool_call("b3", "safe_count"),
            make_tool_call("b4", "unsafe_count"),
        ];

        let results = executor.execute_batch(&calls).await;

        assert_eq!(results.len(), 4, "结果数量应与输入一致");
        assert_eq!(results[0].tool_call_id, "b1");
        assert_eq!(results[1].tool_call_id, "b2");
        assert_eq!(results[2].tool_call_id, "b3");
        assert_eq!(results[3].tool_call_id, "b4");

        assert!(!results[0].is_error);
        assert!(!results[1].is_error);
        assert!(!results[2].is_error);
        assert!(!results[3].is_error);
    }

    #[tokio::test]
    async fn test_execute_batch_duplicate_call_ids_use_original_index() {
        #[derive(Debug)]
        struct EchoArgTool;

        #[async_trait]
        impl Tool for EchoArgTool {
            fn name(&self) -> &str {
                "echo_arg"
            }
            fn description(&self) -> &str {
                "回显参数"
            }
            fn parameters_schema(&self) -> Value {
                json!({
                    "type": "object",
                    "properties": {"value": {"type": "string"}},
                    "required": ["value"]
                })
            }
            fn metadata(&self) -> orangecoding_tools::ToolMetadata {
                orangecoding_tools::ToolMetadata::read_only()
            }
            async fn execute(&self, params: Value) -> orangecoding_tools::ToolResult<String> {
                Ok(params["value"].as_str().unwrap_or_default().to_string())
            }
        }

        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoArgTool));
        let executor = ToolExecutor::new(Arc::new(registry));
        let calls = vec![
            ToolCall::new("dup", "echo_arg", json!({"value": "first"})),
            ToolCall::new("dup", "echo_arg", json!({"value": "second"})),
        ];

        let results = executor.execute_batch(&calls).await;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].content, "first");
        assert_eq!(results[1].content, "second");
    }

    /// 测试所有 unsafe 工具应串行执行（不并发）
    ///
    /// 使用带延迟的 unsafe 工具，如果并发执行则执行时间 ~200ms，
    /// 如果串行执行则 ~600ms。当前实现会并发。
    #[tokio::test]
    async fn test_execute_batch_unsafe_tools_run_sequentially() {
        #[derive(Debug)]
        struct SlowUnsafeTool;

        #[async_trait]
        impl Tool for SlowUnsafeTool {
            fn name(&self) -> &str {
                "slow_unsafe"
            }
            fn description(&self) -> &str {
                "慢速非安全工具"
            }
            fn parameters_schema(&self) -> Value {
                json!({"type": "object", "properties": {}})
            }
            fn metadata(&self) -> orangecoding_tools::ToolMetadata {
                orangecoding_tools::ToolMetadata::default()
            }
            async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok("done".to_string())
            }
        }

        let registry = ToolRegistry::new();
        registry.register(Arc::new(SlowUnsafeTool));

        let executor = ToolExecutor::new(Arc::new(registry));

        let calls = vec![
            make_tool_call("s1", "slow_unsafe"),
            make_tool_call("s2", "slow_unsafe"),
            make_tool_call("s3", "slow_unsafe"),
        ];

        let start = std::time::Instant::now();
        let results = executor.execute_batch(&calls).await;
        let elapsed = start.elapsed();

        assert_eq!(results.len(), 3);
        // 串行执行 3 个 100ms 任务应至少 250ms
        // 并发执行约 100-150ms
        assert!(
            elapsed >= Duration::from_millis(250),
            "unsafe 工具应串行执行，实际耗时 {:?}（并发执行约 100ms，串行应 >250ms）",
            elapsed
        );
    }

    // --- Pre-Check Gate 测试 ---

    /// Mock diff provider for testing
    struct MockDiffProvider {
        diff: String,
    }

    impl DiffProvider for MockDiffProvider {
        fn get_diff(&self, _working_dir: &PathBuf) -> String {
            self.diff.clone()
        }
    }

    #[test]
    fn test_is_git_commit_call_detects_commit() {
        assert!(is_git_commit_call(
            "bash",
            &json!({"command": "git commit -m 'feat: add feature'"})
        ));
        assert!(is_git_commit_call(
            "Bash",
            &json!({"command": "git add -A && git commit -m 'fix: bug'"})
        ));
        assert!(is_git_commit_call(
            "bash",
            &json!({"command": "GIT COMMIT -m 'uppercase'"})
        ));
    }

    #[test]
    fn test_is_git_commit_call_ignores_non_commit() {
        assert!(!is_git_commit_call(
            "bash",
            &json!({"command": "git status"})
        ));
        assert!(!is_git_commit_call(
            "bash",
            &json!({"command": "git log --oneline"})
        ));
        assert!(!is_git_commit_call("read", &json!({})));
        assert!(!is_git_commit_call(
            "bash",
            &json!({"command": "ls -la"})
        ));
    }

    #[tokio::test]
    async fn test_gate_disabled_when_no_config() {
        #[derive(Debug)]
        struct CommitTool;

        #[async_trait]
        impl Tool for CommitTool {
            fn name(&self) -> &str {
                "bash"
            }
            fn description(&self) -> &str {
                "bash tool"
            }
            fn parameters_schema(&self) -> Value {
                json!({"type": "object", "properties": {"command": {"type": "string"}}})
            }
            async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
                Ok("committed".to_string())
            }
        }

        let registry = ToolRegistry::new();
        registry.register(Arc::new(CommitTool));
        let executor = ToolExecutor::new(Arc::new(registry));
        let call = ToolCall::new("c1", "bash", json!({"command": "git commit -m 'test'"}));

        let result = executor.execute_tool_call(&call).await;
        assert!(!result.is_error, "无 gate_config 时不应拦截");
    }

    #[tokio::test]
    async fn test_gate_allows_unrelated_change() {
        #[derive(Debug)]
        struct CommitTool;

        #[async_trait]
        impl Tool for CommitTool {
            fn name(&self) -> &str {
                "bash"
            }
            fn description(&self) -> &str {
                "bash tool"
            }
            fn parameters_schema(&self) -> Value {
                json!({"type": "object", "properties": {"command": {"type": "string"}}})
            }
            async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
                Ok("committed".to_string())
            }
        }

        let diff = "\
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,2 +1,3 @@
+updated docs
";

        let registry = ToolRegistry::new();
        registry.register(Arc::new(CommitTool));
        let executor = ToolExecutor::new(Arc::new(registry))
            .with_gate_check(GateCheckConfig {
                working_directory: PathBuf::from("/tmp"),
            })
            .with_diff_provider(Arc::new(MockDiffProvider {
                diff: diff.to_string(),
            }));

        let call = ToolCall::new("c2", "bash", json!({"command": "git commit -m 'docs: update'"}));
        let result = executor.execute_tool_call(&call).await;
        assert!(!result.is_error, "无关文件变更应 Allow");
    }

    #[tokio::test]
    async fn test_gate_blocks_critical_auth_change() {
        #[derive(Debug)]
        struct CommitTool;

        #[async_trait]
        impl Tool for CommitTool {
            fn name(&self) -> &str {
                "bash"
            }
            fn description(&self) -> &str {
                "bash tool"
            }
            fn parameters_schema(&self) -> Value {
                json!({"type": "object", "properties": {"command": {"type": "string"}}})
            }
            async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
                Ok("should not reach".to_string())
            }
        }

        let diff = "\
diff --git a/crates/orangecoding-control-server/src/auth.rs b/crates/orangecoding-control-server/src/auth.rs
--- a/crates/orangecoding-control-server/src/auth.rs
+++ b/crates/orangecoding-control-server/src/auth.rs
@@ -10,3 +10,5 @@
+added line 1
+added line 2
";

        let registry = ToolRegistry::new();
        registry.register(Arc::new(CommitTool));
        let executor = ToolExecutor::new(Arc::new(registry))
            .with_gate_check(GateCheckConfig {
                working_directory: PathBuf::from("/tmp"),
            })
            .with_diff_provider(Arc::new(MockDiffProvider {
                diff: diff.to_string(),
            }));

        let call = ToolCall::new("c3", "bash", json!({"command": "git commit -m 'auth change'"}));
        let result = executor.execute_tool_call(&call).await;
        assert!(result.is_error, "Auth 文件变更应 Block");
        assert!(
            result.content.contains("BLOCKED"),
            "错误信息应包含 BLOCKED，实际: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn test_gate_warns_session_change() {
        #[derive(Debug)]
        struct CommitTool;

        #[async_trait]
        impl Tool for CommitTool {
            fn name(&self) -> &str {
                "bash"
            }
            fn description(&self) -> &str {
                "bash tool"
            }
            fn parameters_schema(&self) -> Value {
                json!({"type": "object", "properties": {"command": {"type": "string"}}})
            }
            async fn execute(&self, _params: Value) -> orangecoding_tools::ToolResult<String> {
                Ok("committed".to_string())
            }
        }

        let diff = "\
diff --git a/src/session.rs b/src/session.rs
--- a/src/session.rs
+++ b/src/session.rs
@@ -1,3 +1,4 @@
+new session logic
-old session logic
";

        let registry = ToolRegistry::new();
        registry.register(Arc::new(CommitTool));
        let executor = ToolExecutor::new(Arc::new(registry))
            .with_gate_check(GateCheckConfig {
                working_directory: PathBuf::from("/tmp"),
            })
            .with_diff_provider(Arc::new(MockDiffProvider {
                diff: diff.to_string(),
            }));

        let call =
            ToolCall::new("c4", "bash", json!({"command": "git commit -m 'session change'"}));
        let result = executor.execute_tool_call(&call).await;
        assert!(!result.is_error, "Session 文件变更应 Warn 但不阻止");
    }
}
