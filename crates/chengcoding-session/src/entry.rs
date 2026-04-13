//! 会话条目类型模块
//!
//! 本模块定义了会话中所有条目的数据结构，包括消息、思考级别变更、
//! 模型切换、上下文压缩、分支摘要、书签标签和模式变更等。
//! 所有条目使用统一的 `SessionEntry` 结构，通过 `EntryData` 枚举区分具体类型。

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use chengcoding_core::message::Role;
use chengcoding_core::TokenUsage;

// ---------------------------------------------------------------------------
// 条目标识符
// ---------------------------------------------------------------------------

/// 会话条目的唯一标识符（雪花ID风格的十六进制字符串）
///
/// 使用时间戳（毫秒）加随机数生成，保证全局唯一性且按时间排序。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntryId(String);

impl EntryId {
    /// 生成新的条目 ID（时间戳十六进制 + 随机十六进制后缀）
    pub fn new() -> Self {
        let ts = Utc::now().timestamp_millis() as u64;
        let rand_bytes: u32 = rand_u32();
        Self(format!("{:012x}{:08x}", ts, rand_bytes))
    }

    /// 从已有字符串创建条目 ID
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取 ID 的字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 使用 ring 生成随机 u32
fn rand_u32() -> u32 {
    let rng = ring::rand::SystemRandom::new();
    let mut buf = [0u8; 4];
    ring::rand::SecureRandom::fill(&rng, &mut buf).expect("系统随机数生成失败");
    u32::from_le_bytes(buf)
}

impl Default for EntryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// 条目类型枚举
// ---------------------------------------------------------------------------

/// 会话条目类型枚举
///
/// 标识条目的语义类型，决定了 `EntryData` 中具体数据的类型。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    /// 消息（用户/助手/系统/工具）
    Message,
    /// 思考级别变更
    ThinkingLevel,
    /// 模型切换
    ModelChange,
    /// 上下文压缩摘要
    Compaction,
    /// 分支摘要
    BranchSummary,
    /// 书签标签
    Label,
    /// 模式变更
    ModeChange,
    /// 自定义条目
    Custom,
}

impl fmt::Display for EntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            EntryType::Message => "消息",
            EntryType::ThinkingLevel => "思考级别",
            EntryType::ModelChange => "模型切换",
            EntryType::Compaction => "上下文压缩",
            EntryType::BranchSummary => "分支摘要",
            EntryType::Label => "书签标签",
            EntryType::ModeChange => "模式变更",
            EntryType::Custom => "自定义",
        };
        write!(f, "{label}")
    }
}

// ---------------------------------------------------------------------------
// 条目数据
// ---------------------------------------------------------------------------

/// 条目数据 - 按类型区分的具体数据
///
/// 与 `EntryType` 一一对应，封装不同类型条目的详细信息。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum EntryData {
    /// 消息数据
    Message(MessageEntry),
    /// 思考级别变更数据
    ThinkingLevel(ThinkingLevelEntry),
    /// 模型切换数据
    ModelChange(ModelChangeEntry),
    /// 上下文压缩摘要数据
    Compaction(CompactionEntry),
    /// 分支摘要数据
    BranchSummary(BranchSummaryEntry),
    /// 书签标签数据
    Label(LabelEntry),
    /// 模式变更数据
    ModeChange(ModeChangeEntry),
    /// 自定义数据（任意 JSON）
    Custom(serde_json::Value),
}

// ---------------------------------------------------------------------------
// 具体条目结构
// ---------------------------------------------------------------------------

/// 工具调用条目 - 描述一次工具调用的请求信息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallEntry {
    /// 工具调用的唯一标识符
    pub id: String,
    /// 工具名称
    pub name: String,
    /// 工具参数（JSON）
    pub arguments: serde_json::Value,
}

/// 消息条目 - 对话中的一条消息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageEntry {
    /// 消息角色（来自 ceair-core）
    pub role: Role,
    /// 消息文本内容
    pub content: String,
    /// 工具调用列表（助手消息可能包含）
    #[serde(default)]
    pub tool_calls: Vec<ToolCallEntry>,
    /// 关联的工具调用 ID（工具结果消息使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 使用的模型名称
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Token 使用量统计
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

/// 思考级别变更条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThinkingLevelEntry {
    /// 新的思考级别
    pub level: String,
}

/// 模型切换条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelChangeEntry {
    /// 切换前的模型
    pub from: Option<String>,
    /// 切换后的模型
    pub to: String,
}

/// 上下文压缩摘要条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactionEntry {
    /// 压缩后的摘要文本
    pub summary: String,
    /// 短摘要（用于快速预览）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_summary: Option<String>,
    /// 保留的第一个条目 ID（之前的条目被压缩）
    pub first_kept_entry_id: EntryId,
    /// 压缩前的 token 数
    pub tokens_before: u64,
}

