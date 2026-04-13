//! # 消息分组模块
//!
//! 将对话消息按 API 轮次（round）分组，支持按组压缩和上下文管理。
//!
//! # 设计思想
//! 参考 reference 中的消息分组策略：
//! - 一次 API 调用 = 一个 round（包含 assistant 响应 + tool 调用/结果）
//! - 压缩以 round 为单位操作，保证语义完整性
//! - 保留最近 N 个 round 不压缩（preserve_recent）
//! - 支持计算每个 round 的 token 估算

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 消息角色
// ---------------------------------------------------------------------------

/// 消息角色
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        }
    }
}

// ---------------------------------------------------------------------------
// 简化消息
// ---------------------------------------------------------------------------

/// 用于分组的简化消息表示
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupMessage {
    /// 消息角色
    pub role: MessageRole,
    /// 消息内容
    pub content: String,
    /// 原始消息在对话中的索引
    pub original_index: usize,
}

impl GroupMessage {
    /// 估算 token 数（粗略：每 4 个字符 ≈ 1 token）
    pub fn estimated_tokens(&self) -> usize {
        // 对于中文，每个字符约 1-2 token；英文约 4 字符 = 1 token
        // 使用保守估计：每 3 个字符算 1 token
        (self.content.len() / 3).max(1)
    }
}

// ---------------------------------------------------------------------------
// Round（API 轮次）
// ---------------------------------------------------------------------------

/// API 轮次 — 一组语义相关的消息
///
/// 一个 round 通常包含：
/// - 1 条 user/assistant 消息（触发消息）
/// - 0~N 条 tool 调用/结果消息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Round {
    /// 轮次索引（从 0 开始）
    pub index: usize,
    /// 本轮包含的消息
    pub messages: Vec<GroupMessage>,
}

impl Round {
    /// 新建空轮次
    pub fn new(index: usize) -> Self {
        Self {
            index,
            messages: Vec::new(),
        }
    }

    /// 估算本轮的总 token 数
    pub fn estimated_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.estimated_tokens()).sum()
    }

    /// 本轮消息数量
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// 是否包含工具调用
    pub fn has_tool_calls(&self) -> bool {
        self.messages.iter().any(|m| m.role == MessageRole::Tool)
    }
}

// ---------------------------------------------------------------------------
// 消息分组器
// ---------------------------------------------------------------------------

/// 消息分组器 — 将扁平消息列表分割为 API 轮次
///
/// 分组规则：
/// 1. system 消息独立为 round 0
/// 2. 每个 user 消息开启新 round
/// 3. assistant 和 tool 消息归入当前 round
pub struct MessageGrouper;

