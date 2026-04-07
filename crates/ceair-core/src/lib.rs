//! CEAIR 核心类型和 trait 库
//!
//! `ceair-core` 是 CEAIR 系统的基础 crate，定义了所有其他 crate 共享的
//! 核心类型、错误处理、事件系统和消息格式。
//!
//! # 模块结构
//!
//! - [`types`] - 核心类型定义（标识符、枚举、数据结构）
//! - [`error`] - 统一错误类型和 Result 别名
//! - [`event`] - 事件驱动系统（事件类型、事件总线、事件处理器）
//! - [`message`] - AI 对话消息类型（角色、消息、工具调用、对话管理）
//!
//! # 使用示例
//!
//! ```rust
//! use ceair_core::{AgentId, SessionId, AgentRole, AgentStatus, TokenUsage};
//! use ceair_core::message::{Conversation, Message};
//!
//! // 创建代理标识
//! let agent_id = AgentId::new();
//! let session_id = SessionId::new();
//!
//! // 创建对话
//! let mut conv = Conversation::with_system_prompt("你是一个AI编程助手");
//! conv.add_message(Message::user("请帮我写一个排序函数"));
//! ```

/// 核心类型模块 - 标识符、枚举和基础数据结构
pub mod types;

/// 错误处理模块 - 统一错误类型和 Result 类型别名
pub mod error;

/// 事件系统模块 - 事件类型、事件总线和处理器接口
pub mod event;

/// 消息类型模块 - AI 对话消息、工具调用和对话管理
pub mod message;

// ---------------------------------------------------------------------------
// 便捷的重导出 - 让常用类型可以直接从 crate 根引用
// ---------------------------------------------------------------------------

/// 重导出核心标识符类型
pub use types::{AgentId, SessionId, ToolName};

/// 重导出代理相关枚举
pub use types::{AgentRole, AgentStatus};

/// 重导出核心数据结构
pub use types::{AgentCapability, TokenUsage};

/// 重导出统一错误类型和 Result 别名
pub use error::{CeairError, ErrorKind, Result};

/// 重导出事件系统核心类型
pub use event::{AgentEvent, EventBus, EventHandler};
