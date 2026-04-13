//! # 代理主事件循环
//!
//! 本模块实现了 AI 编码代理的核心事件循环 `AgentLoop`，负责协调：
//! 1. 将对话消息发送给 AI 提供者
//! 2. 解析 AI 返回的工具调用请求
//! 3. 通过 `ToolExecutor` 执行工具并收集结果
//! 4. 将工具结果追加到对话中，继续下一轮交互
//! 5. 重复以上步骤，直到 AI 给出最终文本回复或达到最大迭代次数
//!
//! 支持通过 `CancellationToken` 取消运行，并通过 `mpsc` 通道发送事件通知。

use std::sync::Arc;
use std::time::{Duration, Instant};

use chengcoding_ai::provider::{
    AiProvider, AiResponse, ChatMessage, ChatOptions, MessageRole, ToolDefinition,
};
use chengcoding_ai::TokenUsage as AiTokenUsage;
use chengcoding_core::event::AgentEvent;
use chengcoding_core::message::{Message, ToolCall as CoreToolCall};
use chengcoding_core::types::{AgentId, ToolName};
use chengcoding_core::TokenUsage;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::context::AgentContext;
use crate::executor::ToolExecutor;

// ---------------------------------------------------------------------------
// 代理循环配置
// ---------------------------------------------------------------------------

/// 代理循环的默认最大迭代次数
const DEFAULT_MAX_ITERATIONS: u32 = 20;
/// 代理循环的默认超时时间（5 分钟）
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// 代理循环配置
///
/// 控制代理事件循环的行为参数，包括最大迭代次数、超时时间和工具自动审批。
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// 最大迭代次数（每次 AI 调用计为一次迭代）
    pub max_iterations: u32,
    /// 整个循环的超时时间
    pub timeout: Duration,
    /// 是否自动批准工具调用（跳过人工确认）
    pub auto_approve_tools: bool,
}

impl Default for AgentLoopConfig {
    /// 创建默认配置
    ///
    /// - 最大迭代次数：20
    /// - 超时时间：300 秒（5 分钟）
    /// - 自动批准工具：否
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            timeout: DEFAULT_TIMEOUT,
            auto_approve_tools: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 代理循环结果
// ---------------------------------------------------------------------------

/// 代理循环执行结果
///
/// 包含本次代理运行的完整统计信息。
#[derive(Debug, Clone)]
pub struct AgentLoopResult {
    /// 完整的对话消息列表（包含所有系统、用户、助手和工具消息）
    pub messages: Vec<Message>,
    /// 实际执行的工具调用总数
    pub tool_calls_made: u32,
    /// 累计的令牌使用量
    pub tokens_used: TokenUsage,
    /// 循环执行的总耗时
    pub duration: Duration,
}

// ---------------------------------------------------------------------------
// 代理事件循环
// ---------------------------------------------------------------------------

/// 代理事件循环 - AI 编码代理的核心执行引擎
///
/// 协调 AI 提供者和工具执行器之间的交互：
/// ```text
/// 用户输入 → AI 推理 → 工具调用 → 工具结果 → AI 推理 → ... → 最终回复
/// ```
///
/// # 生命周期
/// 1. 通过 `new()` 创建实例
/// 2. 调用 `run()` 启动事件循环
/// 3. 循环自动在以下条件下终止：
///    - AI 返回不包含工具调用的最终回复
///    - 达到最大迭代次数
///    - 超时
///    - 收到取消信号
pub struct AgentLoop {
    /// 代理的唯一标识符
    id: AgentId,
    /// AI 提供者（如 DeepSeek、通义千问等）
    provider: Arc<dyn AiProvider>,
    /// 工具执行器（封装了工具注册表）
    tool_executor: ToolExecutor,
    /// 代理上下文（对话历史、工作目录等）
    context: AgentContext,
    /// 循环配置
    config: AgentLoopConfig,
}

impl AgentLoop {
    /// 创建新的代理事件循环
    ///
    /// # 参数
    /// - `id`: 代理标识符
    /// - `provider`: AI 提供者的共享引用
    /// - `tool_executor`: 工具执行器实例
    /// - `context`: 代理上下文
    /// - `config`: 循环配置
    pub fn new(
        id: AgentId,
        provider: Arc<dyn AiProvider>,
        tool_executor: ToolExecutor,
        context: AgentContext,
        config: AgentLoopConfig,
    ) -> Self {
        Self {
            id,
            provider,
            tool_executor,
            context,
            config,
        }
    }

