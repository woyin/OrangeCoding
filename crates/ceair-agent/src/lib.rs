//! # CEAIR 代理核心模块
//!
//! 本 crate 实现了 AI 编码代理的核心事件循环、上下文管理和工具执行。
//!
//! ## 模块结构
//!
//! - [`agent_loop`] - 代理主事件循环，协调 AI 调用与工具执行
//! - [`context`] - 代理上下文管理，维护对话历史和工作目录
//! - [`executor`] - 工具执行器，支持并行执行和超时控制
//!
//! ## 使用示例
//!
//! ```no_run
//! use ceair_agent::{AgentLoop, AgentLoopConfig, AgentContext};
//! use ceair_core::{AgentId, SessionId};
//! use std::sync::Arc;
//!
//! // 创建代理上下文
//! let context = AgentContext::new(SessionId::new(), std::path::PathBuf::from("."));
//!
//! // 配置代理循环
//! let config = AgentLoopConfig::default();
//! ```

/// 代理主事件循环模块
pub mod agent_loop;

/// 上下文压缩模块
pub mod compaction;

/// 代理上下文管理模块
pub mod context;

/// 工具执行器模块
pub mod executor;

/// Hashline 编辑模块
pub mod hashline;

// ---------------------------------------------------------------------------
// 公共重导出 - 便于外部直接引用核心类型
// ---------------------------------------------------------------------------

/// 重导出代理事件循环相关类型
pub use agent_loop::{AgentLoop, AgentLoopConfig, AgentLoopResult};

/// 重导出代理上下文
pub use context::AgentContext;

/// 重导出工具执行器
pub use executor::ToolExecutor;
