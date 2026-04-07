//! 事件系统模块
//!
//! 本模块定义了 CEAIR 系统中的事件类型和事件总线接口。
//! 事件驱动架构允许各组件之间松耦合地通信，
//! 便于实现 TUI 更新、日志记录、审计追踪等功能。

use std::fmt;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::message::ToolCall;
use crate::types::{AgentId, SessionId, TokenUsage, ToolName};

// ---------------------------------------------------------------------------
// 事件类型
// ---------------------------------------------------------------------------

/// 代理事件 - 描述代理生命周期中发生的各种事件
///
/// 每个事件都携带时间戳和相关上下文数据，用于：
/// - TUI 界面的实时更新
/// - 操作日志和审计追踪
/// - 多代理之间的状态同步
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentEvent {
    /// 代理已启动
    ///
    /// 当代理被创建并开始处理任务时触发。
    Started {
        /// 代理标识符
        agent_id: AgentId,
        /// 所属会话标识符
        session_id: SessionId,
        /// 事件发生时间
        timestamp: DateTime<Utc>,
    },

    /// 收到消息
    ///
    /// 当代理接收到用户或其他代理发来的消息时触发。
    MessageReceived {
        /// 代理标识符
        agent_id: AgentId,
        /// 所属会话标识符
        session_id: SessionId,
        /// 消息内容摘要（可能经过截断）
        content_preview: String,
        /// 事件发生时间
        timestamp: DateTime<Utc>,
    },

    /// 请求工具调用
    ///
    /// 当 AI 模型决定调用某个工具时触发，此时工具尚未执行。
    ToolCallRequested {
        /// 代理标识符
        agent_id: AgentId,
        /// 所属会话标识符
        session_id: SessionId,
        /// 请求调用的工具信息
        tool_call: ToolCall,
        /// 事件发生时间
        timestamp: DateTime<Utc>,
    },

    /// 工具调用完成
    ///
    /// 当工具执行完毕并返回结果时触发。
    ToolCallCompleted {
        /// 代理标识符
        agent_id: AgentId,
        /// 所属会话标识符
        session_id: SessionId,
        /// 工具名称
        tool_name: ToolName,
        /// 执行是否成功
        success: bool,
        /// 工具执行耗时（毫秒）
        duration_ms: u64,
        /// 事件发生时间
        timestamp: DateTime<Utc>,
    },

    /// Token 使用量更新
    ///
    /// 当完成一次 AI 模型调用后，更新 token 消耗统计。
    TokenUsageUpdated {
        /// 代理标识符
        agent_id: AgentId,
        /// 所属会话标识符
        session_id: SessionId,
        /// 本次调用的 token 使用量
        usage: TokenUsage,
        /// 事件发生时间
        timestamp: DateTime<Utc>,
    },

    /// 流式输出块
    ///
    /// 当 AI 模型以流式方式返回内容时，每个文本块触发一次。
    StreamChunk {
        /// 代理标识符
        agent_id: AgentId,
        /// 所属会话标识符
        session_id: SessionId,
        /// 文本块内容
        content: String,
        /// 事件发生时间
        timestamp: DateTime<Utc>,
    },

    /// 代理已完成
    ///
    /// 当代理成功完成任务时触发。
    Completed {
        /// 代理标识符
        agent_id: AgentId,
        /// 所属会话标识符
        session_id: SessionId,
        /// 最终输出摘要
        summary: String,
        /// 事件发生时间
        timestamp: DateTime<Utc>,
    },

    /// 代理遇到错误
    ///
    /// 当代理执行过程中遇到不可恢复的错误时触发。
    Error {
        /// 代理标识符
        agent_id: AgentId,
        /// 所属会话标识符
        session_id: SessionId,
        /// 错误描述信息
        error_message: String,
        /// 事件发生时间
        timestamp: DateTime<Utc>,
    },
}

impl AgentEvent {
    /// 获取事件的时间戳
    pub fn timestamp(&self) -> &DateTime<Utc> {
        match self {
            AgentEvent::Started { timestamp, .. }
            | AgentEvent::MessageReceived { timestamp, .. }
            | AgentEvent::ToolCallRequested { timestamp, .. }
            | AgentEvent::ToolCallCompleted { timestamp, .. }
            | AgentEvent::TokenUsageUpdated { timestamp, .. }
            | AgentEvent::StreamChunk { timestamp, .. }
            | AgentEvent::Completed { timestamp, .. }
            | AgentEvent::Error { timestamp, .. } => timestamp,
        }
    }

