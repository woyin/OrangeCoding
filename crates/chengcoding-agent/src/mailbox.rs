//! # Agent 邮箱通信
//!
//! 基于内存的 Agent 间异步消息传递系统。
//!
//! # 设计思想
//! 参考 reference 中 Agent 间通信的设计：
//! - 每个 Agent 有独立的收件箱
//! - 支持结构化消息（关闭请求、状态报告等）
//! - 未读消息过滤，方便 Agent 只处理新消息
//! - 消息不可变，只能标记为已读

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// 消息类型
// ---------------------------------------------------------------------------

/// 邮箱消息类型
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageKind {
    /// 普通文本消息
    Text,
    /// 关闭请求
    ShutdownRequest,
    /// 关闭确认
    ShutdownResponse,
    /// 状态报告
    StatusReport,
    /// 任务结果
    TaskResult,
}

/// 邮箱消息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailboxMessage {
    /// 发送者 Agent 名称
    pub from: String,
    /// 消息内容
    pub text: String,
    /// 消息类型
    pub kind: MessageKind,
    /// 时间戳（Unix 毫秒）
    pub timestamp: u64,
    /// 是否已读
    pub read: bool,
    /// 摘要（可选，用于长消息的简短描述）
    pub summary: Option<String>,
}

// ---------------------------------------------------------------------------
// 邮箱
// ---------------------------------------------------------------------------

/// Agent 邮箱
///
/// 存储收到的消息列表，支持读取、写入和标记已读
pub struct Mailbox {
    /// Agent 名称
    agent_name: String,
    /// 消息列表
    messages: Vec<MailboxMessage>,
}

impl Mailbox {
    /// 创建空邮箱
    pub fn new(agent_name: impl Into<String>) -> Self {
        Self {
            agent_name: agent_name.into(),
            messages: Vec::new(),
        }
    }

    /// Agent 名称
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    /// 所有消息
    pub fn messages(&self) -> &[MailboxMessage] {
        &self.messages
    }

    /// 未读消息
    pub fn unread(&self) -> Vec<&MailboxMessage> {
        self.messages.iter().filter(|m| !m.read).collect()
    }

    /// 按类型过滤消息
    pub fn by_kind(&self, kind: &MessageKind) -> Vec<&MailboxMessage> {
        self.messages.iter().filter(|m| &m.kind == kind).collect()
    }

    /// 添加消息
    pub fn deliver(&mut self, message: MailboxMessage) {
        self.messages.push(message);
    }

    /// 标记指定索引的消息为已读
    ///
    /// 如果索引超出范围返回 false
    pub fn mark_read(&mut self, index: usize) -> bool {
        if let Some(msg) = self.messages.get_mut(index) {
            msg.read = true;
            true
        } else {
            false
        }
    }

    /// 标记所有消息为已读
    pub fn mark_all_read(&mut self) {
        for msg in &mut self.messages {
            msg.read = true;
        }
    }

    /// 消息总数
    pub fn count(&self) -> usize {
        self.messages.len()
    }

    /// 未读消息数
    pub fn unread_count(&self) -> usize {
        self.messages.iter().filter(|m| !m.read).count()
    }

    /// 清空所有消息
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

// ---------------------------------------------------------------------------
// 邮箱系统
// ---------------------------------------------------------------------------

/// 邮箱系统 — 管理多个 Agent 的邮箱
///
/// 提供 Agent 间的消息路由
pub struct MailboxSystem {
    /// Agent 名称到邮箱的映射
    mailboxes: HashMap<String, Mailbox>,
}

impl MailboxSystem {
    pub fn new() -> Self {
        Self {
            mailboxes: HashMap::new(),
        }
    }

    /// 注册 Agent 邮箱
    pub fn register(&mut self, agent_name: impl Into<String>) {
        let name = agent_name.into();
        self.mailboxes
            .entry(name.clone())
            .or_insert_with(|| Mailbox::new(name));
    }

    /// 发送消息到指定 Agent
    ///
    /// 如果目标 Agent 没有邮箱，自动创建
    pub fn send(&mut self, to: &str, from: &str, text: impl Into<String>, kind: MessageKind) {
        let message = MailboxMessage {
            from: from.to_string(),
            text: text.into(),
            kind,
            timestamp: current_timestamp_ms(),
            read: false,
            summary: None,
        };

        self.mailboxes
            .entry(to.to_string())
            .or_insert_with(|| Mailbox::new(to))
            .deliver(message);
    }

    /// 获取指定 Agent 的邮箱（只读）
    pub fn get_mailbox(&self, agent_name: &str) -> Option<&Mailbox> {
        self.mailboxes.get(agent_name)
    }

    /// 获取指定 Agent 的邮箱（可变）
    pub fn get_mailbox_mut(&mut self, agent_name: &str) -> Option<&mut Mailbox> {
        self.mailboxes.get_mut(agent_name)
    }

    /// 已注册的 Agent 数量
    pub fn agent_count(&self) -> usize {
        self.mailboxes.len()
    }
}

impl Default for MailboxSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// 获取当前时间戳（Unix 毫秒）
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(from: &str, text: &str) -> MailboxMessage {
        MailboxMessage {
            from: from.to_string(),
            text: text.to_string(),
            kind: MessageKind::Text,
            timestamp: 1000,
            read: false,
            summary: None,
        }
    }

