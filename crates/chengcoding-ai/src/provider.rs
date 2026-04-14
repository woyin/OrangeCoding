//! AI 提供者特征定义和核心类型
//!
//! 本模块定义了所有 AI 服务商需要实现的统一接口，
//! 以及聊天消息、工具调用、令牌用量等核心数据结构。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::{AiError, AiResult};

// ============================================================
// 消息相关类型
// ============================================================

/// 消息角色枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// 系统消息，用于设定 AI 行为
    System,
    /// 用户消息
    User,
    /// AI 助手的回复
    Assistant,
    /// 工具/函数返回的结果
    Tool,
}

/// 聊天消息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 消息角色
    pub role: MessageRole,
    /// 消息文本内容（可为空，例如纯工具调用时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// 工具调用 ID（当角色为 Tool 时必填）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 助手发起的工具调用列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 消息附带的名称标识（部分 API 支持）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    /// 创建系统消息
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: Some(content.into()),
            tool_call_id: None,
            tool_calls: None,
            name: None,
        }
    }

    /// 创建用户消息
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: Some(content.into()),
            tool_call_id: None,
            tool_calls: None,
            name: None,
        }
    }

    /// 创建助手消息
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: Some(content.into()),
            tool_call_id: None,
            tool_calls: None,
            name: None,
        }
    }

    /// 创建工具结果消息
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: Some(content.into()),
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
            name: None,
        }
    }
}

// ============================================================
// 工具/函数调用相关类型
// ============================================================

/// 工具调用结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具调用的唯一标识
    pub id: String,
    /// 调用类型（通常为 "function"）
    #[serde(rename = "type")]
    pub call_type: String,
    /// 函数调用详情
    pub function: FunctionCall,
}

/// 函数调用详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// 函数名称
    pub name: String,
    /// 函数参数的 JSON 字符串
    pub arguments: String,
}

/// 工具定义结构体，用于向 AI 描述可用工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具类型（通常为 "function"）
    #[serde(rename = "type")]
    pub tool_type: String,
    /// 函数的详细描述
    pub function: FunctionDefinition,
}

/// 函数定义详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// 函数名称
    pub name: String,
    /// 函数功能描述
    pub description: String,
    /// 函数参数的 JSON Schema 定义
    pub parameters: ToolParameter,
}

/// 工具参数的 JSON Schema 描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// 参数类型（通常为 "object"）
    #[serde(rename = "type")]
    pub param_type: String,
    /// 各参数的属性定义
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    /// 必填参数列表
    #[serde(default)]
    pub required: Vec<String>,
}

// ============================================================
// 请求选项
// ============================================================

/// 聊天补全请求选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOptions {
    /// 使用的模型名称
    pub model: String,
    /// 温度参数，控制生成随机性 (0.0 ~ 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// 最大生成令牌数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Top-p 采样参数 (0.0 ~ 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// 停止序列列表，遇到任一序列时停止生成
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            model: String::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop_sequences: None,
        }
    }
}

impl ChatOptions {
    /// 使用指定模型创建默认选项
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// 设置温度参数
    pub fn temperature(mut self, temp: f64) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// 设置最大令牌数
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// 设置 top_p 参数
    pub fn top_p(mut self, p: f64) -> Self {
        self.top_p = Some(p);
        self
    }

    /// 设置停止序列
    pub fn stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }
}

// ============================================================
// 响应相关类型
// ============================================================

/// 令牌用量统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 输入（提示词）令牌数
    pub prompt_tokens: u32,
    /// 输出（生成）令牌数
    pub completion_tokens: u32,
    /// 总令牌数
    pub total_tokens: u32,
}

/// AI 聊天补全响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    /// 生成的文本内容
    pub content: String,
    /// 工具调用列表（若模型触发了工具调用）
    pub tool_calls: Vec<ToolCall>,
    /// 令牌用量统计
    pub usage: TokenUsage,
    /// 实际使用的模型名称
    pub model: String,
    /// 结束原因（如 "stop"、"tool_calls"、"length" 等）
    pub finish_reason: String,
}

/// 流式事件枚举
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 文本内容增量
    ContentDelta(String),
    /// 工具调用增量
    ToolCallDelta {
        /// 工具调用 ID
        id: String,
        /// 函数名称（可能仅在第一个增量中出现）
        name: String,
        /// 函数参数片段
        arguments: String,
    },
    /// 令牌用量统计（通常在流末尾）
    Usage(TokenUsage),
    /// 流结束标记
    Done,
}

/// 流式响应类型，封装异步事件流
pub type StreamResponse =
    Pin<Box<dyn futures::Stream<Item = AiResult<StreamEvent>> + Send + 'static>>;

// ============================================================
// 提供者配置
// ============================================================

/// AI 提供者配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API 密钥
    pub api_key: String,
    /// API 密钥的辅助密钥（部分服务商需要，如文心一言的 Secret Key）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_secret: Option<String>,
    /// 自定义 API 基础地址（可选，用于私有部署）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// 默认模型名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    /// 请求超时时间（秒）
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// 额外的自定义配置项
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

/// 默认超时时间：60 秒
fn default_timeout() -> u64 {
    60
}

// ============================================================
// AI 提供者特征
// ============================================================

/// AI 提供者统一接口
///
/// 所有 AI 服务商的适配器都需要实现此特征，
/// 以确保上层逻辑可以以统一方式调用不同的 AI 服务。
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// 获取提供者名称
    fn name(&self) -> &str;

    /// 发送聊天补全请求（非流式）
    ///
    /// # 参数
    /// - `messages`: 聊天消息列表
    /// - `tools`: 可用工具定义列表（可为空）
    /// - `options`: 请求选项（模型、温度等）
    ///
    /// # 返回
    /// 完整的 AI 响应，包含生成内容、工具调用和用量统计
    async fn chat_completion(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<AiResponse>;

    /// 发送流式聊天补全请求
    ///
    /// # 参数
    /// - `messages`: 聊天消息列表
    /// - `tools`: 可用工具定义列表（可为空）
    /// - `options`: 请求选项（模型、温度等）
    ///
    /// # 返回
    /// 异步事件流，逐步返回生成内容的增量
    async fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> AiResult<StreamResponse>;
}