    /// 获取事件关联的代理 ID
    pub fn agent_id(&self) -> &AgentId {
        match self {
            AgentEvent::Started { agent_id, .. }
            | AgentEvent::MessageReceived { agent_id, .. }
            | AgentEvent::ToolCallRequested { agent_id, .. }
            | AgentEvent::ToolCallCompleted { agent_id, .. }
            | AgentEvent::TokenUsageUpdated { agent_id, .. }
            | AgentEvent::StreamChunk { agent_id, .. }
            | AgentEvent::Completed { agent_id, .. }
            | AgentEvent::Error { agent_id, .. } => agent_id,
        }
    }

    /// 获取事件关联的会话 ID
    pub fn session_id(&self) -> &SessionId {
        match self {
            AgentEvent::Started { session_id, .. }
            | AgentEvent::MessageReceived { session_id, .. }
            | AgentEvent::ToolCallRequested { session_id, .. }
            | AgentEvent::ToolCallCompleted { session_id, .. }
            | AgentEvent::TokenUsageUpdated { session_id, .. }
            | AgentEvent::StreamChunk { session_id, .. }
            | AgentEvent::Completed { session_id, .. }
            | AgentEvent::Error { session_id, .. } => session_id,
        }
    }

    /// 获取事件类型的简短名称，用于日志和指标
    pub fn event_kind(&self) -> &'static str {
        match self {
            AgentEvent::Started { .. } => "started",
            AgentEvent::MessageReceived { .. } => "message_received",
            AgentEvent::ToolCallRequested { .. } => "tool_call_requested",
            AgentEvent::ToolCallCompleted { .. } => "tool_call_completed",
            AgentEvent::TokenUsageUpdated { .. } => "token_usage_updated",
            AgentEvent::StreamChunk { .. } => "stream_chunk",
            AgentEvent::Completed { .. } => "completed",
            AgentEvent::Error { .. } => "error",
        }
    }
}

