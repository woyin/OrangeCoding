//! MCP 错误类型定义
//!
//! 本模块定义了 MCP 操作中可能出现的各种错误类型。

use thiserror::Error;

/// MCP 错误类型枚举
///
/// 覆盖了协议处理、传输通信、超时等各类错误场景。
#[derive(Debug, Error)]
pub enum McpError {
    /// 协议级别错误（JSON-RPC 格式不正确、响应解析失败等）
    #[error("MCP 协议错误: {0}")]
    Protocol(String),

    /// 传输层错误（IO 失败、连接断开等）
    #[error("MCP 传输错误: {0}")]
    Transport(String),

    /// 请求超时错误
    #[error("MCP 请求超时: {0}")]
    Timeout(String),

    /// 内部逻辑错误
    #[error("MCP 内部错误: {0}")]
    Internal(String),

    /// IO 错误（来自底层 IO 操作）
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化/反序列化错误
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    /// 来自 ceair-core 的错误
    #[error("核心错误: {0}")]
    Core(#[from] ceair_core::CeairError),
}

/// MCP 操作结果类型别名
pub type McpResult<T> = Result<T, McpError>;

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试错误的 Display 实现
    #[test]
    fn test_error_display() {
        let err = McpError::Protocol("无效消息".to_string());
        assert!(format!("{}", err).contains("协议错误"));
        assert!(format!("{}", err).contains("无效消息"));
    }

    /// 测试各种错误变体
    #[test]
    fn test_error_variants() {
        let protocol = McpError::Protocol("test".into());
        assert!(format!("{}", protocol).contains("协议"));

        let transport = McpError::Transport("test".into());
        assert!(format!("{}", transport).contains("传输"));

        let timeout = McpError::Timeout("test".into());
        assert!(format!("{}", timeout).contains("超时"));

        let internal = McpError::Internal("test".into());
        assert!(format!("{}", internal).contains("内部"));
    }

    /// 测试从 IO 错误的自动转换
    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "管道断裂");
        let mcp_err: McpError = io_err.into();
        assert!(format!("{}", mcp_err).contains("IO"));
    }

    /// 测试从 JSON 错误的自动转换
    #[test]
    fn test_json_error_conversion() {
        let json_result: Result<serde_json::Value, _> = serde_json::from_str("invalid json");
        let json_err = json_result.unwrap_err();
        let mcp_err: McpError = json_err.into();
        assert!(format!("{}", mcp_err).contains("JSON"));
    }
}