// ============================================================
// 提供者工厂
// ============================================================

/// 提供者工厂，用于根据名称创建对应的 AI 提供者实例
pub struct ProviderFactory;

impl ProviderFactory {
    /// 根据提供者名称和配置创建 AI 提供者实例
    ///
    /// # 支持的提供者名称
    /// - `"openai"` — OpenAI 兼容 API（支持 OpenAI、Ollama、LM Studio、vLLM 等）
    /// - `"anthropic"` / `"claude"` — Anthropic Messages API
    /// - `"deepseek"` — DeepSeek 深度求索
    /// - `"qianwen"` / `"tongyi"` — 通义千问
    /// - `"wenxin"` / `"ernie"` — 文心一言
    ///
    /// # 错误
    /// 传入不支持的提供者名称时返回 `AiError::UnsupportedProvider`
    pub fn create_provider(name: &str, config: ProviderConfig) -> AiResult<Box<dyn AiProvider>> {
        match name.to_lowercase().as_str() {
            "openai" | "zai" | "z.ai" | "zen" | "opencode-zen" => Ok(Box::new(
                crate::providers::openai::OpenAiProvider::new(config)?,
            )),
            "anthropic" | "claude" => Ok(Box::new(
                crate::providers::anthropic::AnthropicProvider::new(config)?,
            )),
            "deepseek" => Ok(Box::new(crate::providers::deepseek::DeepSeekProvider::new(
                config,
            )?)),
            "qianwen" | "tongyi" | "dashscope" => Ok(Box::new(
                crate::providers::qianwen::QianwenProvider::new(config)?,
            )),
            "wenxin" | "ernie" | "baidu" => Ok(Box::new(
                crate::providers::wenxin::WenxinProvider::new(config)?,
            )),
            other => Err(AiError::UnsupportedProvider(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_constructors() {
        // 测试系统消息构造
        let msg = ChatMessage::system("你是一个编程助手");
        assert_eq!(msg.role, MessageRole::System);
        assert_eq!(msg.content.unwrap(), "你是一个编程助手");

        // 测试用户消息构造
        let msg = ChatMessage::user("帮我写一个排序算法");
        assert_eq!(msg.role, MessageRole::User);

        // 测试助手消息构造
        let msg = ChatMessage::assistant("好的，这是一个快速排序的实现...");
        assert_eq!(msg.role, MessageRole::Assistant);

        // 测试工具结果消息构造
        let msg = ChatMessage::tool_result("call_123", r#"{"result": "success"}"#);
        assert_eq!(msg.role, MessageRole::Tool);
        assert_eq!(msg.tool_call_id.unwrap(), "call_123");
    }

    #[test]
    fn test_chat_options_builder() {
        // 测试链式构建请求选项
        let opts = ChatOptions::with_model("deepseek-chat")
            .temperature(0.7)
            .max_tokens(4096)
            .top_p(0.9)
            .stop_sequences(vec!["###".to_string()]);

        assert_eq!(opts.model, "deepseek-chat");
        assert_eq!(opts.temperature.unwrap(), 0.7);
        assert_eq!(opts.max_tokens.unwrap(), 4096);
        assert_eq!(opts.top_p.unwrap(), 0.9);
        assert_eq!(opts.stop_sequences.unwrap().len(), 1);
    }

    #[test]
    fn test_message_role_serialization() {
        // 验证角色的 JSON 序列化格式
        let role = MessageRole::System;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, r#""system""#);

        let role = MessageRole::Assistant;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, r#""assistant""#);
    }

    #[test]
    fn test_tool_call_serialization() {
        // 验证工具调用的序列化和反序列化
        let tool_call = ToolCall {
            id: "call_abc123".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "get_weather".to_string(),
                arguments: r#"{"city": "北京"}"#.to_string(),
            },
        };

        let json = serde_json::to_string(&tool_call).unwrap();
        let deserialized: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "call_abc123");
        assert_eq!(deserialized.function.name, "get_weather");
    }

    #[test]
    fn test_provider_factory_unsupported() {
        // 测试不支持的提供者名称
        let config = ProviderConfig {
            api_key: "test-key".to_string(),
            api_secret: None,
            base_url: None,
            default_model: None,
            timeout_secs: 60,
            extra: HashMap::new(),
        };

        let result = ProviderFactory::create_provider("unknown_provider", config);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(matches!(err, crate::AiError::UnsupportedProvider(_)));
    }

    #[test]
    fn test_provider_factory_valid_names() {
        // 验证所有支持的提供者名称都能成功创建实例
        let names = vec![
            "openai",
            "anthropic",
            "claude",
            "deepseek",
            "qianwen",
            "tongyi",
            "dashscope",
            "wenxin",
            "ernie",
            "baidu",
            "zai",
            "z.ai",
            "zen",
            "opencode-zen",
        ];

        for name in names {
            let config = ProviderConfig {
                api_key: "test-key".to_string(),
                api_secret: Some("test-secret".to_string()),
                base_url: None,
                default_model: None,
                timeout_secs: 60,
                extra: HashMap::new(),
            };

            let result = ProviderFactory::create_provider(name, config);
            assert!(result.is_ok(), "创建提供者 '{}' 失败", name);
        }
    }

    #[test]
    fn test_token_usage_default() {
        // 验证令牌用量的默认值
        let usage = TokenUsage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }
}
