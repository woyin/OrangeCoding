//! Anthropic Messages API 适配器
//!
//! 支持 Claude 系列模型：
//! - Claude Sonnet 4
//! - Claude Opus 4
//! - Claude Haiku
//!
//! Anthropic API 格式与 OpenAI 不同，主要区别：
//! - 系统消息是顶层字段而非消息数组中的元素
//! - 响应内容是内容块数组而非字符串
//! - 工具定义使用 `input_schema` 而非 `parameters`
//! - 用量字段名为 `input_tokens`/`output_tokens`

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

use crate::provider::{
    AiProvider, AiResponse, ChatMessage, ChatOptions, FunctionCall, MessageRole, ProviderConfig,
    StreamEvent, StreamResponse, TokenUsage, ToolCall, ToolDefinition,
};
use crate::stream::SseParser;
use crate::{AiError, AiResult};

/// Anthropic 默认 API 基础地址
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// Anthropic 默认模型
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// Anthropic API 版本头
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Messages API 适配器
///
/// 支持 Claude 系列模型，使用 Anthropic 专有 API 格式。
pub struct AnthropicProvider {
    /// HTTP 客户端
    client: Client,
    /// API 密钥
    api_key: String,
    /// API 基础地址
    base_url: String,
    /// 默认模型名称
    default_model: String,
}

