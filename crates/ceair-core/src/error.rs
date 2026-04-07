//! 错误类型定义模块
//!
//! 本模块使用 `thiserror` 定义了 CEAIR 系统的统一错误类型。
//! 所有 crate 都应该使用此处定义的 `CeairError` 和 `Result<T>` 类型，
//! 确保错误处理的一致性。

use std::fmt;

use serde::{Deserialize, Serialize};

/// CEAIR 系统统一错误类型
///
/// 涵盖了系统运行过程中可能遇到的所有错误类别。
/// 每个变体都携带上下文信息，便于调试和错误追踪。
#[derive(Debug, thiserror::Error)]
pub enum CeairError {
    /// 配置错误 - 配置文件解析失败或配置值无效
    #[error("配置错误: {message}")]
    Config {
        /// 错误描述
        message: String,
        /// 可选的错误来源
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// IO 错误 - 文件读写、路径操作等
    #[error("IO 错误: {message}")]
    Io {
        /// 错误描述
        message: String,
        /// 原始 IO 错误
        #[source]
        source: Option<std::io::Error>,
    },

    /// 网络错误 - HTTP 请求失败、连接超时等
    #[error("网络错误: {message}")]
    Network {
        /// 错误描述
        message: String,
        /// 可选的错误来源
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// AI 模型错误 - 模型调用失败、响应格式异常等
    #[error("AI 错误: {message}")]
    Ai {
        /// 错误描述
        message: String,
        /// 可选的模型名称
        model: Option<String>,
    },

    /// 代理错误 - 代理运行时异常
    #[error("代理错误 [{agent_id}]: {message}")]
    Agent {
        /// 出错的代理标识
        agent_id: String,
        /// 错误描述
        message: String,
    },

    /// 工具错误 - 工具调用失败或返回异常结果
    #[error("工具错误 [{tool_name}]: {message}")]
    Tool {
        /// 出错的工具名称
        tool_name: String,
        /// 错误描述
        message: String,
    },

    /// 协议错误 - 通信协议解析或验证失败
    #[error("协议错误: {message}")]
    Protocol {
        /// 错误描述
        message: String,
    },

    /// 序列化错误 - JSON 等格式的序列化/反序列化失败
    #[error("序列化错误: {message}")]
    Serialization {
        /// 错误描述
        message: String,
        /// 可选的错误来源
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// 认证错误 - API 密钥无效、令牌过期等
    #[error("认证错误: {message}")]
    Auth {
        /// 错误描述
        message: String,
    },

    /// 内部错误 - 不应该出现的系统内部错误
    #[error("内部错误: {message}")]
    Internal {
        /// 错误描述
        message: String,
        /// 可选的错误来源
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// CEAIR 系统统一 Result 类型别名
///
/// 所有可能产生错误的函数都应该返回此类型。
pub type Result<T> = std::result::Result<T, CeairError>;

// ---------------------------------------------------------------------------
// From 转换实现 - 让常见错误类型可以自动转换为 CeairError
// ---------------------------------------------------------------------------

impl From<std::io::Error> for CeairError {
    /// 将标准 IO 错误转换为 CeairError::Io
    fn from(err: std::io::Error) -> Self {
        CeairError::Io {
            message: err.to_string(),
            source: Some(err),
        }
    }
}

impl From<serde_json::Error> for CeairError {
    /// 将 JSON 序列化错误转换为 CeairError::Serialization
    fn from(err: serde_json::Error) -> Self {
        CeairError::Serialization {
            message: err.to_string(),
            source: Some(Box::new(err)),
        }
    }
}

// ---------------------------------------------------------------------------
// 便捷构造方法
// ---------------------------------------------------------------------------

impl CeairError {
    /// 创建一个配置错误
    pub fn config(message: impl Into<String>) -> Self {
        CeairError::Config {
            message: message.into(),
            source: None,
        }
    }

    /// 创建一个带来源的配置错误
    pub fn config_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        CeairError::Config {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// 创建一个 IO 错误（不带原始来源）
    pub fn io(message: impl Into<String>) -> Self {
        CeairError::Io {
            message: message.into(),
            source: None,
        }
    }

    /// 创建一个网络错误
    pub fn network(message: impl Into<String>) -> Self {
        CeairError::Network {
            message: message.into(),
            source: None,
        }
    }

    /// 创建一个带来源的网络错误
    pub fn network_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        CeairError::Network {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// 创建一个 AI 模型错误
    pub fn ai(message: impl Into<String>) -> Self {
        CeairError::Ai {
            message: message.into(),
            model: None,
        }
    }

    /// 创建一个带模型名称的 AI 错误
    pub fn ai_with_model(message: impl Into<String>, model: impl Into<String>) -> Self {
        CeairError::Ai {
            message: message.into(),
            model: Some(model.into()),
        }
    }

    /// 创建一个代理错误
    pub fn agent(agent_id: impl Into<String>, message: impl Into<String>) -> Self {
        CeairError::Agent {
            agent_id: agent_id.into(),
            message: message.into(),
        }
    }

    /// 创建一个工具错误
    pub fn tool(tool_name: impl Into<String>, message: impl Into<String>) -> Self {
        CeairError::Tool {
            tool_name: tool_name.into(),
            message: message.into(),
        }
    }

    /// 创建一个协议错误
    pub fn protocol(message: impl Into<String>) -> Self {
        CeairError::Protocol {
            message: message.into(),
        }
    }

    /// 创建一个序列化错误
    pub fn serialization(message: impl Into<String>) -> Self {
        CeairError::Serialization {
            message: message.into(),
            source: None,
        }
    }

    /// 创建一个认证错误
    pub fn auth(message: impl Into<String>) -> Self {
        CeairError::Auth {
            message: message.into(),
        }
    }

    /// 创建一个内部错误
    pub fn internal(message: impl Into<String>) -> Self {
        CeairError::Internal {
            message: message.into(),
            source: None,
        }
    }

    /// 创建一个带来源的内部错误
    pub fn internal_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        CeairError::Internal {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// 判断错误是否可重试
    ///
    /// 网络错误和部分 AI 错误通常可以重试，
    /// 而配置错误、认证错误等通常不可重试。
    pub fn is_retryable(&self) -> bool {
        matches!(self, CeairError::Network { .. } | CeairError::Ai { .. })
    }

    /// 获取错误分类的简短标签，用于日志和指标
    pub fn error_kind(&self) -> ErrorKind {
        match self {
            CeairError::Config { .. } => ErrorKind::Config,
            CeairError::Io { .. } => ErrorKind::Io,
            CeairError::Network { .. } => ErrorKind::Network,
            CeairError::Ai { .. } => ErrorKind::Ai,
            CeairError::Agent { .. } => ErrorKind::Agent,
            CeairError::Tool { .. } => ErrorKind::Tool,
            CeairError::Protocol { .. } => ErrorKind::Protocol,
            CeairError::Serialization { .. } => ErrorKind::Serialization,
            CeairError::Auth { .. } => ErrorKind::Auth,
            CeairError::Internal { .. } => ErrorKind::Internal,
        }
    }
}

/// 错误分类枚举 - 用于日志记录和指标统计
///
/// 与 `CeairError` 对应，但不携带具体数据，
/// 适合用作 HashMap 的键或日志字段值。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// 配置类错误
    Config,
    /// IO 类错误
    Io,
    /// 网络类错误
    Network,
    /// AI 模型类错误
    Ai,
    /// 代理类错误
    Agent,
    /// 工具类错误
    Tool,
    /// 协议类错误
    Protocol,
    /// 序列化类错误
    Serialization,
    /// 认证类错误
    Auth,
    /// 内部类错误
    Internal,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            ErrorKind::Config => "config",
            ErrorKind::Io => "io",
            ErrorKind::Network => "network",
            ErrorKind::Ai => "ai",
            ErrorKind::Agent => "agent",
            ErrorKind::Tool => "tool",
            ErrorKind::Protocol => "protocol",
            ErrorKind::Serialization => "serialization",
            ErrorKind::Auth => "auth",
            ErrorKind::Internal => "internal",
        };
        write!(f, "{label}")
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试配置错误的创建和显示() {
        let err = CeairError::config("缺少 API 密钥配置");
        let msg = format!("{err}");
        assert!(msg.contains("配置错误"));
        assert!(msg.contains("缺少 API 密钥配置"));
    }

    #[test]
    fn 测试IO错误的自动转换() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let ceair_err: CeairError = io_err.into();
        let msg = format!("{ceair_err}");
        assert!(msg.contains("IO 错误"));
    }

    #[test]
    fn 测试JSON错误的自动转换() {
        // 构造一个无效的 JSON 来触发解析错误
        let json_result: std::result::Result<serde_json::Value, _> =
            serde_json::from_str("无效JSON");
        let json_err = json_result.unwrap_err();
        let ceair_err: CeairError = json_err.into();
        let msg = format!("{ceair_err}");
        assert!(msg.contains("序列化错误"));
    }

    #[test]
    fn 测试代理错误的显示格式() {
        let err = CeairError::agent("agent-001", "任务执行超时");
        let msg = format!("{err}");
        assert!(msg.contains("agent-001"));
        assert!(msg.contains("任务执行超时"));
    }

    #[test]
    fn 测试工具错误的显示格式() {
        let err = CeairError::tool("file_read", "文件权限不足");
        let msg = format!("{err}");
        assert!(msg.contains("file_read"));
        assert!(msg.contains("文件权限不足"));
    }

    #[test]
    fn 测试错误的可重试判断() {
        // 网络错误应该可重试
        assert!(CeairError::network("连接超时").is_retryable());
        // AI 错误应该可重试
        assert!(CeairError::ai("模型响应超时").is_retryable());
        // 配置错误不应该重试
        assert!(!CeairError::config("配置无效").is_retryable());
        // 认证错误不应该重试
        assert!(!CeairError::auth("密钥过期").is_retryable());
    }

    #[test]
    fn 测试错误分类() {
        assert_eq!(CeairError::config("x").error_kind(), ErrorKind::Config);
        assert_eq!(CeairError::io("x").error_kind(), ErrorKind::Io);
        assert_eq!(CeairError::network("x").error_kind(), ErrorKind::Network);
        assert_eq!(CeairError::ai("x").error_kind(), ErrorKind::Ai);
        assert_eq!(
            CeairError::agent("a", "x").error_kind(),
            ErrorKind::Agent
        );
        assert_eq!(CeairError::tool("t", "x").error_kind(), ErrorKind::Tool);
        assert_eq!(
            CeairError::protocol("x").error_kind(),
            ErrorKind::Protocol
        );
        assert_eq!(
            CeairError::serialization("x").error_kind(),
            ErrorKind::Serialization
        );
        assert_eq!(CeairError::auth("x").error_kind(), ErrorKind::Auth);
        assert_eq!(
            CeairError::internal("x").error_kind(),
            ErrorKind::Internal
        );
    }

    #[test]
    fn 测试AI错误带模型名称() {
        let err = CeairError::ai_with_model("配额超限", "gpt-4");
        match &err {
            CeairError::Ai { model, .. } => {
                assert_eq!(model.as_deref(), Some("gpt-4"));
            }
            _ => panic!("应该是 AI 错误变体"),
        }
    }

    #[test]
    fn 测试Result类型别名() {
        // 验证 Result 类型别名可以正常使用
        fn 示例函数() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(示例函数().unwrap(), 42);

        fn 错误函数() -> Result<i32> {
            Err(CeairError::internal("测试错误"))
        }
        assert!(错误函数().is_err());
    }

    #[test]
    fn 测试错误分类的序列化() {
        let kind = ErrorKind::Network;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"network\"");

        let deserialized: ErrorKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ErrorKind::Network);
    }

    #[test]
    fn 测试带来源的内部错误() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "底层错误");
        let err = CeairError::internal_with_source("操作失败", io_err);
        let msg = format!("{err}");
        assert!(msg.contains("操作失败"));

        // 验证 source 可以获取
        assert!(err.source().is_some());
    }

    /// 引入 Error trait 以使用 source() 方法
    use std::error::Error;
}
