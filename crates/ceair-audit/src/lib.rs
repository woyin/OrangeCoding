//! # CEAIR 审计日志系统
//!
//! 本模块提供完整的审计日志功能，包括：
//! - 审计日志记录器（异步批量写入）
//! - 敏感信息脱敏处理
//! - 哈希链防篡改验证

/// 审计日志记录器模块
pub mod logger;

/// 敏感信息脱敏模块
pub mod sanitizer;

/// 哈希链防篡改模块
pub mod chain;

// 重新导出核心类型，方便外部使用
pub use logger::{AuditEntry, AuditLogger, AuditLoggerConfig};
pub use sanitizer::Sanitizer;
pub use chain::HashChain;

/// 审计系统错误类型
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// IO 操作错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 哈希链验证失败
    #[error("哈希链验证失败: {0}")]
    ChainVerification(String),

    /// 日志轮转错误
    #[error("日志轮转错误: {0}")]
    Rotation(String),
}

/// 审计系统结果类型
pub type AuditResult<T> = Result<T, AuditError>;
