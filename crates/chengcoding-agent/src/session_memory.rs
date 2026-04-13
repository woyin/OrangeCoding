//! # 会话记忆（短期记忆）
//!
//! 在单次会话内跟踪关键信息，会话结束后自动丢弃。
//!
//! # 设计思想
//! 参考 reference 中的 session memory 概念：
//! - 短期记忆只在会话内有效，不持久化
//! - 跟踪：工作目录变化、已修改文件、关键决策、用户偏好
//! - 为上下文压缩和 autoDream 提供数据来源
//! - 容量有限，超出时按 FIFO 淘汰旧条目

use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 会话记忆条目
// ---------------------------------------------------------------------------

/// 会话记忆条目类型
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionMemoryKind {
    /// 修改过的文件路径
    FileModified,
    /// 关键决策记录
    Decision,
    /// 遇到的错误
    Error,
    /// 用户在本会话中的偏好
    SessionPreference,
    /// 工作目录变化
    DirectoryChange,
    /// 自定义标记
    Custom(String),
}

/// 会话记忆条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMemoryEntry {
    /// 条目类型
    pub kind: SessionMemoryKind,
    /// 内容
    pub content: String,
    /// 记录时的轮次
    pub turn_index: usize,
}

// ---------------------------------------------------------------------------
// 会话记忆存储
// ---------------------------------------------------------------------------

/// 会话记忆存储
///
/// 使用 VecDeque 实现 FIFO 淘汰的有限容量短期记忆。
/// 会话结束后整个实例被丢弃。
pub struct SessionMemory {
    /// 记忆条目队列
    entries: VecDeque<SessionMemoryEntry>,
    /// 最大容量
    max_entries: usize,
    /// 已修改文件集合（快速查重）
    modified_files: HashSet<String>,
}

impl SessionMemory {
    /// 创建新的会话记忆
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            modified_files: HashSet::new(),
        }
    }

    /// 使用默认容量 (200) 创建
    pub fn with_defaults() -> Self {
        Self::new(200)
    }

    /// 添加记忆条目
    ///
    /// 超出容量时淘汰最旧的条目
    pub fn add(&mut self, kind: SessionMemoryKind, content: impl Into<String>, turn_index: usize) {
        let content = content.into();

        // 如果是文件修改，更新文件集合
        if kind == SessionMemoryKind::FileModified {
            self.modified_files.insert(content.clone());
        }

        let entry = SessionMemoryEntry {
            kind,
            content,
            turn_index,
        };

        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// 记录文件修改
    pub fn record_file_modified(&mut self, path: impl Into<String>, turn_index: usize) {
        self.add(SessionMemoryKind::FileModified, path, turn_index);
    }

    /// 记录决策
    pub fn record_decision(&mut self, decision: impl Into<String>, turn_index: usize) {
        self.add(SessionMemoryKind::Decision, decision, turn_index);
    }

    /// 记录错误
    pub fn record_error(&mut self, error: impl Into<String>, turn_index: usize) {
        self.add(SessionMemoryKind::Error, error, turn_index);
    }

    /// 获取所有已修改的文件路径
    pub fn modified_files(&self) -> &HashSet<String> {
        &self.modified_files
    }

    /// 按类型过滤条目
    pub fn by_kind(&self, kind: &SessionMemoryKind) -> Vec<&SessionMemoryEntry> {
        self.entries.iter().filter(|e| &e.kind == kind).collect()
    }

    /// 获取最近 N 条记录
    pub fn recent(&self, n: usize) -> Vec<&SessionMemoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// 获取指定轮次之后的所有记录
    pub fn since_turn(&self, turn: usize) -> Vec<&SessionMemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.turn_index >= turn)
            .collect()
    }

    /// 总条目数
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// 生成会话摘要（用于 autoDream 的输入）
    ///
    /// 返回本会话的关键信息概要
    pub fn generate_summary(&self) -> String {
        let mut lines = Vec::new();

        // 修改的文件
        if !self.modified_files.is_empty() {
            lines.push("修改的文件:".to_string());
            for f in &self.modified_files {
                lines.push(format!("  - {}", f));
            }
        }

        // 关键决策
        let decisions = self.by_kind(&SessionMemoryKind::Decision);
        if !decisions.is_empty() {
            lines.push("关键决策:".to_string());
            for d in &decisions {
                lines.push(format!("  - {}", d.content));
            }
        }

        // 错误记录
        let errors = self.by_kind(&SessionMemoryKind::Error);
        if !errors.is_empty() {
            lines.push(format!("遇到 {} 个错误", errors.len()));
        }

        if lines.is_empty() {
            "本会话无重要记录".to_string()
        } else {
            lines.join("\n")
        }
    }

    /// 清除所有记录
    pub fn clear(&mut self) {
        self.entries.clear();
        self.modified_files.clear();
    }
}