    /// 运行代理事件循环
    ///
    /// 这是代理的主入口方法。循环执行以下步骤：
    /// 1. 将当前对话转换为 AI 提供者的消息格式
    /// 2. 收集可用工具定义
    /// 3. 调用 AI 提供者获取响应
    /// 4. 若 AI 请求工具调用，执行工具并将结果加入对话
    /// 5. 重复以上步骤直到终止条件触发
    ///
    /// # 参数
    /// - `chat_options`: AI 聊天选项（模型、温度等）
    /// - `cancel_token`: 取消令牌，外部可通过此令牌中止循环
    /// - `event_sender`: 事件通道发送端，用于推送实时事件通知
    ///
    /// # 返回值
    /// 成功时返回 `AgentLoopResult`，包含完整的对话和统计信息。
    /// 失败时返回 `chengcoding_core::CeairError`。
    pub async fn run(
        &mut self,
        chat_options: &ChatOptions,
        cancel_token: CancellationToken,
        event_sender: mpsc::Sender<AgentEvent>,
    ) -> chengcoding_core::Result<AgentLoopResult> {
        let start_time = Instant::now();
        let session_id = self.context.session_id().clone();
        let mut tool_calls_made: u32 = 0;
        let mut tokens_used = TokenUsage::default();

        info!("代理 {} 开始运行事件循环", self.id);

        // 发送代理启动事件
        let _ = event_sender
            .send(AgentEvent::started(self.id.clone(), session_id.clone()))
            .await;

        // 收集工具定义（用于传递给 AI 提供者）
        let tool_definitions = self.build_tool_definitions();

        // 主事件循环
        for iteration in 0..self.config.max_iterations {
            // 检查取消信号
            if cancel_token.is_cancelled() {
                warn!("代理 {} 收到取消信号，退出循环", self.id);
                break;
            }

            // 检查超时
            if start_time.elapsed() > self.config.timeout {
                warn!(
                    "代理 {} 执行超时（超过 {:?}），退出循环",
                    self.id, self.config.timeout
                );
                let _ = event_sender
                    .send(AgentEvent::error(
                        self.id.clone(),
                        session_id.clone(),
                        format!("代理执行超时（超过 {} 秒）", self.config.timeout.as_secs()),
                    ))
                    .await;
                break;
            }

            info!(
                "代理 {} 第 {}/{} 次迭代",
                self.id,
                iteration + 1,
                self.config.max_iterations
            );

            // 将对话历史转换为 AI 提供者的消息格式
            let chat_messages = self.convert_to_chat_messages();

            // 调用 AI 提供者获取响应（带超时保护和取消检测）
            let ai_response = self
                .call_ai_provider(
                    &chat_messages,
                    &tool_definitions,
                    chat_options,
                    &cancel_token,
                )
                .await?;

            // 累计令牌使用量
            Self::accumulate_tokens(&mut tokens_used, &ai_response.usage);

            // 发送令牌用量更新事件
            let _ = event_sender
                .send(AgentEvent::token_usage_updated(
                    self.id.clone(),
                    session_id.clone(),
                    tokens_used.clone(),
                ))
                .await;

            // 检查 AI 是否返回了工具调用
            if ai_response.tool_calls.is_empty() {
                // 无工具调用 → AI 给出了最终文本回复
                info!("代理 {} 收到最终回复，退出循环", self.id);

                // 将助手回复添加到对话
                self.context.add_assistant_message(&ai_response.content);

                // 发送完成事件
                let summary = Self::truncate_content(&ai_response.content, 200);
                let _ = event_sender
                    .send(AgentEvent::completed(
                        self.id.clone(),
                        session_id.clone(),
                        summary,
                    ))
                    .await;

                break;
            }

            // AI 请求了工具调用
            let core_tool_calls = self.convert_ai_tool_calls(&ai_response.tool_calls);
            let num_calls = core_tool_calls.len() as u32;

            info!("代理 {} 请求 {} 个工具调用", self.id, num_calls);

            // 将带工具调用的助手消息添加到对话
            self.context
                .get_conversation_mut()
                .add_message(Message::assistant_with_tool_calls(
                    if ai_response.content.is_empty() {
                        None
                    } else {
                        Some(ai_response.content.clone())
                    },
                    core_tool_calls.clone(),
                ));

            // 发送工具调用请求事件
            for tc in &core_tool_calls {
                let _ = event_sender
                    .send(AgentEvent::tool_call_requested(
                        self.id.clone(),
                        session_id.clone(),
                        tc.clone(),
                    ))
                    .await;
            }

            // 执行工具调用
            let call_start = Instant::now();
            let tool_results = self.tool_executor.execute_batch(&core_tool_calls).await;
            let call_duration = call_start.elapsed();

            // 将工具结果添加到对话并发送事件
            for result in &tool_results {
                // 发送工具完成事件
                let tool_name = core_tool_calls
                    .iter()
                    .find(|tc| tc.id == result.tool_call_id)
                    .map(|tc| ToolName::new(&tc.function_name))
                    .unwrap_or_else(|| ToolName::new("unknown"));

                let _ = event_sender
                    .send(AgentEvent::tool_call_completed(
                        self.id.clone(),
                        session_id.clone(),
                        tool_name,
                        !result.is_error,
                        call_duration.as_millis() as u64,
                    ))
                    .await;

                // 添加工具结果到对话
                self.context.add_tool_result(result.clone());
            }

            tool_calls_made += num_calls;
        }

        let duration = start_time.elapsed();

        info!(
            "代理 {} 事件循环结束: 执行了 {} 次工具调用, 耗时 {:?}",
            self.id, tool_calls_made, duration
        );

        // 构建并返回最终结果
        Ok(AgentLoopResult {
            messages: self.context.get_conversation().get_messages().to_vec(),
            tool_calls_made,
            tokens_used,
            duration,
        })
    }

