//! ChengCoding 会话管理模块
//!
//! `chengcoding-session` 实现了基于 JSONL 树存储的会话管理系统。
//! 支持多分支对话树、上下文压缩、会话恢复和 Blob 外部存储。
//!
//! # 模块结构
//!
//! - [`entry`] - 会话条目类型（消息、压缩、分支摘要等）
//! - [`storage`] - JSONL 文件存储后端
//! - [`tree`] - 会话树结构（父子关系管理）
//! - [`manager`] - 会话生命周期管理（创建、恢复、列表）
//! - [`blob`] - Blob 外部存储（内容寻址）
//! - [`error`] - 会话错误类型
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use chengcoding_session::{SessionManager, Session};
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), chengcoding_session::SessionError> {
//! let manager = SessionManager::new(PathBuf::from("~/.chengcoding/sessions"));
//! let cwd = PathBuf::from("/home/user/project");
//!
//! // 创建新会话
//! let mut session = manager.create_session(&cwd).await?;
//! session.add_user_message("你好").await?;
//! session.add_assistant_message("你好！", "gpt-4", None).await?;
//!
//! // 获取上下文消息
//! let messages = session.context_messages();
//! # Ok(())
//! # }
//! ```

/// 会话条目类型模块
pub mod entry;

/// 会话错误类型模块
pub mod error;

/// JSONL 文件存储模块
pub mod storage;

/// 会话树结构模块
pub mod tree;

/// 会话管理器模块
pub mod manager;

/// Blob 外部存储模块
pub mod blob;

// ---------------------------------------------------------------------------
// 便捷的重导出
// ---------------------------------------------------------------------------

/// 重导出条目类型
pub use entry::{
    BranchSummaryEntry, CompactionEntry, EntryData, EntryId, EntryType, LabelEntry, MessageEntry,
    ModeChangeEntry, ModelChangeEntry, SessionEntry, ThinkingLevelEntry, ToolCallEntry,
};

/// 重导出存储类型
pub use storage::{SessionHeader, SessionStorage};

/// 重导出树类型
pub use tree::SessionTree;

/// 重导出管理器类型
pub use manager::{Session, SessionInfo, SessionManager};

/// 重导出 Blob 存储
pub use blob::BlobStore;

/// 重导出错误类型
pub use error::{SessionError, SessionResult};
