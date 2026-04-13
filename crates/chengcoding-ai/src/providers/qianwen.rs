//! 通义千问（Qianwen）AI 提供者适配器
//!
//! 通义千问使用阿里云 DashScope API，其请求/响应格式与 OpenAI 不同，
//! 需要特殊的消息包装和响应解析。
//!
//! 默认 API 端点: `https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation`

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::provider::{
    AiProvider, AiResponse, ChatMessage, ChatOptions, FunctionCall, MessageRole, ProviderConfig,
    StreamEvent, StreamResponse, TokenUsage, ToolCall, ToolDefinition,
};
use crate::stream::SseParser;
use crate::{AiError, AiResult};

/// 通义千问默认 API 端点
const DEFAULT_BASE_URL: &str =
    "https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation";

/// 通义千问默认模型
const DEFAULT_MODEL: &str = "qwen-turbo";

/// 通义千问 AI 提供者
///
/// 适配阿里云 DashScope API，支持通义千问系列模型。
/// 其 API 格式与 OpenAI 标准不同，需要进行格式转换。
pub struct QianwenProvider {
    /// HTTP 客户端
    client: Client,
    /// API 密钥（DashScope API Key）
    api_key: String,
    /// API 端点地址
    base_url: String,
    /// 默认模型名称
    default_model: String,
}

impl QianwenProvider {
    /// 创建新的通义千问提供者实例
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

    /// 确定实际使用的模型名称
    fn resolve_model(&self, options: &ChatOptions) -> String {
        if options.model.is_empty() {
            self.default_model.clone()
        } else {
            options.model.clone()
        }
    }

