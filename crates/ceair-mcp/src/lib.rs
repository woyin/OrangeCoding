//! CEAIR MCP（Model Context Protocol）实现
//!
//! 本 crate 实现了 MCP 协议，为 CEAIR AI 编码助手提供
//! 标准化的工具调用接口。通过 MCP，外部工具可以以统一的方式
//! 被发现、描述和调用。
//!
//! # 模块结构
//!
//! - [`protocol`] - JSON-RPC 2.0 协议基础类型
//! - [`transport`] - 传输层抽象与实现（stdio）
//! - [`server`] - MCP 服务器端实现
//! - [`client`] - MCP 客户端实现
//! - [`error`] - 错误类型定义
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use ceair_mcp::server::{McpServer, McpCapabilities, ToolDefinition};
//! use ceair_mcp::transport::StdioTransport;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let server = McpServer::new(
//!         "ceair-mcp-server",
//!         "0.1.0",
//!         McpCapabilities::with_tools(),
//!     );
//!     let transport = Arc::new(StdioTransport::new());
//!     // server.start(transport).await.unwrap();
//! }
//! ```

/// JSON-RPC 2.0 协议类型
pub mod protocol;

/// 传输层抽象与实现
pub mod transport;

/// MCP 服务器实现
pub mod server;

/// MCP 客户端实现
pub mod client;

/// 错误类型定义
pub mod error;

// ============================================================================
// 便捷的类型重导出
// ============================================================================

/// 重导出常用的协议类型
pub use protocol::{
    JsonRpcError, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RequestId,
};

/// 重导出传输层类型
pub use transport::{StdioTransport, Transport};

/// 重导出服务器类型
pub use server::{McpCapabilities, McpServer, ToolDefinition, ToolHandler, ToolResult};

/// 重导出客户端类型
pub use client::{ClientConfig, McpClient};

/// 重导出错误类型
pub use error::McpError;