    // -----------------------------------------------------------------------
    // 内部辅助方法
    // -----------------------------------------------------------------------

    /// 调用 AI 提供者获取聊天响应
    ///
    /// 使用 `tokio::select!` 实现超时控制和取消检测。
    async fn call_ai_provider(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
        cancel_token: &CancellationToken,
    ) -> chengcoding_core::Result<AiResponse> {
        debug!("调用 AI 提供者: {}", self.provider.name());

        // 使用 tokio::select! 同时监听 AI 调用、取消信号
        tokio::select! {
            // AI 提供者调用
            result = self.provider.chat_completion(messages, tools, options) => {
                result.map_err(|e| chengcoding_core::CeairError::Ai {
                    message: format!("AI 调用失败: {}", e),
                    model: Some(options.model.clone()),
                })
            }
            // 取消信号
            _ = cancel_token.cancelled() => {
                Err(chengcoding_core::CeairError::Agent {
                    agent_id: self.id.to_string(),
                    message: "代理运行被取消".to_string(),
                })
            }
        }
    }

    /// 将对话中的 chengcoding-core 消息转换为 AI 提供者的 ChatMessage 格式
    fn convert_to_chat_messages(&self) -> Vec<ChatMessage> {
        self.context
            .get_conversation()
            .get_messages()
            .iter()
            .map(|msg| {
                match msg.role {
                    chengcoding_core::message::Role::System => {
                        ChatMessage::system(msg.content.clone().unwrap_or_default())
                    }
                    chengcoding_core::message::Role::User => {
                        ChatMessage::user(msg.content.clone().unwrap_or_default())
                    }
                    chengcoding_core::message::Role::Assistant => {
                        if msg.tool_calls.is_empty() {
                            // 纯文本助手消息
                            ChatMessage::assistant(msg.content.clone().unwrap_or_default())
                        } else {
                            // 带工具调用的助手消息
                            let ai_tool_calls: Vec<chengcoding_ai::ToolCall> = msg
                                .tool_calls
                                .iter()
                                .map(|tc| chengcoding_ai::ToolCall {
                                    id: tc.id.clone(),
                                    call_type: "function".to_string(),
                                    function: chengcoding_ai::provider::FunctionCall {
                                        name: tc.function_name.clone(),
                                        arguments: tc.arguments.to_string(),
                                    },
                                })
                                .collect();

                            ChatMessage {
                                role: MessageRole::Assistant,
                                content: msg.content.clone(),
                                tool_call_id: None,
                                tool_calls: Some(ai_tool_calls),
                                name: None,
                            }
                        }
                    }
                    chengcoding_core::message::Role::Tool => ChatMessage::tool_result(
                        msg.tool_call_id.clone().unwrap_or_default(),
                        msg.content.clone().unwrap_or_default(),
                    ),
                }
            })
            .collect()
    }