/// 分支摘要条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchSummaryEntry {
    /// 分支起始条目 ID
    pub from_id: EntryId,
    /// 分支摘要文本
    pub summary: String,
}

/// 书签标签条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabelEntry {
    /// 标签名称
    pub name: String,
    /// 标签描述
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 模式变更条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModeChangeEntry {
    /// 切换前的模式
    pub from: Option<String>,
    /// 切换后的模式
    pub to: String,
}

// ---------------------------------------------------------------------------
// 会话条目
// ---------------------------------------------------------------------------

/// 会话条目 - 所有条目的统一结构
///
/// 每个条目都有唯一 ID、可选的父条目引用（构成树结构）、
/// 时间戳以及按类型区分的具体数据。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionEntry {
    /// 条目类型
    pub entry_type: EntryType,
    /// 条目唯一标识符
    pub id: EntryId,
    /// 父条目 ID（根条目为 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<EntryId>,
    /// 条目创建时间
    pub timestamp: DateTime<Utc>,
    /// 条目数据
    pub data: EntryData,
}

impl SessionEntry {
    /// 创建消息类型的条目
    pub fn message(parent_id: Option<EntryId>, msg: MessageEntry) -> Self {
        Self {
            entry_type: EntryType::Message,
            id: EntryId::new(),
            parent_id,
            timestamp: Utc::now(),
            data: EntryData::Message(msg),
        }
    }

    /// 创建压缩类型的条目
    pub fn compaction(parent_id: Option<EntryId>, compaction: CompactionEntry) -> Self {
        Self {
            entry_type: EntryType::Compaction,
            id: EntryId::new(),
            parent_id,
            timestamp: Utc::now(),
            data: EntryData::Compaction(compaction),
        }
    }

    /// 获取条目中的消息数据（如果是消息类型）
    pub fn as_message(&self) -> Option<&MessageEntry> {
        match &self.data {
            EntryData::Message(msg) => Some(msg),
            _ => None,
        }
    }