impl Default for SessionMemory {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let mem = SessionMemory::new(100);
        assert_eq!(mem.count(), 0);
        assert!(mem.modified_files().is_empty());
    }

    #[test]
    fn test_add_and_count() {
        let mut mem = SessionMemory::new(100);
        mem.add(SessionMemoryKind::Decision, "使用 Rust", 0);
        assert_eq!(mem.count(), 1);
    }

    #[test]
    fn test_record_file_modified() {
        let mut mem = SessionMemory::new(100);
        mem.record_file_modified("src/main.rs", 0);
        mem.record_file_modified("src/lib.rs", 1);

        assert_eq!(mem.modified_files().len(), 2);
        assert!(mem.modified_files().contains("src/main.rs"));
        assert!(mem.modified_files().contains("src/lib.rs"));
    }

    #[test]
    fn test_file_modified_dedup() {
        let mut mem = SessionMemory::new(100);
        mem.record_file_modified("src/main.rs", 0);
        mem.record_file_modified("src/main.rs", 1);

        // 条目有 2 条（记录了两次修改）
        assert_eq!(mem.count(), 2);
        // 但文件集合只有 1 个（去重）
        assert_eq!(mem.modified_files().len(), 1);
    }

    #[test]
    fn test_record_decision() {
        let mut mem = SessionMemory::new(100);
        mem.record_decision("使用 TDD 开发", 0);

        let decisions = mem.by_kind(&SessionMemoryKind::Decision);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].content, "使用 TDD 开发");
    }

    #[test]
    fn test_record_error() {
        let mut mem = SessionMemory::new(100);
        mem.record_error("编译失败: 类型不匹配", 3);

        let errors = mem.by_kind(&SessionMemoryKind::Error);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].turn_index, 3);
    }

    #[test]
    fn test_fifo_eviction() {
        let mut mem = SessionMemory::new(3);
        mem.add(SessionMemoryKind::Decision, "第一条", 0);
        mem.add(SessionMemoryKind::Decision, "第二条", 1);
        mem.add(SessionMemoryKind::Decision, "第三条", 2);
        assert_eq!(mem.count(), 3);

        // 第四条应淘汰最旧的
        mem.add(SessionMemoryKind::Decision, "第四条", 3);
        assert_eq!(mem.count(), 3);

        let all: Vec<&str> = mem.entries.iter().map(|e| e.content.as_str()).collect();
        assert!(!all.contains(&"第一条"), "最旧条目应被淘汰");
        assert!(all.contains(&"第四条"));
    }

    #[test]
    fn test_recent() {
        let mut mem = SessionMemory::new(100);
        mem.add(SessionMemoryKind::Decision, "旧", 0);
        mem.add(SessionMemoryKind::Decision, "中", 1);
        mem.add(SessionMemoryKind::Decision, "新", 2);

        let recent = mem.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].content, "新");
        assert_eq!(recent[1].content, "中");
    }

    #[test]
    fn test_since_turn() {
        let mut mem = SessionMemory::new(100);
        mem.add(SessionMemoryKind::Decision, "轮次0", 0);
        mem.add(SessionMemoryKind::Decision, "轮次2", 2);
        mem.add(SessionMemoryKind::Decision, "轮次5", 5);

        let since_2 = mem.since_turn(2);
        assert_eq!(since_2.len(), 2);
        assert_eq!(since_2[0].content, "轮次2");
        assert_eq!(since_2[1].content, "轮次5");
    }

    #[test]
    fn test_by_kind() {
        let mut mem = SessionMemory::new(100);
        mem.record_file_modified("a.rs", 0);
        mem.record_decision("决定", 1);
        mem.record_error("错误", 2);
        mem.record_file_modified("b.rs", 3);

        assert_eq!(mem.by_kind(&SessionMemoryKind::FileModified).len(), 2);
        assert_eq!(mem.by_kind(&SessionMemoryKind::Decision).len(), 1);
        assert_eq!(mem.by_kind(&SessionMemoryKind::Error).len(), 1);
    }

    #[test]
    fn test_generate_summary() {
        let mut mem = SessionMemory::new(100);
        mem.record_file_modified("src/main.rs", 0);
        mem.record_decision("使用 Rust 实现", 1);
        mem.record_error("编译失败", 2);

        let summary = mem.generate_summary();
        assert!(summary.contains("修改的文件"));
        assert!(summary.contains("src/main.rs"));
        assert!(summary.contains("关键决策"));
        assert!(summary.contains("使用 Rust 实现"));
        assert!(summary.contains("1 个错误"));
    }

    #[test]
    fn test_generate_summary_empty() {
        let mem = SessionMemory::new(100);
        let summary = mem.generate_summary();
        assert_eq!(summary, "本会话无重要记录");
    }

    #[test]
    fn test_clear() {
        let mut mem = SessionMemory::new(100);
        mem.record_file_modified("a.rs", 0);
        mem.record_decision("决定", 1);

        mem.clear();
        assert_eq!(mem.count(), 0);
        assert!(mem.modified_files().is_empty());
    }

    #[test]
    fn test_custom_kind() {
        let mut mem = SessionMemory::new(100);
        mem.add(SessionMemoryKind::Custom("bookmark".into()), "重要位置", 0);

        let custom = mem.by_kind(&SessionMemoryKind::Custom("bookmark".into()));
        assert_eq!(custom.len(), 1);
    }

    #[test]
    fn test_with_defaults() {
        let mem = SessionMemory::with_defaults();
        assert_eq!(mem.max_entries, 200);
    }
}
