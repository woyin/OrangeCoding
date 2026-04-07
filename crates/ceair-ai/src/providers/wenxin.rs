//! 文心一言（Wenxin / ERNIE Bot）AI 提供者适配器
//!
//! 文心一言使用百度智能云 API，具有独特的认证流程：
//! 1. 使用 API Key + Secret Key 获取 access_token
//! 2. 在后续请求中使用 access_token 进行鉴权
//!
//! 默认 API 端点: `https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop/chat/`

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::provider::{
    AiProvider, AiResponse, ChatMessage, ChatOptions, FunctionCall, MessageRole, ProviderConfig,
    StreamEvent, StreamResponse, TokenUsage, ToolCall, ToolDefinition,
};
use crate::stream::SseParser;
use crate::{AiError, AiResult};

/// 文心一言默认 API 基础地址
const DEFAULT_BASE_URL: &str =
    "https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop/chat/";

/// 百度 OAuth 令牌获取地址
const TOKEN_URL: &str = "https://aip.baidubce.com/oauth/2.0/token";

/// 文心一言默认模型（映射到 API 路径后缀）
const DEFAULT_MODEL: &str = "completions_pro";

/// 模型名称到 API 路径后缀的映射
fn model_to_endpoint(model: &str) -> &str {
    match model {
        // ERNIE-Bot 4.0
        "ernie-bot-4" | "ernie-4" | "completions_pro" => "completions_pro",
        // ERNIE-Bot 8K
        "ernie-bot-8k" | "ernie-8k" => "ernie_bot_8k",
        // ERNIE-Bot Turbo
        "ernie-bot-turbo" | "ernie-turbo" | "eb-instant" => "eb-instant",
        // ERNIE-Speed
        "ernie-speed" | "ernie-speed-128k" => "ernie-speed-128k",
        // ERNIE-Lite
        "ernie-lite" | "ernie-lite-8k" => "ernie-lite-8k",
        // 默认使用原始模型名作为端点
        other => other,
    }
}

/// 缓存的访问令牌
#[derive(Debug, Clone)]
struct CachedToken {
    /// 访问令牌
    access_token: String,
    /// 过期时间（Unix 时间戳，秒）
    expires_at: u64,
}

/// 文心一言 AI 提供者
///
/// 实现了与百度智能云文心一言 API 的交互，包括：
/// - OAuth 2.0 令牌获取和缓存
/// - 普通聊天补全请求
/// - 流式聊天补全请求
/// - 工具/函数调用
pub struct WenxinProvider {
    /// HTTP 客户端
    client: Client,
    /// API Key
    api_key: String,
    /// Secret Key
    api_secret: String,
    /// API 基础地址
    base_url: String,
    /// 默认模型名称
    default_model: String,
    /// 缓存的访问令牌（线程安全读写锁）
    cached_token: Arc<RwLock<Option<CachedToken>>>,
}

impl WenxinProvider {
    /// 创建新的文心一言提供者实例
    ///
    /// # 注意
    /// 文心一言需要同时提供 `api_key` 和 `api_secret` 才能正常工作。
    pub fn new(config: ProviderConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("无法创建 HTTP 客户端");

        Self {
            client,
            api_key: config.api_key,
            api_secret: config.api_secret.unwrap_or_default(),
            base_url: config
                .base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            default_model: config
                .default_model
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            cached_token: Arc::new(RwLock::new(None)),
        }
    }

    /// 确定实际使用的模型名称
    fn resolve_model(&self, options: &ChatOptions) -> String {
        if options.model.is_empty() {
            self.default_model.clone()
        } else {
            options.model.clone()
        }
    }

    /// 获取指定模型的 API 完整 URL
    fn api_url(&self, model: &str, access_token: &str) -> String {
        let endpoint = model_to_endpoint(model);
        format!(
            "{}{}?access_token={}",
            self.base_url, endpoint, access_token
        )
    }