impl AnthropicProvider {
    /// 创建新的 Anthropic 提供者实例
    pub fn new(config: ProviderConfig) -> AiResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| AiError::Config(format!("无法创建 HTTP 客户端: {}", e)))?;

        Ok(Self {
            client,
            api_key: config.api_key,
            base_url: config
                .base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            default_model: config
                .default_model
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        })
    }

    /// 获取 Messages API 完整 URL
    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }

    /// 确定实际使用的模型名称
    fn resolve_model(&self, options: &ChatOptions) -> String {
        if options.model.is_empty() {
            self.default_model.clone()
        } else {
            options.model.clone()
        }
    }

    /// 从消息列表中提取系统消息并分离非系统消息
    ///
    /// Anthropic API 要求系统消息作为顶层字段，不能放在 messages 数组中。
    fn extract_system_and_messages(
        messages: &[ChatMessage],
    ) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system_prompt = None;
        let mut api_messages = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // 将系统消息提取为顶层字段（多个系统消息拼接）
                    if let Some(ref content) = msg.content {
                        match &mut system_prompt {
                            Some(existing) => {
                                let s: &mut String = existing;
                                s.push('\n');
                                s.push_str(content);
                            }
                            None => {
                                system_prompt = Some(content.clone());
                            }
                        }
                    }
                }
                MessageRole::Tool => {
                    // 工具结果使用 Anthropic 的 tool_result 内容块格式
                    let content_block = json!({
                        "type": "tool_result",
                        "tool_use_id": msg.tool_call_id.as_deref().unwrap_or(""),
                        "content": msg.content.as_deref().unwrap_or(""),
                    });
                    api_messages.push(json!({
                        "role": "user",
                        "content": [content_block],
                    }));
                }
                _ => {
                    // 普通用户/助手消息
                    let role = match msg.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        _ => unreachable!(),
                    };

                    let mut api_msg = json!({ "role": role });

                    if let Some(ref tool_calls) = msg.tool_calls {
                        // 助手消息中包含工具调用时，转换为 Anthropic 的 tool_use 内容块
                        let mut content_blocks: Vec<serde_json::Value> = Vec::new();
                        if let Some(ref text) = msg.content {
                            if !text.is_empty() {
                                content_blocks.push(json!({"type": "text", "text": text}));
                            }
                        }
                        for tc in tool_calls {
                            let input: serde_json::Value =
                                serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.function.name,
                                "input": input,
                            }));
                        }
                        api_msg["content"] = json!(content_blocks);
                    } else {
                        api_msg["content"] = json!(msg.content.as_deref().unwrap_or(""));
                    }

                    api_messages.push(api_msg);
                }
            }
        }

        (system_prompt, api_messages)
    }

    /// 将 OpenAI 格式的工具定义转换为 Anthropic 格式
    ///
    /// 主要差异：Anthropic 使用 `input_schema` 而非 `parameters`
    fn convert_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.function.name,
                    "description": tool.function.description,
                    "input_schema": {
                        "type": tool.function.parameters.param_type,
                        "properties": tool.function.parameters.properties,
                        "required": tool.function.parameters.required,
                    },
                })
            })
            .collect()
    }

    /// 构建 API 请求体
    fn build_request_body(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
        stream: bool,
    ) -> serde_json::Value {
        let model = self.resolve_model(options);
        let (system_prompt, api_messages) = Self::extract_system_and_messages(messages);

        let mut body = json!({
            "model": model,
            "messages": api_messages,
            "max_tokens": options.max_tokens.unwrap_or(4096),
        });

        // 设置系统提示词（Anthropic 的顶层字段）
        if let Some(system) = system_prompt {
            body["system"] = json!(system);
        }

        if let Some(temp) = options.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = options.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(ref stop) = options.stop_sequences {
            if !stop.is_empty() {
                body["stop_sequences"] = json!(stop);
            }
        }

        // 添加 Anthropic 格式的工具定义
        if !tools.is_empty() {
            body["tools"] = json!(Self::convert_tools(tools));
        }

        if stream {
            body["stream"] = json!(true);
        }

        body
    }

    /// 解析 Anthropic 响应中的内容块数组
    fn parse_content_blocks(content: &[serde_json::Value]) -> (String, Vec<ToolCall>) {
        let mut text = String::new();
        let mut tool_calls = Vec::new();

        for block in content {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        text.push_str(t);
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block
                        .get("input")
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "{}".to_string());

                    tool_calls.push(ToolCall {
                        id,
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name,
                            arguments: input,
                        },
                    });
                }
                _ => {}
            }
        }

        (text, tool_calls)
    }

    /// 解析非流式 API 响应
    fn parse_response(&self, response_json: &serde_json::Value) -> AiResult<AiResponse> {
        let content_blocks = response_json
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| AiError::Parse("响应中缺少 content 字段".to_string()))?;

        let (text, tool_calls) = Self::parse_content_blocks(content_blocks);

        let stop_reason = response_json
            .get("stop_reason")
            .and_then(|s| s.as_str())
            .unwrap_or("end_turn")
            .to_string();

        // 映射 Anthropic 用量字段名到统一格式
        let usage = response_json
            .get("usage")
            .map(|u| {
                let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let output = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                TokenUsage {
                    prompt_tokens: input,
                    completion_tokens: output,
                    total_tokens: input + output,
                }
            })
            .unwrap_or_default();

        let model = response_json
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        // 将 Anthropic 的 stop_reason 映射到 OpenAI 兼容格式
        let finish_reason = match stop_reason.as_str() {
            "end_turn" => "stop".to_string(),
            "tool_use" => "tool_calls".to_string(),
            "max_tokens" => "length".to_string(),
            other => other.to_string(),
        };

        Ok(AiResponse {
            content: text,
            tool_calls,
            usage,
            model,
            finish_reason,
        })
    }

    /// 解析 Anthropic 流式 SSE 事件
    fn parse_anthropic_stream_event(data: &str) -> AiResult<StreamEvent> {
        let trimmed = data.trim();

        let json: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
            AiError::Parse(format!(
                "Anthropic SSE 数据解析失败: {} (原始: {})",
                e, trimmed
            ))
        })?;

        let event_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "content_block_delta" => {
                let empty = json!({});
                let delta = json.get("delta").unwrap_or(&empty);
                let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match delta_type {
                    "text_delta" => {
                        let text = delta
                            .get("text")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();
                        Ok(StreamEvent::ContentDelta(text))
                    }
                    "input_json_delta" => {
                        // 工具调用参数增量
                        let partial_json = delta
                            .get("partial_json")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();
                        Ok(StreamEvent::ToolCallDelta {
                            id: String::new(),
                            name: String::new(),
                            arguments: partial_json,
                        })
                    }
                    _ => Ok(StreamEvent::ContentDelta(String::new())),
                }
            }
            "content_block_start" => {
                // 工具调用开始块，包含 id 和 name
                let empty = json!({});
                let content_block = json.get("content_block").unwrap_or(&empty);
                let block_type = content_block
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                if block_type == "tool_use" {
                    let id = content_block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = content_block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    Ok(StreamEvent::ToolCallDelta {
                        id,
                        name,
                        arguments: String::new(),
                    })
                } else {
                    Ok(StreamEvent::ContentDelta(String::new()))
                }
            }
            "message_delta" => {
                // 消息结束事件，包含用量信息
                if let Some(usage) = json.get("usage") {
                    let output = usage
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    return Ok(StreamEvent::Usage(TokenUsage {
                        prompt_tokens: 0,
                        completion_tokens: output,
                        total_tokens: output,
                    }));
                }
                Ok(StreamEvent::ContentDelta(String::new()))
            }
            "message_start" => {
                // 消息开始事件，提取输入令牌用量
                if let Some(message) = json.get("message") {
                    if let Some(usage) = message.get("usage") {
                        let input = usage
                            .get("input_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u32;
                        return Ok(StreamEvent::Usage(TokenUsage {
                            prompt_tokens: input,
                            completion_tokens: 0,
                            total_tokens: input,
                        }));
                    }
                }
                Ok(StreamEvent::ContentDelta(String::new()))
            }
            "message_stop" => Ok(StreamEvent::Done),
            _ => Ok(StreamEvent::ContentDelta(String::new())),
        }
    }

    /// 统一处理 API 错误响应
    fn handle_error_response(status_code: u16, body: &str) -> AiError {
        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| body.to_string());

        match status_code {
            401 => AiError::Auth(format!("Anthropic API 密钥无效: {}", message)),
            429 => AiError::RateLimit {
                retry_after_secs: 30,
            },
            529 => AiError::Api {
                status_code,
                message: format!("Anthropic API 过载: {}", message),
            },
            500..=599 => AiError::Api {
                status_code,
                message: format!("服务器错误: {}", message),
            },
            _ => AiError::Api {
                status_code,
                message,
            },
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    /// 返回提供者名称
    fn name(&self) -> &str {
        "anthropic"
    }

    /// 发送非流式聊天补全请求
    async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<AiResponse> {
        let body = self.build_request_body(messages, tools, options, false);
        let url = self.messages_url();

        tracing::debug!(
            provider = "anthropic",
            model = %self.resolve_model(options),
            "发送聊天补全请求"
        );

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(Self::handle_error_response(status.as_u16(), &error_body));
        }

        let response_json: serde_json::Value = response.json().await?;
        self.parse_response(&response_json)
    }

    /// 发送流式聊天补全请求
    async fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<StreamResponse> {
        let body = self.build_request_body(messages, tools, options, true);
        let url = self.messages_url();

        tracing::debug!(
            provider = "anthropic",
            model = %self.resolve_model(options),
            "发送流式聊天补全请求"
        );

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(Self::handle_error_response(status.as_u16(), &error_body));
        }

        // 将字节流转换为 Anthropic SSE 事件流
        let byte_stream = response.bytes_stream();
        let mut sse_parser = SseParser::new();

        let stream = byte_stream
            .map(move |chunk_result| -> Vec<AiResult<StreamEvent>> {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let events = sse_parser.feed(&text);

                        events
                            .into_iter()
                            .map(|sse_event| Self::parse_anthropic_stream_event(&sse_event.data))
                            .collect()
                    }
                    Err(e) => {
                        vec![Err(AiError::Stream(format!("读取流数据失败: {}", e)))]
                    }
                }
            })
            .flat_map(futures::stream::iter);

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{FunctionDefinition, ProviderConfig, ToolParameter};
    use std::collections::HashMap;

    /// 创建测试用的提供者配置
    fn test_config() -> ProviderConfig {
        ProviderConfig {
            api_key: "test-api-key".to_string(),
            api_secret: None,
            base_url: None,
            default_model: None,
            timeout_secs: 30,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_provider_name() {
        // 验证提供者名称
        let provider = AnthropicProvider::new(test_config()).unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_default_config() {
        // 验证默认配置值
        let provider = AnthropicProvider::new(test_config()).unwrap();
        assert_eq!(provider.base_url, "https://api.anthropic.com");
        assert_eq!(provider.default_model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_build_request_body_basic() {
        // 验证基本请求体构建
        let provider = AnthropicProvider::new(test_config()).unwrap();
        let messages = vec![ChatMessage::user("你好")];
        let options = ChatOptions::with_model("claude-sonnet-4-20250514")
            .temperature(0.7)
            .max_tokens(2048);

        let body = provider.build_request_body(&messages, &[], &options, false);

        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["max_tokens"], 2048);
        assert_eq!(body["temperature"], 0.7);
        // 无系统消息时不应包含 system 字段
        assert!(body.get("system").is_none());
        // 无工具时不应包含 tools 字段
        assert!(body.get("tools").is_none());
        // 非流式请求不应包含 stream 字段
        assert!(body.get("stream").is_none());
    }

    #[test]
    fn test_build_request_body_with_system_prompt() {
        // 验证系统消息被提取为顶层字段
        let provider = AnthropicProvider::new(test_config()).unwrap();
        let messages = vec![
            ChatMessage::system("你是一个编程助手"),
            ChatMessage::user("你好"),
        ];
        let options = ChatOptions::with_model("claude-sonnet-4-20250514");

        let body = provider.build_request_body(&messages, &[], &options, false);

        // 系统消息应在顶层
        assert_eq!(body["system"], "你是一个编程助手");
        // messages 数组中不应包含系统消息
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
    }

    #[test]
    fn test_build_request_body_with_tools() {
        // 验证工具定义转换为 Anthropic 格式
        let provider = AnthropicProvider::new(test_config()).unwrap();
        let messages = vec![ChatMessage::user("北京天气如何？")];
        let tools = vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "get_weather".to_string(),
                description: "获取指定城市的天气".to_string(),
                parameters: ToolParameter {
                    param_type: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert(
                            "city".to_string(),
                            serde_json::json!({"type": "string", "description": "城市名称"}),
                        );
                        props
                    },
                    required: vec!["city".to_string()],
                },
            },
        }];
        let options = ChatOptions::with_model("claude-sonnet-4-20250514");

        let body = provider.build_request_body(&messages, &tools, &options, false);

        let tools_arr = body["tools"].as_array().unwrap();
        assert_eq!(tools_arr.len(), 1);
        // Anthropic 使用 input_schema 而非 parameters
        assert!(tools_arr[0].get("input_schema").is_some());
        assert_eq!(tools_arr[0]["name"], "get_weather");
        assert_eq!(tools_arr[0]["description"], "获取指定城市的天气");
    }

    #[test]
    fn test_convert_messages() {
        // 验证消息转换：系统消息提取、工具结果格式转换
        let messages = vec![
            ChatMessage::system("你是助手"),
            ChatMessage::user("你好"),
            ChatMessage::assistant("你好！"),
            ChatMessage::tool_result("call_123", r#"{"result": "success"}"#),
        ];

        let (system, api_msgs) = AnthropicProvider::extract_system_and_messages(&messages);

        // 系统消息被提取
        assert_eq!(system.unwrap(), "你是助手");

        // 非系统消息转换正确
        assert_eq!(api_msgs.len(), 3);
        assert_eq!(api_msgs[0]["role"], "user");
        assert_eq!(api_msgs[1]["role"], "assistant");
        // 工具结果转换为 user 角色 + tool_result 内容块
        assert_eq!(api_msgs[2]["role"], "user");
        let content = api_msgs[2]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_result");
        assert_eq!(content[0]["tool_use_id"], "call_123");
    }

    #[test]
    fn test_parse_response_text() {
        // 验证文本响应解析
        let provider = AnthropicProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "id": "msg_abc123",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "你好！有什么可以帮你的？"}
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 20
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.content, "你好！有什么可以帮你的？");
        assert_eq!(response.model, "claude-sonnet-4-20250514");
        // end_turn 映射为 stop
        assert_eq!(response.finish_reason, "stop");
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn test_parse_response_with_tool_use() {
        // 验证带工具调用的响应解析
        let provider = AnthropicProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "id": "msg_456",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "让我查一下天气。"},
                {
                    "type": "tool_use",
                    "id": "toolu_abc",
                    "name": "get_weather",
                    "input": {"city": "北京"}
                }
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 15,
                "output_tokens": 25
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.content, "让我查一下天气。");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id, "toolu_abc");
        assert_eq!(response.tool_calls[0].function.name, "get_weather");
        // tool_use 映射为 tool_calls
        assert_eq!(response.finish_reason, "tool_calls");
    }

    #[test]
    fn test_parse_content_blocks() {
        // 验证内容块数组解析
        let blocks = vec![
            serde_json::json!({"type": "text", "text": "第一段"}),
            serde_json::json!({"type": "text", "text": "第二段"}),
            serde_json::json!({
                "type": "tool_use",
                "id": "toolu_1",
                "name": "search",
                "input": {"query": "test"}
            }),
        ];

        let (text, tool_calls) = AnthropicProvider::parse_content_blocks(&blocks);
        assert_eq!(text, "第一段第二段");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "search");
    }

    #[test]
    fn test_handle_error_auth() {
        // 验证 401 认证错误处理
        let err = AnthropicProvider::handle_error_response(
            401,
            r#"{"error": {"message": "Invalid API key"}}"#,
        );
        assert!(matches!(err, AiError::Auth(_)));
        assert!(err.to_string().contains("Anthropic"));
    }

    #[test]
    fn test_handle_error_rate_limit() {
        // 验证 429 频率限制错误处理
        let err = AnthropicProvider::handle_error_response(
            429,
            r#"{"error": {"message": "Rate limit exceeded"}}"#,
        );
        assert!(matches!(err, AiError::RateLimit { .. }));
    }

    #[test]
    fn test_handle_error_overloaded() {
        // 验证 529 过载错误处理
        let err = AnthropicProvider::handle_error_response(
            529,
            r#"{"error": {"message": "API is overloaded"}}"#,
        );
        assert!(matches!(
            err,
            AiError::Api {
                status_code: 529,
                ..
            }
        ));
        assert!(err.to_string().contains("过载"));
    }

    #[test]
    fn test_parse_stream_content_delta() {
        // 验证 Anthropic 流式文本增量事件
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"你好"}}"#;
        let event = AnthropicProvider::parse_anthropic_stream_event(data).unwrap();
        assert!(matches!(event, StreamEvent::ContentDelta(ref s) if s == "你好"));
    }

    #[test]
    fn test_parse_stream_tool_use() {
        // 验证 Anthropic 流式工具调用开始事件
        let data = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_abc","name":"get_weather"}}"#;
        let event = AnthropicProvider::parse_anthropic_stream_event(data).unwrap();
        match event {
            StreamEvent::ToolCallDelta { id, name, .. } => {
                assert_eq!(id, "toolu_abc");
                assert_eq!(name, "get_weather");
            }
            _ => panic!("期望 ToolCallDelta 事件"),
        }
    }

    #[test]
    fn test_parse_stream_done() {
        // 验证 Anthropic 流结束事件
        let data = r#"{"type":"message_stop"}"#;
        let event = AnthropicProvider::parse_anthropic_stream_event(data).unwrap();
        assert!(matches!(event, StreamEvent::Done));
    }

    #[test]
    fn test_usage_field_mapping() {
        // 验证 Anthropic 用量字段名映射到统一格式
        let provider = AnthropicProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "id": "msg_789",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "OK"}],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        // input_tokens → prompt_tokens
        assert_eq!(response.usage.prompt_tokens, 100);
        // output_tokens → completion_tokens
        assert_eq!(response.usage.completion_tokens, 50);
        // total = input + output
        assert_eq!(response.usage.total_tokens, 150);
    }
}