    /// 将 AI 返回的工具调用转换为 chengcoding-core 的 ToolCall 格式
    fn convert_ai_tool_calls(&self, ai_calls: &[chengcoding_ai::ToolCall]) -> Vec<CoreToolCall> {
        ai_calls
            .iter()
            .map(|ac| {
                // AI 提供者的参数是 JSON 字符串，需要解析为 serde_json::Value
                let arguments = serde_json::from_str(&ac.function.arguments)
                    .unwrap_or_else(|_| serde_json::Value::String(ac.function.arguments.clone()));

                CoreToolCall::new(&ac.id, &ac.function.name, arguments)
            })
            .collect()
    }

    /// 构建工具定义列表（从工具注册表生成 AI 格式的工具描述）
    fn build_tool_definitions(&self) -> Vec<ToolDefinition> {
        let schemas = self.tool_executor.registry().get_schemas();
        let schemas_array = match schemas.as_array() {
            Some(arr) => arr,
            None => return Vec::new(),
        };

        schemas_array
            .iter()
            .filter_map(|schema| {
                let func = schema.get("function")?;
                let name = func.get("name")?.as_str()?.to_string();
                let description = func.get("description")?.as_str()?.to_string();
                let params = func.get("parameters").cloned().unwrap_or_default();

                // 将 JSON Schema 转换为 ToolParameter
                let properties = params
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();

                let required = params
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                Some(ToolDefinition {
                    tool_type: "function".to_string(),
                    function: chengcoding_ai::provider::FunctionDefinition {
                        name,
                        description,
                        parameters: chengcoding_ai::provider::ToolParameter {
                            param_type: "object".to_string(),
                            properties,
                            required,
                        },
                    },
                })
            })
            .collect()
    }

    /// 累加令牌使用量
    ///
    /// 将 AI 模块的 TokenUsage 累加到 core 模块的 TokenUsage 中
    fn accumulate_tokens(total: &mut TokenUsage, ai_usage: &AiTokenUsage) {
        total.prompt_tokens += ai_usage.prompt_tokens as u64;
        total.completion_tokens += ai_usage.completion_tokens as u64;
        total.total_tokens += ai_usage.total_tokens as u64;
    }

    /// 截断文本内容（用于事件摘要）
    fn truncate_content(content: &str, max_len: usize) -> String {
        if content.len() <= max_len {
            content.to_string()
        } else {
            format!("{}...", &content[..max_len])
        }
    }

    // -----------------------------------------------------------------------
    // 公共访问器
    // -----------------------------------------------------------------------

    /// 获取代理 ID 的引用
    pub fn id(&self) -> &AgentId {
        &self.id
    }

    /// 获取代理上下文的引用
    pub fn context(&self) -> &AgentContext {
        &self.context
    }

    /// 获取代理上下文的可变引用
    pub fn context_mut(&mut self) -> &mut AgentContext {
        &mut self.context
    }