impl fmt::Display for AgentEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentEvent::Started {
                agent_id,
                timestamp,
                ..
            } => {
                write!(f, "[{timestamp}] 代理 {agent_id} 已启动")
            }
            AgentEvent::MessageReceived {
                agent_id,
                content_preview,
                timestamp,
                ..
            } => {
                write!(
                    f,
                    "[{timestamp}] 代理 {agent_id} 收到消息: {content_preview}"
                )
            }
            AgentEvent::ToolCallRequested {
                agent_id,
                tool_call,
                timestamp,
                ..
            } => {
                write!(
                    f,
                    "[{timestamp}] 代理 {agent_id} 请求调用工具: {}",
                    tool_call.function_name
                )
            }
            AgentEvent::ToolCallCompleted {
                agent_id,
                tool_name,
                success,
                duration_ms,
                timestamp,
                ..
            } => {
                let status = if *success { "成功" } else { "失败" };
                write!(
                    f,
                    "[{timestamp}] 代理 {agent_id} 工具 {tool_name} 调用{status} (耗时 {duration_ms}ms)"
                )
            }
            AgentEvent::TokenUsageUpdated {
                agent_id,
                usage,
                timestamp,
                ..
            } => {
                write!(
                    f,
                    "[{timestamp}] 代理 {agent_id} {usage}"
                )
            }
            AgentEvent::StreamChunk {
                agent_id,
                content,
                timestamp,
                ..
            } => {
                // 截断过长的流式内容
                let preview = if content.len() > 50 {
                    format!("{}...", &content[..50])
                } else {
                    content.clone()
                };
                write!(f, "[{timestamp}] 代理 {agent_id} 流式输出: {preview}")
            }
            AgentEvent::Completed {
                agent_id,
                summary,
                timestamp,
                ..
            } => {
                write!(f, "[{timestamp}] 代理 {agent_id} 已完成: {summary}")
            }
            AgentEvent::Error {
                agent_id,
                error_message,
                timestamp,
                ..
            } => {
                write!(
                    f,
                    "[{timestamp}] 代理 {agent_id} 错误: {error_message}"
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 事件总线 trait
// ---------------------------------------------------------------------------

/// 事件总线接口 - 负责事件的发布和订阅
///
/// 实现此 trait 的类型需要提供事件发布和订阅能力。
/// 典型的实现可以基于 `tokio::sync::broadcast` 或 `tokio::sync::mpsc`。
#[async_trait]
pub trait EventBus: Send + Sync {
    /// 发布一个事件到事件总线
    ///
    /// 所有已订阅的处理器都会收到该事件。
    /// 如果没有订阅者，事件会被静默丢弃。
    async fn publish(&self, event: AgentEvent) -> Result<()>;

    /// 订阅事件总线，注册一个事件处理器
    ///
    /// 返回一个订阅 ID，可用于后续取消订阅。
    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<String>;

    /// 取消订阅
    ///
    /// 使用 subscribe 返回的订阅 ID 来取消订阅。
    async fn unsubscribe(&self, subscription_id: &str) -> Result<()>;
}

/// 事件处理器接口 - 处理接收到的事件
///
/// 实现此 trait 的类型可以对特定类型的事件做出响应。
/// 例如：TUI 更新处理器、日志记录处理器、审计追踪处理器等。
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// 处理一个事件
    ///
    /// 实现者应该尽快完成处理，避免阻塞事件总线。
    /// 对于耗时操作，应该使用异步任务来处理。
    async fn handle(&self, event: &AgentEvent) -> Result<()>;

    /// 获取处理器的名称，用于日志和调试
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// 便捷构造方法
// ---------------------------------------------------------------------------

impl AgentEvent {
    /// 创建一个代理启动事件
    pub fn started(agent_id: AgentId, session_id: SessionId) -> Self {
        AgentEvent::Started {
            agent_id,
            session_id,
            timestamp: Utc::now(),
        }
    }

    /// 创建一个消息接收事件
    ///
    /// 自动将内容截断为指定长度作为预览。
    pub fn message_received(
        agent_id: AgentId,
        session_id: SessionId,
        content: &str,
    ) -> Self {
        /// 预览文本的最大字符数
        const MAX_PREVIEW_LEN: usize = 200;

        let content_preview = if content.len() > MAX_PREVIEW_LEN {
            format!("{}...", &content[..MAX_PREVIEW_LEN])
        } else {
            content.to_owned()
        };

        AgentEvent::MessageReceived {
            agent_id,
            session_id,
            content_preview,
            timestamp: Utc::now(),
        }
    }

    /// 创建一个工具调用请求事件
    pub fn tool_call_requested(
        agent_id: AgentId,
        session_id: SessionId,
        tool_call: ToolCall,
    ) -> Self {
        AgentEvent::ToolCallRequested {
            agent_id,
            session_id,
            tool_call,
            timestamp: Utc::now(),
        }
    }

    /// 创建一个工具调用完成事件
    pub fn tool_call_completed(
        agent_id: AgentId,
        session_id: SessionId,
        tool_name: ToolName,
        success: bool,
        duration_ms: u64,
    ) -> Self {
        AgentEvent::ToolCallCompleted {
            agent_id,
            session_id,
            tool_name,
            success,
            duration_ms,
            timestamp: Utc::now(),
        }
    }

    /// 创建一个 token 使用量更新事件
    pub fn token_usage_updated(
        agent_id: AgentId,
        session_id: SessionId,
        usage: TokenUsage,
    ) -> Self {
        AgentEvent::TokenUsageUpdated {
            agent_id,
            session_id,
            usage,
            timestamp: Utc::now(),
        }
    }

    /// 创建一个流式输出块事件
    pub fn stream_chunk(
        agent_id: AgentId,
        session_id: SessionId,
        content: impl Into<String>,
    ) -> Self {
        AgentEvent::StreamChunk {
            agent_id,
            session_id,
            content: content.into(),
            timestamp: Utc::now(),
        }
    }

    /// 创建一个代理完成事件
    pub fn completed(
        agent_id: AgentId,
        session_id: SessionId,
        summary: impl Into<String>,
    ) -> Self {
        AgentEvent::Completed {
            agent_id,
            session_id,
            summary: summary.into(),
            timestamp: Utc::now(),
        }
    }

    /// 创建一个错误事件
    pub fn error(
        agent_id: AgentId,
        session_id: SessionId,
        error_message: impl Into<String>,
    ) -> Self {
        AgentEvent::Error {
            agent_id,
            session_id,
            error_message: error_message.into(),
            timestamp: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ToolCall;

    #[test]
    fn 测试代理启动事件的创建() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        let event = AgentEvent::started(agent_id.clone(), session_id.clone());

        assert_eq!(event.agent_id(), &agent_id);
        assert_eq!(event.session_id(), &session_id);
        assert_eq!(event.event_kind(), "started");
    }

    #[test]
    fn 测试消息接收事件的内容截断() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        // 创建一个超长字符串
        let long_content = "a".repeat(500);
        let event =
            AgentEvent::message_received(agent_id, session_id, &long_content);

        match &event {
            AgentEvent::MessageReceived {
                content_preview, ..
            } => {
                // 预览应该被截断
                assert!(content_preview.len() < 500);
                assert!(content_preview.ends_with("..."));
            }
            _ => panic!("应该是 MessageReceived 事件"),
        }
    }

    #[test]
    fn 测试消息接收事件的短内容不截断() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        let short_content = "你好世界";
        let event =
            AgentEvent::message_received(agent_id, session_id, short_content);

        match &event {
            AgentEvent::MessageReceived {
                content_preview, ..
            } => {
                assert_eq!(content_preview, short_content);
            }
            _ => panic!("应该是 MessageReceived 事件"),
        }
    }

    #[test]
    fn 测试工具调用完成事件() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        let event = AgentEvent::tool_call_completed(
            agent_id,
            session_id,
            ToolName::new("file_read"),
            true,
            150,
        );

        assert_eq!(event.event_kind(), "tool_call_completed");
        match &event {
            AgentEvent::ToolCallCompleted {
                success,
                duration_ms,
                ..
            } => {
                assert!(*success);
                assert_eq!(*duration_ms, 150);
            }
            _ => panic!("应该是 ToolCallCompleted 事件"),
        }
    }

    #[test]
    fn 测试事件的显示格式() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();

        let event = AgentEvent::started(agent_id, session_id);
        let display = format!("{event}");
        assert!(display.contains("已启动"));

        let agent_id2 = AgentId::new();
        let session_id2 = SessionId::new();
        let event2 = AgentEvent::error(agent_id2, session_id2, "测试错误");
        let display2 = format!("{event2}");
        assert!(display2.contains("错误"));
        assert!(display2.contains("测试错误"));
    }

    #[test]
    fn 测试流式输出事件() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        let event =
            AgentEvent::stream_chunk(agent_id, session_id, "Hello, World!");

        match &event {
            AgentEvent::StreamChunk { content, .. } => {
                assert_eq!(content, "Hello, World!");
            }
            _ => panic!("应该是 StreamChunk 事件"),
        }
    }

    #[test]
    fn 测试token使用量更新事件() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        let usage = TokenUsage::new(100, 50);
        let event = AgentEvent::token_usage_updated(
            agent_id,
            session_id,
            usage.clone(),
        );

        match &event {
            AgentEvent::TokenUsageUpdated {
                usage: event_usage, ..
            } => {
                assert_eq!(event_usage.total_tokens, 150);
            }
            _ => panic!("应该是 TokenUsageUpdated 事件"),
        }
    }

    #[test]
    fn 测试事件的JSON序列化和反序列化() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        let event =
            AgentEvent::completed(agent_id, session_id, "任务已完成");

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

        // 验证事件类型一致
        assert_eq!(deserialized.event_kind(), "completed");
    }

    #[test]
    fn 测试工具调用请求事件() {
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        let tool_call = ToolCall::new(
            "call_001",
            "file_read",
            serde_json::json!({"path": "/src/main.rs"}),
        );

        let event = AgentEvent::tool_call_requested(
            agent_id.clone(),
            session_id.clone(),
            tool_call,
        );

        assert_eq!(event.event_kind(), "tool_call_requested");
        match &event {
            AgentEvent::ToolCallRequested { tool_call, .. } => {
                assert_eq!(tool_call.function_name, "file_read");
            }
            _ => panic!("应该是 ToolCallRequested 事件"),
        }
    }

    #[test]
    fn 测试所有事件类型的event_kind() {
        let aid = AgentId::new();
        let sid = SessionId::new();

        assert_eq!(
            AgentEvent::started(aid.clone(), sid.clone()).event_kind(),
            "started"
        );
        assert_eq!(
            AgentEvent::message_received(aid.clone(), sid.clone(), "hi")
                .event_kind(),
            "message_received"
        );
        assert_eq!(
            AgentEvent::completed(aid.clone(), sid.clone(), "done")
                .event_kind(),
            "completed"
        );
        assert_eq!(
            AgentEvent::error(aid.clone(), sid.clone(), "oops").event_kind(),
            "error"
        );
        assert_eq!(
            AgentEvent::stream_chunk(aid.clone(), sid.clone(), "x")
                .event_kind(),
            "stream_chunk"
        );
    }
}
