//! DeepSeek 深度求索 AI 提供者适配器
//!
//! DeepSeek 使用与 OpenAI 兼容的 API 格式，支持聊天补全和工具调用。
//! 默认 API 端点: `https://api.deepseek.com/v1/chat/completions`

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::provider::{
    AiProvider, AiResponse, ChatMessage, ChatOptions, FunctionCall, ProviderConfig, StreamEvent,
    StreamResponse, TokenUsage, ToolCall, ToolDefinition,
};
use crate::stream::SseParser;
use crate::{AiError, AiResult};

/// DeepSeek 默认 API 基础地址
const DEFAULT_BASE_URL: &str = "https://api.deepseek.com/v1";

/// DeepSeek 默认模型
const DEFAULT_MODEL: &str = "deepseek-chat";

/// DeepSeek AI 提供者
///
/// 实现了与 DeepSeek API 的交互，包括：
/// - 普通聊天补全请求
/// - 流式聊天补全请求
/// - 工具/函数调用
pub struct DeepSeekProvider {
    /// HTTP 客户端
    client: Client,
    /// API 密钥
    api_key: String,
    /// API 基础地址
    base_url: String,
    /// 默认模型名称
    default_model: String,
}

impl DeepSeekProvider {
    /// 创建新的 DeepSeek 提供者实例
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

        // 添加工具定义（如果有）
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
        // 提取 choices 数组
        let choices = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .ok_or_else(|| AiError::Parse("响应中缺少 choices 字段".to_string()))?;

        let choice = choices
            .first()
            .ok_or_else(|| AiError::Parse("choices 数组为空".to_string()))?;

        // 提取消息内容
        let message = choice
            .get("message")
            .ok_or_else(|| AiError::Parse("choice 中缺少 message 字段".to_string()))?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        // 提取工具调用
        let tool_calls = Self::extract_tool_calls(message);

        // 提取结束原因
        let finish_reason = choice
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .unwrap_or("stop")
            .to_string();

        // 提取用量统计
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
}

#[async_trait]
impl AiProvider for DeepSeekProvider {
    /// 返回提供者名称
    fn name(&self) -> &str {
        "deepseek"
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
            provider = "deepseek",
            model = %self.resolve_model(options),
            "发送聊天补全请求"
        );

        // 发送 HTTP 请求
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        // 处理错误响应
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(Self::handle_error_response(status.as_u16(), &error_body));
        }

        // 解析成功响应
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
            provider = "deepseek",
            model = %self.resolve_model(options),
            "发送流式聊天补全请求"
        );

        // 发送流式 HTTP 请求
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