    /// 获取有效的访问令牌
    ///
    /// 如果缓存的令牌仍在有效期内，直接返回缓存值；
    /// 否则使用 API Key 和 Secret Key 重新获取。
    async fn get_access_token(&self) -> AiResult<String> {
        // 先尝试读取缓存
        {
            let cache = self.cached_token.read().await;
            if let Some(ref token) = *cache {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                // 提前 5 分钟刷新令牌
                if now < token.expires_at.saturating_sub(300) {
                    return Ok(token.access_token.clone());
                }
            }
        }

        // 缓存无效，请求新令牌
        tracing::debug!("文心一言访问令牌已过期或不存在，正在获取新令牌");
        let token = self.fetch_access_token().await?;

        // 更新缓存
        {
            let mut cache = self.cached_token.write().await;
            *cache = Some(token.clone());
        }

        Ok(token.access_token)
    }

    /// 从百度 OAuth 服务获取新的访问令牌
    async fn fetch_access_token(&self) -> AiResult<CachedToken> {
        if self.api_secret.is_empty() {
            return Err(AiError::Auth(
                "文心一言需要同时配置 api_key 和 api_secret".to_string(),
            ));
        }

        let response = self
            .client
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", &self.api_key),
                ("client_secret", &self.api_secret),
            ])
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::Auth(format!(
                "获取文心一言访问令牌失败 ({}): {}",
                status, body
            )));
        }

        let token_response: serde_json::Value = response.json().await?;

        let access_token = token_response
            .get("access_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| AiError::Auth("令牌响应中缺少 access_token".to_string()))?
            .to_string();

        // 计算过期时间
        let expires_in = token_response
            .get("expires_in")
            .and_then(|e| e.as_u64())
            .unwrap_or(86400); // 默认 24 小时

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(CachedToken {
            access_token,
            expires_at: now + expires_in,
        })
    }

    /// 构建文心一言请求体
    ///
    /// 文心一言的请求格式：
    /// ```json
    /// {
    ///     "messages": [
    ///         {"role": "user", "content": "..."}
    ///     ],
    ///     "temperature": 0.7,
    ///     "stream": false
    /// }
    /// ```
    ///
    /// 注意：文心一言不支持 system 角色消息，需要转换为 "system" 参数
    fn build_request_body(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
        stream: bool,
    ) -> serde_json::Value {
        // 分离系统消息和对话消息
        let mut system_content = String::new();
        let mut chat_messages: Vec<serde_json::Value> = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // 文心一言将系统消息作为独立参数
                    if let Some(ref content) = msg.content {
                        if !system_content.is_empty() {
                            system_content.push('\n');
                        }
                        system_content.push_str(content);
                    }
                }
                _ => {
                    chat_messages.push(Self::convert_message(msg));
                }
            }
        }

        let mut body = serde_json::json!({
            "messages": chat_messages,
            "stream": stream,
        });

        // 添加系统消息
        if !system_content.is_empty() {
            body["system"] = serde_json::json!(system_content);
        }

        // 设置可选参数
        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = options.max_tokens {
            body["max_output_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = options.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(ref stop) = options.stop_sequences {
            if !stop.is_empty() {
                body["stop"] = serde_json::json!(stop);
            }
        }

        // 添加工具/函数定义
        if !tools.is_empty() {
            let functions: Vec<serde_json::Value> = tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "name": tool.function.name,
                        "description": tool.function.description,
                        "parameters": tool.function.parameters,
                    })
                })
                .collect();
            body["functions"] = serde_json::json!(functions);
        }

        body
    }

    /// 将通用消息格式转换为文心一言格式
    fn convert_message(msg: &ChatMessage) -> serde_json::Value {
        let role = match msg.role {
            MessageRole::System => "user", // 文心一言不直接支持 system 角色
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "function",
        };

        let mut message = serde_json::json!({
            "role": role,
        });

        if let Some(ref content) = msg.content {
            message["content"] = serde_json::json!(content);
        }

        // 工具调用相关字段
        if let Some(ref name) = msg.name {
            message["name"] = serde_json::json!(name);
        }

        if let Some(ref tool_calls) = msg.tool_calls {
            if let Some(first_call) = tool_calls.first() {
                // 文心一言使用 function_call 而非 tool_calls
                message["function_call"] = serde_json::json!({
                    "name": first_call.function.name,
                    "arguments": first_call.function.arguments,
                });
            }
        }

        message
    }

    /// 解析文心一言非流式响应
    ///
    /// 文心一言响应格式：
    /// ```json
    /// {
    ///     "id": "...",
    ///     "result": "回复内容",
    ///     "is_truncated": false,
    ///     "finish_reason": "normal",
    ///     "usage": {
    ///         "prompt_tokens": 10,
    ///         "completion_tokens": 20,
    ///         "total_tokens": 30
    ///     },
    ///     "function_call": { "name": "...", "arguments": "..." }
    /// }
    /// ```
    fn parse_response(&self, response_json: &serde_json::Value) -> AiResult<AiResponse> {
        // 检查是否有错误
        if let Some(error_code) = response_json.get("error_code") {
            let error_msg = response_json
                .get("error_msg")
                .and_then(|m| m.as_str())
                .unwrap_or("未知错误");
            return Err(AiError::Api {
                status_code: error_code.as_u64().unwrap_or(0) as u16,
                message: error_msg.to_string(),
            });
        }

        // 提取回复内容
        let content = response_json
            .get("result")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();

        // 提取函数调用
        let tool_calls = if let Some(func_call) = response_json.get("function_call") {
            let name = func_call
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let arguments = func_call
                .get("arguments")
                .and_then(|a| a.as_str())
                .unwrap_or("")
                .to_string();

            if !name.is_empty() {
                vec![ToolCall {
                    id: format!("fc_{}", uuid_simple()),
                    call_type: "function".to_string(),
                    function: FunctionCall { name, arguments },
                }]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // 提取结束原因
        let finish_reason = response_json
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .map(|r| Self::normalize_finish_reason(r))
            .unwrap_or_else(|| "stop".to_string());

        // 提取用量统计
        let usage = Self::extract_usage(response_json);

        Ok(AiResponse {
            content,
            tool_calls,
            usage,
            model: String::new(), // 文心一言响应中不包含模型名
            finish_reason,
        })
    }

    /// 标准化结束原因字符串
    ///
    /// 文心一言使用 "normal" 而非 "stop"，需要进行转换
    fn normalize_finish_reason(reason: &str) -> String {
        match reason {
            "normal" => "stop".to_string(),
            "function_call" => "tool_calls".to_string(),
            other => other.to_string(),
        }
    }

    /// 提取令牌用量统计
    fn extract_usage(response: &serde_json::Value) -> TokenUsage {
        response
            .get("usage")
            .map(|u| TokenUsage {
                prompt_tokens: u
                    .get("prompt_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                completion_tokens: u
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                total_tokens: u
                    .get("total_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            })
            .unwrap_or_default()
    }

    /// 解析文心一言流式事件
    fn parse_stream_event(data: &str) -> AiResult<StreamEvent> {
        let json = match SseParser::parse_data(data)? {
            Some(v) => v,
            None => return Ok(StreamEvent::Done),
        };

        // 检查错误
        if let Some(error_code) = json.get("error_code") {
            let error_msg = json
                .get("error_msg")
                .and_then(|m| m.as_str())
                .unwrap_or("未知错误");
            return Err(AiError::Api {
                status_code: error_code.as_u64().unwrap_or(0) as u16,
                message: error_msg.to_string(),
            });
        }

        // 检查是否是流结束
        let is_end = json
            .get("is_end")
            .and_then(|e| e.as_bool())
            .unwrap_or(false);

        // 提取函数调用
        if let Some(func_call) = json.get("function_call") {
            let name = func_call
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let arguments = func_call
                .get("arguments")
                .and_then(|a| a.as_str())
                .unwrap_or("")
                .to_string();

            return Ok(StreamEvent::ToolCallDelta {
                id: format!("fc_{}", uuid_simple()),
                name,
                arguments,
            });
        }

        // 提取文本内容
        if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
            if !result.is_empty() {
                return Ok(StreamEvent::ContentDelta(result.to_string()));
            }
        }

        // 如果是最后一个事件，返回用量信息
        if is_end {
            let usage = Self::extract_usage(&json);
            if usage.total_tokens > 0 {
                return Ok(StreamEvent::Usage(usage));
            }
            return Ok(StreamEvent::Done);
        }

        // 无法识别的事件
        Ok(StreamEvent::ContentDelta(String::new()))
    }
}

/// 生成简单的唯一标识符（不依赖 uuid crate）
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", timestamp)
}

#[async_trait]
impl AiProvider for WenxinProvider {
    /// 返回提供者名称
    fn name(&self) -> &str {
        "wenxin"
    }

    /// 发送非流式聊天补全请求
    async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<AiResponse> {
        // 先获取访问令牌
        let access_token = self.get_access_token().await?;
        let model = self.resolve_model(options);
        let url = self.api_url(&model, &access_token);

        let body = self.build_request_body(messages, tools, options, false);

        tracing::debug!(
            provider = "wenxin",
            model = %model,
            "发送聊天补全请求"
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AiError::Api {
                status_code: status.as_u16(),
                message: error_body,
            });
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
        // 先获取访问令牌
        let access_token = self.get_access_token().await?;
        let model = self.resolve_model(options);
        let url = self.api_url(&model, &access_token);

        let body = self.build_request_body(messages, tools, options, true);

        tracing::debug!(
            provider = "wenxin",
            model = %model,
            "发送流式聊天补全请求"
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(AiError::Api {
                status_code: status.as_u16(),
                message: error_body,
            });
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
// 文心一言特有的结构体定义
// ============================================================

/// 百度 OAuth 令牌响应
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    /// 访问令牌
    access_token: String,
    /// 有效期（秒）
    expires_in: u64,
    /// 令牌类型
    token_type: Option<String>,
}

/// 文心一言聊天响应
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WenxinResponse {
    /// 响应 ID
    id: String,
    /// 回复文本
    result: String,
    /// 是否被截断
    is_truncated: bool,
    /// 结束原因
    finish_reason: String,
    /// 用量统计
    usage: WenxinUsage,
    /// 函数调用
    function_call: Option<WenxinFunctionCall>,
}

/// 文心一言用量统计
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WenxinUsage {
    /// 提示令牌数
    prompt_tokens: u32,
    /// 补全令牌数
    completion_tokens: u32,
    /// 总令牌数
    total_tokens: u32,
}

/// 文心一言函数调用
#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
struct WenxinFunctionCall {
    /// 函数名称
    name: String,
    /// 函数参数
    arguments: String,
}

/// 文心一言流式响应块
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WenxinStreamChunk {
    /// 响应 ID
    id: String,
    /// 增量文本
    result: String,
    /// 是否结束
    is_end: bool,
    /// 用量统计（仅在最后一个块中出现）
    usage: Option<WenxinUsage>,
    /// 函数调用
    function_call: Option<WenxinFunctionCall>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderConfig;
    use std::collections::HashMap;

    /// 创建测试用配置
    fn test_config() -> ProviderConfig {
        ProviderConfig {
            api_key: "test-api-key".to_string(),
            api_secret: Some("test-secret-key".to_string()),
            base_url: None,
            default_model: None,
            timeout_secs: 30,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_provider_name() {
        // 验证提供者名称
        let provider = WenxinProvider::new(test_config());
        assert_eq!(provider.name(), "wenxin");
    }

    #[test]
    fn test_default_model() {
        // 验证默认模型
        let provider = WenxinProvider::new(test_config());
        assert_eq!(provider.default_model, DEFAULT_MODEL);
    }

    #[test]
    fn test_model_to_endpoint_mapping() {
        // 验证模型名到端点的映射
        assert_eq!(model_to_endpoint("ernie-bot-4"), "completions_pro");
        assert_eq!(model_to_endpoint("ernie-bot-turbo"), "eb-instant");
        assert_eq!(model_to_endpoint("ernie-speed"), "ernie-speed-128k");
        assert_eq!(model_to_endpoint("ernie-lite"), "ernie-lite-8k");
        assert_eq!(model_to_endpoint("custom-model"), "custom-model");
    }

    #[test]
    fn test_api_url() {
        // 验证 API URL 拼接
        let provider = WenxinProvider::new(test_config());
        let url = provider.api_url("completions_pro", "test_token");
        assert!(url.contains("completions_pro"));
        assert!(url.contains("access_token=test_token"));
    }

    #[test]
    fn test_build_request_body_basic() {
        // 验证基本请求体
        let provider = WenxinProvider::new(test_config());
        let messages = vec![ChatMessage::user("你好")];
        let options = ChatOptions::with_model("ernie-bot-4").temperature(0.7);

        let body = provider.build_request_body(&messages, &[], &options, false);

        assert_eq!(body["stream"], false);
        assert_eq!(body["temperature"], 0.7);
        assert!(body["messages"].is_array());
    }

    #[test]
    fn test_build_request_body_system_message() {
        // 验证系统消息被提取为独立参数
        let provider = WenxinProvider::new(test_config());
        let messages = vec![
            ChatMessage::system("你是一个编程助手"),
            ChatMessage::user("写一个排序算法"),
        ];
        let options = ChatOptions::default();

        let body = provider.build_request_body(&messages, &[], &options, false);

        // 系统消息应作为 system 参数
        assert_eq!(body["system"], "你是一个编程助手");
        // messages 中不应包含系统消息
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn test_build_request_body_with_functions() {
        // 验证带函数定义的请求体
        let provider = WenxinProvider::new(test_config());
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
        let options = ChatOptions::default();

        let body = provider.build_request_body(&messages, &tools, &options, false);
        assert!(body.get("functions").is_some());
        assert_eq!(body["functions"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_parse_response_basic() {
        // 验证基本响应解析
        let provider = WenxinProvider::new(test_config());
        let response_json = serde_json::json!({
            "id": "as-xxx",
            "result": "你好！我是文心一言。",
            "is_truncated": false,
            "finish_reason": "normal",
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 15,
                "total_tokens": 20
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.content, "你好！我是文心一言。");
        assert_eq!(response.finish_reason, "stop"); // "normal" 被转换为 "stop"
        assert_eq!(response.usage.prompt_tokens, 5);
        assert_eq!(response.usage.completion_tokens, 15);
    }

    #[test]
    fn test_parse_response_with_function_call() {
        // 验证带函数调用的响应解析
        let provider = WenxinProvider::new(test_config());
        let response_json = serde_json::json!({
            "id": "as-xxx",
            "result": "",
            "is_truncated": false,
            "finish_reason": "function_call",
            "function_call": {
                "name": "get_weather",
                "arguments": "{\"city\": \"上海\"}"
            },
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        });

        let response = provider.parse_response(&response_json).unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].function.name, "get_weather");
        assert_eq!(response.finish_reason, "tool_calls");
    }

    #[test]
    fn test_parse_response_error() {
        // 验证错误响应解析
        let provider = WenxinProvider::new(test_config());
        let response_json = serde_json::json!({
            "error_code": 17,
            "error_msg": "Open api daily request limit reached"
        });

        let result = provider.parse_response(&response_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_finish_reason() {
        // 验证结束原因标准化
        assert_eq!(WenxinProvider::normalize_finish_reason("normal"), "stop");
        assert_eq!(
            WenxinProvider::normalize_finish_reason("function_call"),
            "tool_calls"
        );
        assert_eq!(
            WenxinProvider::normalize_finish_reason("length"),
            "length"
        );
    }

    #[test]
    fn test_parse_stream_event_content() {
        // 验证流式文本内容事件
        let data = r#"{"id":"as-xxx","result":"你好","is_end":false}"#;
        let event = WenxinProvider::parse_stream_event(data).unwrap();
        match event {
            StreamEvent::ContentDelta(text) => assert_eq!(text, "你好"),
            _ => panic!("应该是 ContentDelta 事件"),
        }
    }

    #[test]
    fn test_parse_stream_event_end() {
        // 验证流结束事件
        let data = r#"{"id":"as-xxx","result":"","is_end":true,"usage":{"prompt_tokens":5,"completion_tokens":10,"total_tokens":15}}"#;
        let event = WenxinProvider::parse_stream_event(data).unwrap();
        match event {
            StreamEvent::Usage(usage) => {
                assert_eq!(usage.prompt_tokens, 5);
                assert_eq!(usage.total_tokens, 15);
            }
            _ => panic!("应该是 Usage 事件"),
        }
    }

    #[test]
    fn test_parse_stream_event_function_call() {
        // 验证流式函数调用事件
        let data = r#"{"id":"as-xxx","result":"","function_call":{"name":"search","arguments":"{\"q\":\"test\"}"},"is_end":false}"#;
        let event = WenxinProvider::parse_stream_event(data).unwrap();
        match event {
            StreamEvent::ToolCallDelta {
                name, arguments, ..
            } => {
                assert_eq!(name, "search");
                assert_eq!(arguments, "{\"q\":\"test\"}");
            }
            _ => panic!("应该是 ToolCallDelta 事件"),
        }
    }

    #[test]
    fn test_parse_stream_event_done() {
        // 验证 [DONE] 标记
        let event = WenxinProvider::parse_stream_event("[DONE]").unwrap();
        assert!(matches!(event, StreamEvent::Done));
    }

    #[test]
    fn test_parse_stream_event_error() {
        // 验证流式错误事件
        let data = r#"{"error_code": 336003, "error_msg": "Invalid parameter"}"#;
        let result = WenxinProvider::parse_stream_event(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_message_user() {
        // 验证用户消息转换
        let msg = ChatMessage::user("测试");
        let converted = WenxinProvider::convert_message(&msg);
        assert_eq!(converted["role"], "user");
        assert_eq!(converted["content"], "测试");
    }

    #[test]
    fn test_convert_message_tool_result() {
        // 验证工具结果消息转换
        let msg = ChatMessage::tool_result("call_1", "结果");
        let converted = WenxinProvider::convert_message(&msg);
        // 文心一言将 tool 角色映射为 "function"
        assert_eq!(converted["role"], "function");
        assert_eq!(converted["content"], "结果");
    }

    #[test]
    fn test_uuid_simple() {
        // 验证简单 UUID 生成
        let id1 = uuid_simple();
        let id2 = uuid_simple();
        // 不应为空
        assert!(!id1.is_empty());
        assert!(!id2.is_empty());
    }

    #[tokio::test]
    async fn test_get_access_token_no_secret() {
        // 验证缺少 secret key 时的错误处理
        let mut config = test_config();
        config.api_secret = None;

        let provider = WenxinProvider::new(config);
        let result = provider.get_access_token().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AiError::Auth(_)));
    }

    #[test]
    fn test_stream_request_body() {
        // 验证流式请求体包含 stream: true
        let provider = WenxinProvider::new(test_config());
        let messages = vec![ChatMessage::user("你好")];
        let options = ChatOptions::default();

        let body = provider.build_request_body(&messages, &[], &options, true);
        assert_eq!(body["stream"], true);
    }
}