    /// 构建通义千问专用请求体
    ///
    /// DashScope API 使用不同于 OpenAI 的请求格式：
    /// ```json
    /// {
    ///     "model": "qwen-turbo",
    ///     "input": { "messages": [...] },
    ///     "parameters": { ... }
    /// }
    /// ```
    fn build_request_body(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> serde_json::Value {
        let model = self.resolve_model(options);

        // 将消息转换为通义千问格式
        let qw_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| Self::convert_message(msg))
            .collect();

        // 构建参数部分
        let mut parameters = serde_json::json!({});

        if let Some(temp) = options.temperature {
            parameters["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = options.max_tokens {
            parameters["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = options.top_p {
            parameters["top_p"] = serde_json::json!(top_p);
        }
        if let Some(ref stop) = options.stop_sequences {
            if !stop.is_empty() {
                parameters["stop"] = serde_json::json!(stop);
            }
        }

        // 启用增量流式输出
        parameters["incremental_output"] = serde_json::json!(true);

        let mut body = serde_json::json!({
            "model": model,
            "input": {
                "messages": qw_messages
            },
            "parameters": parameters
        });

        // 添加工具定义（如果有）
        if !tools.is_empty() {
            // 通义千问的工具格式与 OpenAI 类似
            body["input"]["tools"] = serde_json::json!(tools);
        }

        body
    }

    /// 将通用消息格式转换为通义千问格式
    fn convert_message(msg: &ChatMessage) -> serde_json::Value {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        let mut message = serde_json::json!({
            "role": role,
        });

        // 设置内容
        if let Some(ref content) = msg.content {
            message["content"] = serde_json::json!(content);
        }

        // 设置工具调用 ID
        if let Some(ref tool_call_id) = msg.tool_call_id {
            message["tool_call_id"] = serde_json::json!(tool_call_id);
        }

        // 设置工具调用列表
        if let Some(ref tool_calls) = msg.tool_calls {
            message["tool_calls"] = serde_json::json!(tool_calls);
        }

        // 设置名称标识
        if let Some(ref name) = msg.name {
            message["name"] = serde_json::json!(name);
        }

        message
    }

    /// 解析通义千问非流式响应
    ///
    /// DashScope 响应格式：
    /// ```json
    /// {
    ///     "output": {
    ///         "text": "...",
    ///         "finish_reason": "stop",
    ///         "choices": [...]
    ///     },
    ///     "usage": {
    ///         "input_tokens": 10,
    ///         "output_tokens": 20,
    ///         "total_tokens": 30
    ///     }
    /// }
    /// ```
    fn parse_response(&self, response_json: &serde_json::Value) -> AiResult<AiResponse> {
        let output = response_json
            .get("output")
            .ok_or_else(|| AiError::Parse("响应中缺少 output 字段".to_string()))?;

        // 通义千问可能在 output.text 或 output.choices[0].message.content 中返回内容
        let content = if let Some(text) = output.get("text").and_then(|t| t.as_str()) {
            text.to_string()
        } else if let Some(choices) = output.get("choices").and_then(|c| c.as_array()) {
            choices
                .first()
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };

        // 提取工具调用
        let tool_calls = if let Some(choices) = output.get("choices").and_then(|c| c.as_array()) {
            choices
                .first()
                .and_then(|c| c.get("message"))
                .map(|m| Self::extract_tool_calls(m))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // 提取结束原因
        let finish_reason = output
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .or_else(|| {
                output
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|c| c.get("finish_reason"))
                    .and_then(|f| f.as_str())
            })
            .unwrap_or("stop")
            .to_string();

        // 提取用量统计（通义千问使用 input_tokens/output_tokens）
        let usage = Self::extract_usage(response_json);

        // 提取模型名称
        let model = response_json
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        Ok(AiResponse {
            content,
            tool_calls,
            usage,
            model,
            finish_reason,
        })
    }

    /// 从消息中提取工具调用
    fn extract_tool_calls(message: &serde_json::Value) -> Vec<ToolCall> {
        message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|tc| {
                        let id = tc.get("id")?.as_str()?.to_string();
                        let func = tc.get("function")?;
                        let name = func.get("name")?.as_str()?.to_string();
                        let arguments = func
                            .get("arguments")
                            .and_then(|a| a.as_str())
                            .unwrap_or("")
                            .to_string();

                        Some(ToolCall {
                            id,
                            call_type: "function".to_string(),
                            function: FunctionCall { name, arguments },
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 提取通义千问的令牌用量统计
    ///
    /// 注意：通义千问使用 `input_tokens` 和 `output_tokens` 而非
    /// `prompt_tokens` 和 `completion_tokens`
    fn extract_usage(response: &serde_json::Value) -> TokenUsage {
        response
            .get("usage")
            .map(|u| {
                let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let output = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let total = u
                    .get("total_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_else(|| (input + output) as u64) as u32;

                TokenUsage {
                    prompt_tokens: input,
                    completion_tokens: output,
                    total_tokens: total,
                }
            })
            .unwrap_or_default()
    }

    /// 解析通义千问流式事件
    ///
    /// DashScope 流式响应格式与标准 OpenAI 不同：
    /// ```json
    /// {
    ///     "output": {
    ///         "text": "增量文本",
    ///         "finish_reason": null
    ///     },
    ///     "usage": { ... }
    /// }
    /// ```
    fn parse_stream_event(data: &str) -> AiResult<StreamEvent> {
        // 先处理 [DONE] 标记
        let json = match SseParser::parse_data(data)? {
            Some(v) => v,
            None => return Ok(StreamEvent::Done),
        };

        // 检查是否有 output 字段（通义千问格式）
        if let Some(output) = json.get("output") {
            // 检查 choices 数组（新版 API 格式）
            if let Some(choices) = output.get("choices").and_then(|c| c.as_array()) {
                if let Some(choice) = choices.first() {
                    if let Some(message) = choice.get("message") {
                        // 文本内容
                        if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                            if !content.is_empty() {
                                return Ok(StreamEvent::ContentDelta(content.to_string()));
                            }
                        }

                        // 工具调用
                        if let Some(tool_calls) =
                            message.get("tool_calls").and_then(|t| t.as_array())
                        {
                            if let Some(tc) = tool_calls.first() {
                                let id = tc
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let func = tc.get("function").cloned().unwrap_or_default();
                                let name = func
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let arguments = func
                                    .get("arguments")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                return Ok(StreamEvent::ToolCallDelta {
                                    id,
                                    name,
                                    arguments,
                                });
                            }
                        }
                    }

                    // 检查结束原因
                    if let Some(finish_reason) = choice.get("finish_reason") {
                        if !finish_reason.is_null() && finish_reason.as_str() != Some("null") {
                            // 在结束时返回用量信息
                            let usage = Self::extract_usage(&json);
                            if usage.total_tokens > 0 {
                                return Ok(StreamEvent::Usage(usage));
                            }
                            return Ok(StreamEvent::Done);
                        }
                    }
                }
            }

            // 旧版格式：直接在 output.text 中返回内容
            if let Some(text) = output.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    return Ok(StreamEvent::ContentDelta(text.to_string()));
                }
            }

            // 检查结束原因
            if let Some(finish_reason) = output.get("finish_reason") {
                if !finish_reason.is_null() && finish_reason.as_str() != Some("null") {
                    let usage = Self::extract_usage(&json);
                    if usage.total_tokens > 0 {
                        return Ok(StreamEvent::Usage(usage));
                    }
                    return Ok(StreamEvent::Done);
                }
            }
        }

        // 如果只有用量信息
        if let Some(usage) = json.get("usage") {
            if !usage.is_null() {
                let token_usage = Self::extract_usage(&json);
                if token_usage.total_tokens > 0 {
                    return Ok(StreamEvent::Usage(token_usage));
                }
            }
        }

        // 无法识别的事件
        Ok(StreamEvent::ContentDelta(String::new()))
    }

    /// 统一处理 API 错误响应
    fn handle_error_response(status_code: u16, body: &str) -> AiError {
        // DashScope 错误格式
        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| {
                v.get("message")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| body.to_string());

        match status_code {
            401 | 403 => AiError::Auth(format!("通义千问 API 密钥无效: {}", message)),
            429 => AiError::RateLimit {
                retry_after_secs: 30,
            },
            _ => AiError::Api {
                status_code,
                message,
            },
        }
    }
}

#[async_trait]
impl AiProvider for QianwenProvider {
    /// 返回提供者名称
    fn name(&self) -> &str {
        "qianwen"
    }

    /// 发送非流式聊天补全请求
    async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<AiResponse> {
        let body = self.build_request_body(messages, tools, options);

        tracing::debug!(
            provider = "qianwen",
            model = %self.resolve_model(options),
            "发送聊天补全请求"
        );

        let response = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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
    ///
    /// 通义千问通过 HTTP 头 `X-DashScope-SSE: enable` 启用 SSE 流式传输
    async fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<StreamResponse> {
        let body = self.build_request_body(messages, tools, options);

        tracing::debug!(
            provider = "qianwen",
            model = %self.resolve_model(options),
            "发送流式聊天补全请求"
        );

        // 通义千问通过特殊 HTTP 头启用 SSE
        let response = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("X-DashScope-SSE", "enable")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(Self::handle_error_response(status.as_u16(), &error_body));
        }

        // 解析 SSE 流
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
                            .map(|sse_event| Self::parse_stream_event(&sse_event.data))
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

// ============================================================
// 通义千问特有的结构体定义
// ============================================================

/// 通义千问请求输入部分
#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct QianwenInput {
    /// 消息列表
    messages: Vec<serde_json::Value>,
}

/// 通义千问请求参数部分
#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct QianwenParameters {
    /// 温度
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// 最大令牌数
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    /// Top-p
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    /// 是否增量输出
    #[serde(skip_serializing_if = "Option::is_none")]
    incremental_output: Option<bool>,
}

/// 通义千问响应输出部分
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct QianwenOutput {
    /// 文本内容
    text: Option<String>,
    /// 结束原因
    finish_reason: Option<String>,
}

/// 通义千问用量统计
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct QianwenUsage {
    /// 输入令牌数
    input_tokens: u32,
    /// 输出令牌数
    output_tokens: u32,
    /// 总令牌数
    total_tokens: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderConfig;
    use std::collections::HashMap;

    /// 创建测试用配置
    fn test_config() -> ProviderConfig {
        ProviderConfig {
            api_key: "test-dashscope-key".to_string(),
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
        let provider = QianwenProvider::new(test_config()).unwrap();
        assert_eq!(provider.name(), "qianwen");
    }

    #[test]
    fn test_default_model() {
        // 验证默认模型
        let provider = QianwenProvider::new(test_config()).unwrap();
        assert_eq!(provider.default_model, DEFAULT_MODEL);
    }

    #[test]
    fn test_custom_model() {
        // 验证自定义默认模型
        let mut config = test_config();
        config.default_model = Some("qwen-max".to_string());

        let provider = QianwenProvider::new(config).unwrap();
        assert_eq!(provider.default_model, "qwen-max");
    }

    #[test]
    fn test_build_request_body() {
        // 验证通义千问请求体格式
        let provider = QianwenProvider::new(test_config()).unwrap();
        let messages = vec![
            ChatMessage::system("你是一个助手"),
            ChatMessage::user("你好"),
        ];
        let options = ChatOptions::with_model("qwen-turbo").temperature(0.8);

        let body = provider.build_request_body(&messages, &[], &options);

        // 验证 DashScope 特有格式
        assert_eq!(body["model"], "qwen-turbo");
        assert!(body.get("input").is_some());
        assert!(body["input"].get("messages").is_some());
        assert!(body.get("parameters").is_some());
        assert_eq!(body["parameters"]["temperature"], 0.8);
        assert_eq!(body["parameters"]["incremental_output"], true);
    }

    #[test]
    fn test_convert_message() {
        // 验证消息格式转换
        let msg = ChatMessage::user("测试消息");
        let converted = QianwenProvider::convert_message(&msg);

        assert_eq!(converted["role"], "user");
        assert_eq!(converted["content"], "测试消息");
    }

    #[test]
    fn test_convert_tool_message() {
        // 验证工具消息格式转换
        let msg = ChatMessage::tool_result("call_123", "执行结果");
        let converted = QianwenProvider::convert_message(&msg);

        assert_eq!(converted["role"], "tool");
        assert_eq!(converted["tool_call_id"], "call_123");
        assert_eq!(converted["content"], "执行结果");
    }

    #[test]
    fn test_parse_response_text_format() {
        // 验证旧版 output.text 格式的响应解析
        let provider = QianwenProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "output": {
                "text": "你好！我是通义千问。",
                "finish_reason": "stop"
            },
            "usage": {
                "input_tokens": 8,
                "output_tokens": 12,
                "total_tokens": 20
            },
            "model": "qwen-turbo"
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.content, "你好！我是通义千问。");
        assert_eq!(response.finish_reason, "stop");
        assert_eq!(response.usage.prompt_tokens, 8);
        assert_eq!(response.usage.completion_tokens, 12);
        assert_eq!(response.usage.total_tokens, 20);
    }

    #[test]
    fn test_parse_response_choices_format() {
        // 验证新版 choices 格式的响应解析
        let provider = QianwenProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "output": {
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "新版格式的回复"
                    },
                    "finish_reason": "stop"
                }]
            },
            "usage": {
                "input_tokens": 5,
                "output_tokens": 10,
                "total_tokens": 15
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.content, "新版格式的回复");
    }

    #[test]
    fn test_extract_usage_qianwen_format() {
        // 验证通义千问特有的用量字段名
        let response = serde_json::json!({
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "total_tokens": 150
            }
        });

        let usage = QianwenProvider::extract_usage(&response);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_extract_usage_auto_total() {
        // 验证缺少 total_tokens 时自动计算
        let response = serde_json::json!({
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        });

        let usage = QianwenProvider::extract_usage(&response);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_parse_stream_event_content() {
        // 验证流式文本内容事件解析
        let data = r#"{"output":{"text":"你好"},"usage":{}}"#;
        let event = QianwenProvider::parse_stream_event(data).unwrap();
        match event {
            StreamEvent::ContentDelta(text) => assert_eq!(text, "你好"),
            _ => panic!("应该是 ContentDelta 事件"),
        }
    }

    #[test]
    fn test_parse_stream_event_done() {
        // 验证 [DONE] 标记处理
        let event = QianwenProvider::parse_stream_event("[DONE]").unwrap();
        assert!(matches!(event, StreamEvent::Done));
    }

    #[test]
    fn test_parse_stream_event_finish() {
        // 验证流结束事件（带用量信息）
        let data = r#"{"output":{"text":"","finish_reason":"stop"},"usage":{"input_tokens":10,"output_tokens":5,"total_tokens":15}}"#;
        let event = QianwenProvider::parse_stream_event(data).unwrap();
        match event {
            StreamEvent::Usage(usage) => {
                assert_eq!(usage.prompt_tokens, 10);
                assert_eq!(usage.completion_tokens, 5);
            }
            _ => panic!("应该是 Usage 事件"),
        }
    }

    #[test]
    fn test_handle_error_auth() {
        // 验证认证错误
        let err = QianwenProvider::handle_error_response(401, r#"{"message": "Invalid API-key"}"#);
        assert!(matches!(err, AiError::Auth(_)));
    }

    #[test]
    fn test_handle_error_rate_limit() {
        // 验证频率限制
        let err = QianwenProvider::handle_error_response(429, "Rate limit");
        assert!(matches!(err, AiError::RateLimit { .. }));
    }

    #[test]
    fn test_build_request_body_with_tools() {
        // 验证带工具定义的请求体
        let provider = QianwenProvider::new(test_config()).unwrap();
        let messages = vec![ChatMessage::user("查天气")];
        let tools = vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: crate::provider::FunctionDefinition {
                name: "get_weather".to_string(),
                description: "获取天气".to_string(),
                parameters: crate::provider::ToolParameter {
                    param_type: "object".to_string(),
                    properties: HashMap::new(),
                    required: vec![],
                },
            },
        }];
        let options = ChatOptions::with_model("qwen-turbo");

        let body = provider.build_request_body(&messages, &tools, &options);
        assert!(body["input"].get("tools").is_some());
    }
}