impl DeepSeekProvider {
    /// 统一处理 API 错误响应
    fn handle_error_response(status_code: u16, body: &str) -> AiError {
        // 尝试从响应体中提取错误信息
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
            401 => AiError::Auth(format!("DeepSeek API 密钥无效: {}", message)),
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

// ============================================================
// DeepSeek 特有的请求/响应结构体（用于序列化辅助）
// ============================================================

/// DeepSeek 聊天补全响应（用于类型安全的反序列化）
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DeepSeekResponse {
    /// 响应 ID
    id: String,
    /// 对象类型
    object: String,
    /// 创建时间戳
    created: u64,
    /// 使用的模型
    model: String,
    /// 选择列表
    choices: Vec<DeepSeekChoice>,
    /// 用量统计
    usage: Option<DeepSeekUsage>,
}

/// DeepSeek 选择项
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DeepSeekChoice {
    /// 选择索引
    index: u32,
    /// 消息内容
    message: DeepSeekMessage,
    /// 结束原因
    finish_reason: Option<String>,
}

/// DeepSeek 消息
#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
struct DeepSeekMessage {
    /// 角色
    role: String,
    /// 内容
    content: Option<String>,
    /// 工具调用
    tool_calls: Option<Vec<serde_json::Value>>,
}

/// DeepSeek 用量统计
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DeepSeekUsage {
    /// 提示令牌数
    prompt_tokens: u32,
    /// 补全令牌数
    completion_tokens: u32,
    /// 总令牌数
    total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderConfig;
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
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        assert_eq!(provider.name(), "deepseek");
    }

    #[test]
    fn test_default_model() {
        // 验证默认模型设置
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        assert_eq!(provider.default_model, DEFAULT_MODEL);
    }

    #[test]
    fn test_custom_base_url() {
        // 验证自定义基础地址
        let mut config = test_config();
        config.base_url = Some("https://custom.api.com/v1".to_string());

        let provider = DeepSeekProvider::new(config).unwrap();
        assert_eq!(provider.base_url, "https://custom.api.com/v1");
    }

    #[test]
    fn test_completions_url() {
        // 验证补全 API URL 拼接
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        assert_eq!(
            provider.completions_url(),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_build_request_body_basic() {
        // 验证基本请求体构建
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        let messages = vec![ChatMessage::user("你好")];
        let options = ChatOptions::with_model("deepseek-chat").temperature(0.7);

        let body = provider.build_request_body(&messages, &[], &options, false);

        assert_eq!(body["model"], "deepseek-chat");
        assert_eq!(body["stream"], false);
        assert_eq!(body["temperature"], 0.7);
        // 没有工具时不应包含 tools 字段
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn test_build_request_body_with_tools() {
        // 验证带工具定义的请求体
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        let messages = vec![ChatMessage::user("北京天气如何？")];
        let tools = vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: crate::provider::FunctionDefinition {
                name: "get_weather".to_string(),
                description: "获取指定城市的天气".to_string(),
                parameters: crate::provider::ToolParameter {
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
        let options = ChatOptions::with_model("deepseek-chat");

        let body = provider.build_request_body(&messages, &tools, &options, false);

        assert!(body.get("tools").is_some());
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_build_request_body_stream() {
        // 验证流式请求体包含 stream_options
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        let messages = vec![ChatMessage::user("你好")];
        let options = ChatOptions::with_model("deepseek-chat");

        let body = provider.build_request_body(&messages, &[], &options, true);

        assert_eq!(body["stream"], true);
        assert!(body.get("stream_options").is_some());
    }

    #[test]
    fn test_parse_response() {
        // 验证响应解析
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "deepseek-chat",
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
        assert_eq!(response.finish_reason, "stop");
        assert_eq!(response.usage.prompt_tokens, 10);
        assert_eq!(response.usage.completion_tokens, 20);
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn test_parse_response_with_tool_calls() {
        // 验证带工具调用的响应解析
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        let response_json = serde_json::json!({
            "id": "chatcmpl-456",
            "model": "deepseek-chat",
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
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].function.name, "get_weather");
        assert_eq!(response.finish_reason, "tool_calls");
    }

    #[test]
    fn test_handle_error_response_auth() {
        // 验证认证错误处理
        let err = DeepSeekProvider::handle_error_response(
            401,
            r#"{"error": {"message": "Invalid API key"}}"#,
        );
        assert!(matches!(err, AiError::Auth(_)));
    }

    #[test]
    fn test_handle_error_response_rate_limit() {
        // 验证频率限制错误处理
        let err = DeepSeekProvider::handle_error_response(429, "Rate limit exceeded");
        assert!(matches!(err, AiError::RateLimit { .. }));
    }

    #[test]
    fn test_handle_error_response_generic() {
        // 验证通用错误处理
        let err = DeepSeekProvider::handle_error_response(500, "Internal server error");
        assert!(matches!(
            err,
            AiError::Api {
                status_code: 500,
                ..
            }
        ));
    }

    #[test]
    fn test_resolve_model_default() {
        // 验证空模型名时使用默认模型
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        let options = ChatOptions::default();
        assert_eq!(provider.resolve_model(&options), DEFAULT_MODEL);
    }

    #[test]
    fn test_resolve_model_custom() {
        // 验证使用自定义模型名
        let provider = DeepSeekProvider::new(test_config()).unwrap();
        let options = ChatOptions::with_model("deepseek-coder");
        assert_eq!(provider.resolve_model(&options), "deepseek-coder");
    }
}