    /// 获取条目中的压缩数据（如果是压缩类型）
    pub fn as_compaction(&self) -> Option<&CompactionEntry> {
        match &self.data {
            EntryData::Compaction(c) => Some(c),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn 测试条目ID的生成和唯一性() {
        let id1 = EntryId::new();
        let id2 = EntryId::new();
        // 两个 ID 必须不同
        assert_ne!(id1, id2);
        // ID 长度为 20（12 位时间戳 + 8 位随机数）
        assert_eq!(id1.as_str().len(), 20);
        assert_eq!(id2.as_str().len(), 20);
    }

    #[test]
    fn 测试条目ID批量生成唯一性() {
        let ids: Vec<EntryId> = (0..100).map(|_| EntryId::new()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "100 个 ID 必须全部唯一");
    }

    #[test]
    fn 测试条目ID从字符串创建() {
        let id = EntryId::from_string("abc123");
        assert_eq!(id.as_str(), "abc123");
        assert_eq!(format!("{id}"), "abc123");
    }

    #[test]
    fn 测试条目类型的显示() {
        assert_eq!(format!("{}", EntryType::Message), "消息");
        assert_eq!(format!("{}", EntryType::ThinkingLevel), "思考级别");
        assert_eq!(format!("{}", EntryType::ModelChange), "模型切换");
        assert_eq!(format!("{}", EntryType::Compaction), "上下文压缩");
        assert_eq!(format!("{}", EntryType::BranchSummary), "分支摘要");
        assert_eq!(format!("{}", EntryType::Label), "书签标签");
        assert_eq!(format!("{}", EntryType::ModeChange), "模式变更");
        assert_eq!(format!("{}", EntryType::Custom), "自定义");
    }

    #[test]
    fn 测试条目类型的序列化和反序列化() {
        let entry_type = EntryType::Message;
        let json = serde_json::to_string(&entry_type).unwrap();
        assert_eq!(json, "\"message\"");
        let deserialized: EntryType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, EntryType::Message);
    }

    #[test]
    fn 测试消息条目的序列化往返() {
        let msg = MessageEntry {
            role: Role::User,
            content: "你好世界".to_string(),
            tool_calls: vec![],
            tool_call_id: None,
            model: None,
            token_usage: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: MessageEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, Role::User);
        assert_eq!(deserialized.content, "你好世界");
    }

    #[test]
    fn 测试带工具调用的消息条目序列化() {
        let msg = MessageEntry {
            role: Role::Assistant,
            content: "我来帮你执行命令".to_string(),
            tool_calls: vec![ToolCallEntry {
                id: "call_001".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "ls"}),
            }],
            tool_call_id: None,
            model: Some("gpt-4".to_string()),
            token_usage: Some(TokenUsage::new(100, 50)),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: MessageEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool_calls.len(), 1);
        assert_eq!(deserialized.tool_calls[0].name, "bash");
        assert_eq!(deserialized.model.as_deref(), Some("gpt-4"));
        assert_eq!(deserialized.token_usage.as_ref().unwrap().total_tokens, 150);
    }

    #[test]
    fn 测试压缩条目的序列化往返() {
        let compaction = CompactionEntry {
            summary: "用户讨论了排序算法的实现".to_string(),
            short_summary: Some("排序算法".to_string()),
            first_kept_entry_id: EntryId::from_string("abc123"),
            tokens_before: 5000,
        };
        let json = serde_json::to_string(&compaction).unwrap();
        let deserialized: CompactionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.summary, "用户讨论了排序算法的实现");
        assert_eq!(deserialized.short_summary.as_deref(), Some("排序算法"));
        assert_eq!(deserialized.tokens_before, 5000);
    }

    #[test]
    fn 测试分支摘要条目的序列化往返() {
        let branch = BranchSummaryEntry {
            from_id: EntryId::from_string("entry_001"),
            summary: "探索了另一种实现方案".to_string(),
        };
        let json = serde_json::to_string(&branch).unwrap();
        let deserialized: BranchSummaryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.from_id.as_str(), "entry_001");
        assert_eq!(deserialized.summary, "探索了另一种实现方案");
    }

    #[test]
    fn 测试思考级别条目的序列化往返() {
        let entry = ThinkingLevelEntry {
            level: "high".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ThinkingLevelEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.level, "high");
    }

    #[test]
    fn 测试模型切换条目的序列化往返() {
        let entry = ModelChangeEntry {
            from: Some("gpt-3.5".to_string()),
            to: "gpt-4".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ModelChangeEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.from.as_deref(), Some("gpt-3.5"));
        assert_eq!(deserialized.to, "gpt-4");
    }

    #[test]
    fn 测试标签条目的序列化往返() {
        let entry = LabelEntry {
            name: "重要节点".to_string(),
            description: Some("这里完成了核心功能".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: LabelEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "重要节点");
        assert_eq!(
            deserialized.description.as_deref(),
            Some("这里完成了核心功能")
        );
    }

    #[test]
    fn 测试模式变更条目的序列化往返() {
        let entry = ModeChangeEntry {
            from: None,
            to: "plan".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ModeChangeEntry = serde_json::from_str(&json).unwrap();
        assert!(deserialized.from.is_none());
        assert_eq!(deserialized.to, "plan");
    }

    #[test]
    fn 测试完整会话条目的序列化往返() {
        let entry = SessionEntry::message(
            None,
            MessageEntry {
                role: Role::User,
                content: "请帮我写测试".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
                model: None,
                token_usage: None,
            },
        );
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.entry_type, EntryType::Message);
        assert!(deserialized.parent_id.is_none());
        let msg = deserialized.as_message().unwrap();
        assert_eq!(msg.content, "请帮我写测试");
    }

    #[test]
    fn 测试带父条目的会话条目序列化() {
        let parent_id = EntryId::from_string("parent_001");
        let entry = SessionEntry::message(
            Some(parent_id.clone()),
            MessageEntry {
                role: Role::Assistant,
                content: "好的".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
                model: Some("claude-3".to_string()),
                token_usage: None,
            },
        );
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.parent_id.as_ref().unwrap().as_str(),
            "parent_001"
        );
    }

    #[test]
    fn 测试自定义条目数据的序列化() {
        let data = EntryData::Custom(serde_json::json!({"key": "value", "num": 42}));
        let json = serde_json::to_string(&data).unwrap();
        let deserialized: EntryData = serde_json::from_str(&json).unwrap();
        match deserialized {
            EntryData::Custom(v) => {
                assert_eq!(v["key"], "value");
                assert_eq!(v["num"], 42);
            }
            _ => panic!("应该是自定义条目数据"),
        }
    }

    #[test]
    fn 测试条目ID的序列化往返() {
        let id = EntryId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: EntryId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn 测试压缩条目创建辅助方法() {
        let entry = SessionEntry::compaction(
            Some(EntryId::from_string("parent")),
            CompactionEntry {
                summary: "摘要".to_string(),
                short_summary: None,
                first_kept_entry_id: EntryId::from_string("kept"),
                tokens_before: 1000,
            },
        );
        assert_eq!(entry.entry_type, EntryType::Compaction);
        let c = entry.as_compaction().unwrap();
        assert_eq!(c.tokens_before, 1000);
    }

    #[test]
    fn 测试as_message对非消息条目返回None() {
        let entry = SessionEntry::compaction(
            None,
            CompactionEntry {
                summary: "摘要".to_string(),
                short_summary: None,
                first_kept_entry_id: EntryId::from_string("kept"),
                tokens_before: 1000,
            },
        );
        assert!(entry.as_message().is_none());
    }
}