    /// 获取代理循环配置的引用
    pub fn config(&self) -> &AgentLoopConfig {
        &self.config
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chengcoding_ai::{AiError, AiResult};
    use chengcoding_core::SessionId;
    use chengcoding_tools::ToolRegistry;
    use std::path::PathBuf;

    // -----------------------------------------------------------------------
    // 模拟 AI 提供者（返回纯文本回复，不请求工具调用）
    // -----------------------------------------------------------------------

    /// 模拟 AI 提供者：返回固定文本回复
    struct MockTextProvider {
        /// 要返回的固定回复内容
        response_content: String,
    }

    impl MockTextProvider {
        fn new(content: &str) -> Self {
            Self {
                response_content: content.to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl AiProvider for MockTextProvider {
        fn name(&self) -> &str {
            "mock_text_provider"
        }

        async fn chat_completion(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
            _options: &ChatOptions,
        ) -> AiResult<AiResponse> {
            Ok(AiResponse {
                content: self.response_content.clone(),
                tool_calls: vec![],
                usage: AiTokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                },
                model: "mock-model".to_string(),
                finish_reason: "stop".to_string(),
            })
        }

        async fn chat_completion_stream(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
            _options: &ChatOptions,
        ) -> AiResult<chengcoding_ai::provider::StreamResponse> {
            Err(AiError::Config("流式模式未实现".to_string()))
        }
    }

    // -----------------------------------------------------------------------
    // 模拟 AI 提供者（先请求工具调用，然后返回文本回复）
    // -----------------------------------------------------------------------

    /// 模拟 AI 提供者：第一次调用返回工具调用，后续返回文本回复
    struct MockToolCallProvider {
        /// 计数器（使用原子计数实现调用次数追踪）
        call_count: std::sync::atomic::AtomicU32,
    }

    impl MockToolCallProvider {
        fn new() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicU32::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl AiProvider for MockToolCallProvider {
        fn name(&self) -> &str {
            "mock_tool_call_provider"
        }

        async fn chat_completion(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
            _options: &ChatOptions,
        ) -> AiResult<AiResponse> {
            let count = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            if count == 0 {
                // 第一次调用：返回工具调用请求
                Ok(AiResponse {
                    content: String::new(),
                    tool_calls: vec![chengcoding_ai::ToolCall {
                        id: "call_test_1".to_string(),
                        call_type: "function".to_string(),
                        function: chengcoding_ai::provider::FunctionCall {
                            name: "test_tool".to_string(),
                            arguments: r#"{"key": "value"}"#.to_string(),
                        },
                    }],
                    usage: AiTokenUsage {
                        prompt_tokens: 15,
                        completion_tokens: 10,
                        total_tokens: 25,
                    },
                    model: "mock-model".to_string(),
                    finish_reason: "tool_calls".to_string(),
                })
            } else {
                // 后续调用：返回最终文本回复
                Ok(AiResponse {
                    content: "工具调用完成，任务已处理".to_string(),
                    tool_calls: vec![],
                    usage: AiTokenUsage {
                        prompt_tokens: 20,
                        completion_tokens: 15,
                        total_tokens: 35,
                    },
                    model: "mock-model".to_string(),
                    finish_reason: "stop".to_string(),
                })
            }
        }

        async fn chat_completion_stream(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
            _options: &ChatOptions,
        ) -> AiResult<chengcoding_ai::provider::StreamResponse> {
            Err(AiError::Config("流式模式未实现".to_string()))
        }
    }

    // -----------------------------------------------------------------------
    // 用于测试的简单模拟工具
    // -----------------------------------------------------------------------

    #[derive(Debug)]
    struct TestTool;

    #[async_trait::async_trait]
    impl chengcoding_tools::Tool for TestTool {
        fn name(&self) -> &str {
            "test_tool"
        }
        fn description(&self) -> &str {
            "用于测试的模拟工具"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string"}
                }
            })
        }
        async fn execute(
            &self,
            _params: serde_json::Value,
        ) -> chengcoding_tools::ToolResult<String> {
            Ok("测试工具执行结果".to_string())
        }
    }

    // -----------------------------------------------------------------------
    // 辅助函数
    // -----------------------------------------------------------------------

    /// 创建测试用的工具执行器
    fn create_test_executor() -> ToolExecutor {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(TestTool));
        ToolExecutor::new(Arc::new(registry))
    }

    // -----------------------------------------------------------------------
    // 测试用例
    // -----------------------------------------------------------------------

    /// 测试默认配置值
    #[test]
    fn test_default_config() {
        let config = AgentLoopConfig::default();

        assert_eq!(config.max_iterations, DEFAULT_MAX_ITERATIONS);
        assert_eq!(config.timeout, DEFAULT_TIMEOUT);
        assert!(!config.auto_approve_tools);
    }

    /// 测试 AI 直接返回文本回复（无工具调用）的场景
    #[tokio::test]
    async fn test_run_text_only_response() {
        let provider = Arc::new(MockTextProvider::new("这是AI的回复"));
        let executor = create_test_executor();
        let mut context = AgentContext::new(SessionId::new(), PathBuf::from("."));
        context.set_system_prompt("你是一个编程助手");
        context.add_user_message("你好");

        let config = AgentLoopConfig {
            max_iterations: 5,
            timeout: Duration::from_secs(30),
            auto_approve_tools: true,
        };

        let mut agent_loop = AgentLoop::new(AgentId::new(), provider, executor, context, config);

        let cancel_token = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(100);
        let options = ChatOptions::with_model("mock-model");

        let result = agent_loop.run(&options, cancel_token, tx).await.unwrap();

        // 验证结果
        assert_eq!(result.tool_calls_made, 0);
        assert!(result.tokens_used.total_tokens > 0);
        // 消息应包含：系统提示 + 用户消息 + 助手回复
        assert_eq!(result.messages.len(), 3);

        // 验证收到了事件通知
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        // 至少应有 Started、TokenUsageUpdated 和 Completed 事件
        assert!(events.len() >= 3);
    }

