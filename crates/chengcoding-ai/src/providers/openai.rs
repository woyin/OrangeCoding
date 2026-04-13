//! OpenAI 兼容 API 适配器
//!
//! 支持所有使用 OpenAI Chat Completions API 格式的服务：
//! - OpenAI (GPT-4, GPT-4o, o1 等)
//! - Ollama (本地模型)
//! - LM Studio (本地模型)
//! - vLLM
//! - Together AI
//! - Groq
//! - 其他兼容服务

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use std::time::Duration;

use crate::provider::{
    AiProvider, AiResponse, ChatMessage, ChatOptions, FunctionCall, ProviderConfig, StreamEvent,
    StreamResponse, TokenUsage, ToolCall, ToolDefinition,
};
use crate::stream::SseParser;
use crate::{AiError, AiResult};

/// OpenAI 默认 API 基础地址
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI 默认模型
const DEFAULT_MODEL: &str = "gpt-4o";

/// OpenAI 兼容 API 适配器
///
/// 支持所有使用 OpenAI Chat Completions API 格式的服务：
/// - OpenAI (GPT-4, GPT-4o, o1 等)
/// - Ollama (本地模型)
/// - LM Studio (本地模型)
/// - vLLM
/// - Together AI
/// - Groq
/// - 其他兼容服务
pub struct OpenAiProvider {
    /// HTTP 客户端
    client: Client,
    /// API 密钥
    api_key: String,
    /// API 基础地址
    base_url: String,
    /// 默认模型名称
    default_model: String,
}

impl OpenAiProvider {
    /// 创建新的 OpenAI 兼容提供者实例
    ///
    /// # 错误
    /// 当 HTTP 客户端初始化失败时返回 `AiError::Config`
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