impl MessageGrouper {
    /// 将消息列表分组为轮次
    pub fn group(messages: &[GroupMessage]) -> Vec<Round> {
        let mut rounds: Vec<Round> = Vec::new();
        let mut current_round_idx = 0;

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // system 消息独立为一个 round
                    let mut round = Round::new(current_round_idx);
                    round.messages.push(msg.clone());
                    rounds.push(round);
                    current_round_idx += 1;
                }
                MessageRole::User => {
                    // user 消息开启新 round
                    let mut round = Round::new(current_round_idx);
                    round.messages.push(msg.clone());
                    rounds.push(round);
                    current_round_idx += 1;
                }
                MessageRole::Assistant | MessageRole::Tool => {
                    // 归入当前 round（如果没有则创建）
                    if rounds.is_empty() {
                        rounds.push(Round::new(current_round_idx));
                        current_round_idx += 1;
                    }
                    rounds.last_mut().unwrap().messages.push(msg.clone());
                }
            }
        }

        rounds
    }

    /// 计算需要压缩的轮次索引范围
    ///
    /// 保留最近 `preserve_recent` 个 round 不压缩。
    /// 返回可压缩的 round 索引列表。
    pub fn compressible_rounds(rounds: &[Round], preserve_recent: usize) -> Vec<usize> {
        if rounds.len() <= preserve_recent {
            return Vec::new();
        }
        let cutoff = rounds.len() - preserve_recent;
        // 跳过 system round (index 0 通常是 system)
        rounds[..cutoff]
            .iter()
            .filter(|r| !r.messages.iter().all(|m| m.role == MessageRole::System))
            .map(|r| r.index)
            .collect()
    }

    /// 计算所有轮次的总 token 估算
    pub fn total_estimated_tokens(rounds: &[Round]) -> usize {
        rounds.iter().map(|r| r.estimated_tokens()).sum()
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试消息
    fn msg(role: MessageRole, content: &str, idx: usize) -> GroupMessage {
        GroupMessage {
            role,
            content: content.to_string(),
            original_index: idx,
        }
    }

    // -----------------------------------------------------------------------
    // GroupMessage 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_message_estimated_tokens() {
        let m = msg(MessageRole::User, "hello world", 0);
        assert!(m.estimated_tokens() > 0);
    }

    #[test]
    fn test_message_estimated_tokens_empty() {
        let m = msg(MessageRole::User, "", 0);
        assert_eq!(m.estimated_tokens(), 1, "空消息至少 1 token");
    }

    // -----------------------------------------------------------------------
    // Round 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_round_empty() {
        let round = Round::new(0);
        assert_eq!(round.estimated_tokens(), 0);
        assert_eq!(round.message_count(), 0);
        assert!(!round.has_tool_calls());
    }

    #[test]
    fn test_round_has_tool_calls() {
        let mut round = Round::new(0);
        round
            .messages
            .push(msg(MessageRole::Assistant, "调用工具", 0));
        assert!(!round.has_tool_calls());

        round.messages.push(msg(MessageRole::Tool, "结果", 1));
        assert!(round.has_tool_calls());
    }

    // -----------------------------------------------------------------------
    // MessageGrouper 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_group_empty() {
        let rounds = MessageGrouper::group(&[]);
        assert!(rounds.is_empty());
    }

    #[test]
    fn test_group_system_only() {
        let messages = vec![msg(MessageRole::System, "你是一个助手", 0)];
        let rounds = MessageGrouper::group(&messages);

        assert_eq!(rounds.len(), 1);
        assert_eq!(rounds[0].messages[0].role, MessageRole::System);
    }

    #[test]
    fn test_group_simple_conversation() {
        let messages = vec![
            msg(MessageRole::System, "系统提示", 0),
            msg(MessageRole::User, "你好", 1),
            msg(MessageRole::Assistant, "你好！", 2),
        ];
        let rounds = MessageGrouper::group(&messages);

        assert_eq!(rounds.len(), 2); // system round + user round
        assert_eq!(rounds[0].messages[0].role, MessageRole::System);
        assert_eq!(rounds[1].messages.len(), 2); // user + assistant
    }

    #[test]
    fn test_group_with_tool_calls() {
        let messages = vec![
            msg(MessageRole::System, "系统提示", 0),
            msg(MessageRole::User, "读取文件", 1),
            msg(MessageRole::Assistant, "我来读取文件", 2),
            msg(MessageRole::Tool, "文件内容...", 3),
            msg(MessageRole::Assistant, "文件内容如下...", 4),
        ];
        let rounds = MessageGrouper::group(&messages);

        assert_eq!(rounds.len(), 2); // system + user round
        assert_eq!(rounds[1].messages.len(), 4); // user + assistant + tool + assistant
        assert!(rounds[1].has_tool_calls());
    }

    #[test]
    fn test_group_multiple_user_turns() {
        let messages = vec![
            msg(MessageRole::User, "第一个问题", 0),
            msg(MessageRole::Assistant, "第一个回答", 1),
            msg(MessageRole::User, "第二个问题", 2),
            msg(MessageRole::Assistant, "第二个回答", 3),
        ];
        let rounds = MessageGrouper::group(&messages);

        assert_eq!(rounds.len(), 2);
        assert_eq!(rounds[0].messages.len(), 2);
        assert_eq!(rounds[1].messages.len(), 2);
    }

    // -----------------------------------------------------------------------
    // compressible_rounds 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_compressible_none_when_few() {
        let rounds = vec![Round::new(0), Round::new(1)];
        let result = MessageGrouper::compressible_rounds(&rounds, 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compressible_preserves_recent() {
        let messages = vec![
            msg(MessageRole::System, "系统", 0),
            msg(MessageRole::User, "问题1", 1),
            msg(MessageRole::User, "问题2", 2),
            msg(MessageRole::User, "问题3", 3),
            msg(MessageRole::User, "问题4", 4),
        ];
        let rounds = MessageGrouper::group(&messages);

        // preserve_recent=2: 保留最后 2 个 round
        let compressible = MessageGrouper::compressible_rounds(&rounds, 2);
        // 应跳过 system round，只返回 user rounds 的索引
        assert!(!compressible.is_empty());
    }

    #[test]
    fn test_compressible_skips_system() {
        let mut rounds = Vec::new();
        // Round 0: system
        let mut r0 = Round::new(0);
        r0.messages.push(msg(MessageRole::System, "系统", 0));
        rounds.push(r0);

        // Round 1-3: user rounds
        for i in 1..=3 {
            let mut r = Round::new(i);
            r.messages.push(msg(MessageRole::User, "问题", i));
            rounds.push(r);
        }

        let compressible = MessageGrouper::compressible_rounds(&rounds, 1);
        // System round 不应出现在可压缩列表中
        assert!(!compressible.contains(&0), "System round 不应被压缩");
    }

    // -----------------------------------------------------------------------
    // token 估算测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_total_estimated_tokens() {
        let messages = vec![
            msg(MessageRole::User, "hello world test", 0),
            msg(MessageRole::Assistant, "response text here", 1),
        ];
        let rounds = MessageGrouper::group(&messages);
        let total = MessageGrouper::total_estimated_tokens(&rounds);
        assert!(total > 0);
    }

    #[test]
    fn test_total_estimated_tokens_empty() {
        let total = MessageGrouper::total_estimated_tokens(&[]);
        assert_eq!(total, 0);
    }

    // -----------------------------------------------------------------------
    // MessageRole 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_role_as_str() {
        assert_eq!(MessageRole::System.as_str(), "system");
        assert_eq!(MessageRole::User.as_str(), "user");
        assert_eq!(MessageRole::Assistant.as_str(), "assistant");
        assert_eq!(MessageRole::Tool.as_str(), "tool");
    }
}
