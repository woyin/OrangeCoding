//! # chengcoding-ai
//!
//! ChengCoding AI 提供者适配层，为不同的 AI 服务商提供统一的接口抽象。
//!
//! ## 支持的提供者
//!
//! - **OpenAI**: OpenAI 兼容 API（GPT-4、GPT-4o、Ollama、LM Studio、vLLM 等）
//! - **Anthropic**: Claude 系列模型（Sonnet 4、Opus 4、Haiku）
//! - **DeepSeek**: 深度求索大模型
//! - **Qianwen (通义千问)**: 阿里云通义千问大模型
//! - **Wenxin (文心一言)**: 百度文心一言大模型
//!
//! ## 功能特性
//!
//! - 统一的聊天补全接口（支持普通请求和流式请求）
//! - 工具/函数调用支持
//! - SSE 流式解析
//! - 提供者工厂模式，方便扩展新的 AI 服务商

/// AI 提供者特征和核心类型
pub mod provider;

/// SSE 流式事件解析器
pub mod stream;

/// 各 AI 服务商的具体实现
pub mod providers;

/// 模型角色路由
pub mod model_roles;

/// 模型回退链模块
pub mod fallback;

// 重新导出核心类型，方便外部使用
pub use model_roles::{ModelConfig, ModelRole, ModelRoleRouter, ThinkingLevel};
pub use provider::{
    AiProvider, AiResponse, ChatMessage, ChatOptions, MessageRole, ProviderConfig, ProviderFactory,
    StreamEvent, TokenUsage, ToolCall, ToolDefinition, ToolParameter,
};
pub use stream::{SseParser, StreamAggregator};

/// AI 模块专用错误类型
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// 网络请求错误
    #[error("网络请求错误: {0}")]
    Network(String),

    /// API 返回错误
    #[error("API 错误 (状态码 {status_code}): {message}")]
    Api {
        /// HTTP 状态码
        status_code: u16,
        /// 错误消息
        message: String,
    },

    /// 认证错误
    #[error("认证错误: {0}")]
    Auth(String),

    /// 响应解析错误
    #[error("响应解析错误: {0}")]
    Parse(String),

    /// 流式传输错误
    #[error("流式传输错误: {0}")]
    Stream(String),

    /// 配置错误
    #[error("配置错误: {0}")]
    Config(String),

    /// 不支持的提供者
    #[error("不支持的提供者: {0}")]
    UnsupportedProvider(String),

    /// 速率限制
    #[error("请求频率超限，请在 {retry_after_secs} 秒后重试")]
    RateLimit {
        /// 建议的重试等待时间（秒）
        retry_after_secs: u64,
    },

    /// 超时错误
    #[error("请求超时: {0}")]
    Timeout(String),
}

/// AI 模块统一结果类型
pub type AiResult<T> = Result<T, AiError>;

impl From<reqwest::Error> for AiError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            AiError::Timeout(err.to_string())
        } else if err.is_connect() {
            AiError::Network(format!("连接失败: {}", err))
        } else {
            AiError::Network(err.to_string())
        }
    }
}

impl From<serde_json::Error> for AiError {
    fn from(err: serde_json::Error) -> Self {
        AiError::Parse(err.to_string())
    }
}

impl From<url::ParseError> for AiError {
    fn from(err: url::ParseError) -> Self {
        AiError::Config(format!("URL 解析错误: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_error_display() {
        // 验证各种错误类型的显示格式
        let err = AiError::Network("连接超时".to_string());
        assert!(err.to_string().contains("网络请求错误"));

        let err = AiError::Api {
            status_code: 429,
            message: "请求过多".to_string(),
        };
        assert!(err.to_string().contains("429"));

        let err = AiError::Auth("API 密钥无效".to_string());
        assert!(err.to_string().contains("认证错误"));

        let err = AiError::RateLimit {
            retry_after_secs: 30,
        };
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_reqwest_error_conversion() {
        // 验证 URL 解析错误能正确转换
        let url_err = url::Url::parse("not a url").unwrap_err();
        let ai_err: AiError = url_err.into();
        assert!(matches!(ai_err, AiError::Config(_)));
    }
}