    /// 获取聊天补全 API 完整 URL
    fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    /// 确定实际使用的模型名称
    fn resolve_model(&self, options: &ChatOptions) -> String {
        if options.model.is_empty() {
            self.default_model.clone()
        } else {
            options.model.clone()
        }
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

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": stream,
        });

        // 设置可选参数
        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = options.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = options.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(ref stop) = options.stop_sequences {
            if !stop.is_empty() {
                body["stop"] = serde_json::json!(stop);
            }
        }

        // 添加工具定义
        if !tools.is_empty() {
            body["tools"] = serde_json::json!(tools);
        }

        // 流式请求时要求返回用量信息
        if stream {
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }

        body
    }

    /// 解析非流式 API 响应
    fn parse_response(&self, response_json: &serde_json::Value) -> AiResult<AiResponse> {
        let choices = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .ok_or_else(|| AiError::Parse("响应中缺少 choices 字段".to_string()))?;

        let choice = choices
            .first()
            .ok_or_else(|| AiError::Parse("choices 数组为空".to_string()))?;

        let message = choice
            .get("message")
            .ok_or_else(|| AiError::Parse("choice 中缺少 message 字段".to_string()))?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let tool_calls = Self::extract_tool_calls(message);

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .unwrap_or("stop")
            .to_string();

        let usage = Self::extract_usage(response_json);

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

    /// 从消息中提取工具调用列表
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

    /// 从响应中提取令牌用量统计
    fn extract_usage(response: &serde_json::Value) -> TokenUsage {
        response
            .get("usage")
            .map(|u| TokenUsage {
                prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                completion_tokens: u
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            })
            .unwrap_or_default()
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
            401 => AiError::Auth(format!("OpenAI API 密钥无效: {}", message)),
            429 => AiError::RateLimit {
                retry_after_secs: 30,
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
impl AiProvider for OpenAiProvider {
    /// 返回提供者名称
    fn name(&self) -> &str {
        "openai"
    }

    /// 发送非流式聊天补全请求
    async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<AiResponse> {
        let body = self.build_request_body(messages, tools, options, false);
        let url = self.completions_url();

        tracing::debug!(
            provider = "openai",
            model = %self.resolve_model(options),
            "发送聊天补全请求"
        );

        let response = self
            .client
            .post(&url)
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
    async fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<StreamResponse> {
        let body = self.build_request_body(messages, tools, options, true);
        let url = self.completions_url();

        tracing::debug!(
            provider = "openai",
            model = %self.resolve_model(options),
            "发送流式聊天补全请求"
        );

        let response = self
            .client
            .post(&url)
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

        // 将字节流转换为 SSE 事件流
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
                            .map(|sse_event| SseParser::parse_openai_stream_event(&sse_event.data))
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
    use crate::stream::SseParser;
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
        let provider = OpenAiProvider::new(test_config()).unwrap();
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_default_config() {
        // 验证默认配置值
        let provider = OpenAiProvider::new(test_config()).unwrap();
        assert_eq!(provider.base_url, "https://api.openai.com/v1");
        assert_eq!(provider.default_model, "gpt-4o");
    }

    #[test]
    fn test_custom_base_url() {
        // 验证自定义基础地址（如 Ollama、LM Studio 等）
        let mut config = test_config();
        config.base_url = Some("http://localhost:11434/v1".to_string());
        config.default_model = Some("llama3".to_string());

        let provider = OpenAiProvider::new(config).unwrap();
        assert_eq!(provider.base_url, "http://localhost:11434/v1");
        assert_eq!(provider.default_model, "llama3");
        assert_eq!(
            provider.completions_url(),
            "http://localhost:11434/v1/chat/completions"
        );
    }

    #[test]
    fn test_build_request_body_basic() {
        // 验证基本请求体构建
        let provider = OpenAiProvider::new(test_config()).unwrap();
        let messages = vec![ChatMessage::user("你好")];
        let options = ChatOptions::with_model("gpt-4o")
            .temperature(0.7)
            .max_tokens(4096);

        let body = provider.build_request_body(&messages, &[], &options, false);

        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["stream"], false);
        assert_eq!(body["temperature"], 0.7);
        assert_eq!(body["max_tokens"], 4096);
        // 没有工具时不应包含 tools 字段
        assert!(body.get("tools").is_none());
        // 非流式请求不应包含 stream_options
        assert!(body.get("stream_options").is_none());
    }

    #[test]
    fn test_build_request_body_with_tools() {
        // 验证带工具定义的请求体
        let provider = OpenAiProvider::new(test_config()).unwrap();
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
        let options = ChatOptions::with_model("gpt-4o");

        let body = provider.build_request_body(&messages, &tools, &options, false);

        assert!(body.get("tools").is_some());
        let tools_arr = body["tools"].as_array().unwrap();
        assert_eq!(tools_arr.len(), 1);
        assert_eq!(tools_arr[0]["type"], "function");
        assert_eq!(tools_arr[0]["function"]["name"], "get_weather");
    }

    #[test]
    fn test_build_request_body_streaming() {
        // 验证流式请求体包含 stream_options
        let provider = OpenAiProvider::new(test_config()).unwrap();
        let messages = vec![ChatMessage::user("你好")];
        let options = ChatOptions::with_model("gpt-4o");

        let body = provider.build_request_body(&messages, &[], &options, true);

        assert_eq!(body["stream"], true);
        assert!(body.get("stream_options").is_some());
        assert_eq!(body["stream_options"]["include_usage"], true);
    }

    #[test]
    fn test_parse_response_basic() {
        // 验证基本响应解析
        let provider = OpenAiProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "id": "chatcmpl-abc123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "你好！有什么可以帮你的？"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.content, "你好！有什么可以帮你的？");
        assert_eq!(response.model, "gpt-4o");
        assert_eq!(response.finish_reason, "stop");
        assert_eq!(response.usage.prompt_tokens, 10);
        assert_eq!(response.usage.completion_tokens, 20);
        assert_eq!(response.usage.total_tokens, 30);
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn test_parse_response_with_tool_calls() {
        // 验证带工具调用的响应解析
        let provider = OpenAiProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "id": "chatcmpl-456",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\": \"北京\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 15,
                "completion_tokens": 10,
                "total_tokens": 25
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.content, "");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id, "call_abc");
        assert_eq!(response.tool_calls[0].call_type, "function");
        assert_eq!(response.tool_calls[0].function.name, "get_weather");
        assert_eq!(
            response.tool_calls[0].function.arguments,
            "{\"city\": \"北京\"}"
        );
        assert_eq!(response.finish_reason, "tool_calls");
    }

    #[test]
    fn test_parse_response_empty_content() {
        // 验证 content 为 null 时返回空字符串
        let provider = OpenAiProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 0,
                "total_tokens": 5
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.content, "");
    }

    #[test]
    fn test_handle_error_response_auth() {
        // 验证 401 认证错误处理
        let err = OpenAiProvider::handle_error_response(
            401,
            r#"{"error": {"message": "Incorrect API key provided"}}"#,
        );
        assert!(matches!(err, AiError::Auth(_)));
        assert!(err.to_string().contains("OpenAI"));
    }

    #[test]
    fn test_handle_error_response_rate_limit() {
        // 验证 429 频率限制错误处理
        let err = OpenAiProvider::handle_error_response(
            429,
            r#"{"error": {"message": "Rate limit exceeded"}}"#,
        );
        assert!(matches!(err, AiError::RateLimit { .. }));
    }

    #[test]
    fn test_handle_error_response_server_error() {
        // 验证 500 服务器错误处理
        let err = OpenAiProvider::handle_error_response(500, "Internal server error");
        assert!(matches!(
            err,
            AiError::Api {
                status_code: 500,
                ..
            }
        ));
        assert!(err.to_string().contains("服务器错误"));
    }

    #[test]
    fn test_resolve_model_default() {
        // 验证空模型名时使用默认模型
        let provider = OpenAiProvider::new(test_config()).unwrap();
        let options = ChatOptions::default();
        assert_eq!(provider.resolve_model(&options), "gpt-4o");
    }

    #[test]
    fn test_resolve_model_custom() {
        // 验证使用自定义模型名
        let provider = OpenAiProvider::new(test_config()).unwrap();
        let options = ChatOptions::with_model("gpt-4-turbo");
        assert_eq!(provider.resolve_model(&options), "gpt-4-turbo");
    }

    #[test]
    fn test_parse_stream_event_content_delta() {
        // 验证流式内容增量事件解析（使用 SseParser 公共方法）
        let data = r#"{"choices":[{"index":0,"delta":{"content":"你好"},"finish_reason":null}]}"#;
        let event = SseParser::parse_openai_stream_event(data).unwrap();
        assert!(matches!(event, StreamEvent::ContentDelta(ref s) if s == "你好"));
    }

    #[test]
    fn test_parse_stream_event_done() {
        // 验证流结束标记解析
        let event = SseParser::parse_openai_stream_event("[DONE]").unwrap();
        assert!(matches!(event, StreamEvent::Done));
    }
}
