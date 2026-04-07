//! # 工具执行器
//!
//! 本模块提供 `ToolExecutor`，负责接收 AI 模型返回的工具调用请求，
//! 通过 `ToolRegistry` 查找并执行对应的工具，支持：
//! - 单个工具调用执行
//! - 批量并行执行多个工具调用
//! - 单个工具执行的超时控制
//! - 执行错误的安全包装

use std::sync::Arc;
use std::time::Duration;

use ceair_core::message::{ToolCall, ToolResult};
use ceair_tools::ToolRegistry;
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// 默认常量
// ---------------------------------------------------------------------------

/// 单个工具执行的默认超时时间（30 秒）
const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

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
#[derive(Debug, Clone)]
pub struct ToolExecutor {
    /// 工具注册表，存储所有可用工具
    registry: Arc<ToolRegistry>,
    /// 单个工具执行的超时时间
    timeout: Duration,
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
                error!("工具 {} 执行失败: {} (ID: {})", tool_name, tool_err, tool_call_id);
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

        // 构建异步任务列表并并行执行
        let futures: Vec<_> = tool_calls
            .iter()
            .map(|tc| self.execute_tool_call(tc))
            .collect();

        let results = futures::future::join_all(futures).await;

        // 统计成功与失败数量
        let success_count = results.iter().filter(|r| !r.is_error).count();
        let error_count = results.len() - success_count;

        info!(
            "批量执行完成: {} 个成功, {} 个失败",
            success_count, error_count
        );

        results
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

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ceair_tools::{Tool, ToolError};
    use serde_json::{json, Value};
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
        async fn execute(&self, _params: Value) -> ceair_tools::ToolResult<String> {
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
        async fn execute(&self, _params: Value) -> ceair_tools::ToolResult<String> {
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
        async fn execute(&self, _params: Value) -> ceair_tools::ToolResult<String> {
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
}