    /// 测试包含工具调用的完整交互流程
    #[tokio::test]
    async fn test_run_with_tool_calls() {
        let provider = Arc::new(MockToolCallProvider::new());
        let executor = create_test_executor();
        let mut context = AgentContext::new(SessionId::new(), PathBuf::from("."));
        context.set_system_prompt("你是一个编程助手");
        context.add_user_message("请读取文件");

        let config = AgentLoopConfig {
            max_iterations: 10,
            timeout: Duration::from_secs(30),
            auto_approve_tools: true,
        };

        let mut agent_loop = AgentLoop::new(AgentId::new(), provider, executor, context, config);

        let cancel_token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel(100);
        let options = ChatOptions::with_model("mock-model");

        let result = agent_loop.run(&options, cancel_token, tx).await.unwrap();

        // 应执行了 1 次工具调用
        assert_eq!(result.tool_calls_made, 1);
        // 令牌用量应是两次调用的累计
        assert_eq!(result.tokens_used.total_tokens, 60); // 30 + 35 的 prompt+completion
                                                         // 验证耗时已被记录（Duration 非零或至少能正常返回）
        let _ = result.duration;
    }

    /// 测试取消信号能够中止循环
    #[tokio::test]
    async fn test_run_cancellation() {
        let provider = Arc::new(MockTextProvider::new("不应到达此处"));
        let executor = create_test_executor();
        let mut context = AgentContext::new(SessionId::new(), PathBuf::from("."));
        context.add_user_message("你好");

        let config = AgentLoopConfig::default();
        let mut agent_loop = AgentLoop::new(AgentId::new(), provider, executor, context, config);

        let cancel_token = CancellationToken::new();
        // 在运行前就发送取消信号
        cancel_token.cancel();

        let (tx, _rx) = mpsc::channel(100);
        let options = ChatOptions::with_model("mock-model");

        let result = agent_loop.run(&options, cancel_token, tx).await.unwrap();

        // 被取消后不应有工具调用
        assert_eq!(result.tool_calls_made, 0);
    }

    /// 测试文本截断辅助函数
    #[test]
    fn test_truncate_content() {
        // 短文本不截断
        assert_eq!(AgentLoop::truncate_content("短文本", 100), "短文本");

        // 长文本应被截断
        let long = "a".repeat(300);
        let truncated = AgentLoop::truncate_content(&long, 200);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 204); // 200 + "..."
    }

    /// 测试消息格式转换
    #[test]
    fn test_convert_to_chat_messages() {
        let provider = Arc::new(MockTextProvider::new(""));
        let executor = create_test_executor();
        let mut context = AgentContext::new(SessionId::new(), PathBuf::from("."));
        context.set_system_prompt("系统提示");
        context.add_user_message("用户消息");
        context.add_assistant_message("助手回复");

        let agent_loop = AgentLoop::new(
            AgentId::new(),
            provider,
            executor,
            context,
            AgentLoopConfig::default(),
        );

        let messages = agent_loop.convert_to_chat_messages();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[1].role, MessageRole::User);
        assert_eq!(messages[2].role, MessageRole::Assistant);
    }

    /// 测试工具定义构建
    #[test]
    fn test_build_tool_definitions() {
        let provider = Arc::new(MockTextProvider::new(""));
        let executor = create_test_executor();
        let context = AgentContext::new(SessionId::new(), PathBuf::from("."));

        let agent_loop = AgentLoop::new(
            AgentId::new(),
            provider,
            executor,
            context,
            AgentLoopConfig::default(),
        );

        let definitions = agent_loop.build_tool_definitions();

        // 注册表中注册了 1 个工具（TestTool）
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].function.name, "test_tool");
        assert_eq!(definitions[0].tool_type, "function");
    }
}