    // --- Mailbox 测试 ---

    #[test]
    fn test_new_mailbox_empty() {
        let mb = Mailbox::new("agent-a");
        assert_eq!(mb.agent_name(), "agent-a");
        assert_eq!(mb.count(), 0);
        assert_eq!(mb.unread_count(), 0);
    }

    #[test]
    fn test_deliver_and_count() {
        let mut mb = Mailbox::new("agent-a");
        mb.deliver(make_msg("b", "hello"));
        assert_eq!(mb.count(), 1);
        assert_eq!(mb.unread_count(), 1);
    }

    #[test]
    fn test_unread_filter() {
        let mut mb = Mailbox::new("agent-a");
        mb.deliver(make_msg("b", "msg1"));
        mb.deliver(make_msg("c", "msg2"));
        mb.mark_read(0);

        let unread = mb.unread();
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].text, "msg2");
    }

    #[test]
    fn test_mark_read() {
        let mut mb = Mailbox::new("agent-a");
        mb.deliver(make_msg("b", "hello"));
        assert!(mb.mark_read(0));
        assert_eq!(mb.unread_count(), 0);
    }

    #[test]
    fn test_mark_read_out_of_bounds() {
        let mut mb = Mailbox::new("agent-a");
        assert!(!mb.mark_read(0));
    }

    #[test]
    fn test_mark_all_read() {
        let mut mb = Mailbox::new("agent-a");
        mb.deliver(make_msg("b", "1"));
        mb.deliver(make_msg("c", "2"));
        mb.deliver(make_msg("d", "3"));
        mb.mark_all_read();
        assert_eq!(mb.unread_count(), 0);
    }

    #[test]
    fn test_by_kind() {
        let mut mb = Mailbox::new("agent-a");
        mb.deliver(make_msg("b", "text"));
        mb.deliver(MailboxMessage {
            from: "c".into(),
            text: "shutdown".into(),
            kind: MessageKind::ShutdownRequest,
            timestamp: 1000,
            read: false,
            summary: None,
        });

        assert_eq!(mb.by_kind(&MessageKind::Text).len(), 1);
        assert_eq!(mb.by_kind(&MessageKind::ShutdownRequest).len(), 1);
        assert_eq!(mb.by_kind(&MessageKind::TaskResult).len(), 0);
    }

    #[test]
    fn test_clear() {
        let mut mb = Mailbox::new("agent-a");
        mb.deliver(make_msg("b", "hello"));
        mb.clear();
        assert_eq!(mb.count(), 0);
    }

    // --- MailboxSystem 测试 ---

    #[test]
    fn test_system_register_and_send() {
        let mut sys = MailboxSystem::new();
        sys.register("agent-a");
        sys.send("agent-a", "agent-b", "hello", MessageKind::Text);

        let mb = sys.get_mailbox("agent-a").unwrap();
        assert_eq!(mb.count(), 1);
        assert_eq!(mb.messages()[0].from, "agent-b");
    }

    #[test]
    fn test_system_auto_create_mailbox() {
        let mut sys = MailboxSystem::new();
        // 发送到未注册的 Agent，应自动创建邮箱
        sys.send("new-agent", "sender", "hi", MessageKind::Text);

        let mb = sys.get_mailbox("new-agent").unwrap();
        assert_eq!(mb.count(), 1);
    }

    #[test]
    fn test_system_multiple_agents() {
        let mut sys = MailboxSystem::new();
        sys.send("a", "b", "hello-a", MessageKind::Text);
        sys.send("b", "a", "hello-b", MessageKind::Text);

        assert_eq!(sys.agent_count(), 2);
        assert_eq!(sys.get_mailbox("a").unwrap().count(), 1);
        assert_eq!(sys.get_mailbox("b").unwrap().count(), 1);
    }

    #[test]
    fn test_system_get_nonexistent() {
        let sys = MailboxSystem::new();
        assert!(sys.get_mailbox("ghost").is_none());
    }

    #[test]
    fn test_system_get_mut() {
        let mut sys = MailboxSystem::new();
        sys.send("a", "b", "hello", MessageKind::Text);

        let mb = sys.get_mailbox_mut("a").unwrap();
        mb.mark_all_read();
        assert_eq!(mb.unread_count(), 0);
    }

    #[test]
    fn test_message_summary() {
        let msg = MailboxMessage {
            from: "a".into(),
            text: "very long text...".into(),
            kind: MessageKind::Text,
            timestamp: 1000,
            read: false,
            summary: Some("简短摘要".into()),
        };
        assert_eq!(msg.summary.as_deref(), Some("简短摘要"));
    }

    #[test]
    fn test_message_kinds() {
        let kinds = vec![
            MessageKind::Text,
            MessageKind::ShutdownRequest,
            MessageKind::ShutdownResponse,
            MessageKind::StatusReport,
            MessageKind::TaskResult,
        ];
        assert_eq!(kinds.len(), 5);
    }
}
