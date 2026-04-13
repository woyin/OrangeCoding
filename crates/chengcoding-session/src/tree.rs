//! 会话树结构模块
//!
//! 本模块实现了基于父子关系的会话树结构。
//! 会话中的条目通过 `parent_id` 链接形成树形结构，支持分支和导航。
//! 主要功能：
//! - 从条目列表构建树
//! - 提取活跃分支（从根到当前叶子的路径）
//! - 导航到不同的分支
//! - 提取用于 AI 上下文的消息列表

use std::collections::HashMap;

use crate::entry::{EntryId, MessageEntry, SessionEntry};

// ---------------------------------------------------------------------------
// 会话树
// ---------------------------------------------------------------------------

/// 会话树 - 管理条目间的父子关系
///
/// 通过 `parent_id` 构建的树形结构，支持多分支浏览和导航。
/// `leaf_id` 标记当前活跃分支的叶子节点。
pub struct SessionTree {
    /// 所有条目（按索引存储）
    entries: Vec<SessionEntry>,
    /// 条目 ID 到索引的映射
    index: HashMap<EntryId, usize>,
    /// 子条目映射（父ID -> 子ID列表）
    children: HashMap<EntryId, Vec<EntryId>>,
    /// 根条目 ID 列表（没有 parent_id 的条目）
    roots: Vec<EntryId>,
    /// 当前活跃叶子节点的 ID
    leaf_id: Option<EntryId>,
}

impl SessionTree {
    /// 创建空的会话树
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            children: HashMap::new(),
            roots: Vec::new(),
            leaf_id: None,
        }
    }

    /// 从条目列表构建树
    ///
    /// 按顺序添加所有条目，最后一个条目成为叶子节点。
    pub fn from_entries(entries: Vec<SessionEntry>) -> Self {
        let mut tree = Self::new();
        for entry in entries {
            tree.add_entry(entry);
        }
        tree
    }

    /// 添加新条目到树中
    ///
    /// 自动维护父子关系映射，并将新条目设为叶子节点。
    pub fn add_entry(&mut self, entry: SessionEntry) {
        let id = entry.id.clone();
        let parent = entry.parent_id.clone();

        // 建立索引
        let idx = self.entries.len();
        self.index.insert(id.clone(), idx);

        // 更新父子关系
        if let Some(ref pid) = parent {
            self.children
                .entry(pid.clone())
                .or_default()
                .push(id.clone());
        } else {
            self.roots.push(id.clone());
        }

        self.entries.push(entry);
        // 新添加的条目成为叶子
        self.leaf_id = Some(id);
    }

    /// 获取从根到当前叶子的路径（活跃分支）
    ///
    /// 从叶子节点向上回溯到根，然后反转得到从根到叶子的路径。
    pub fn active_branch(&self) -> Vec<&SessionEntry> {
        let leaf = match &self.leaf_id {
            Some(id) => id,
            None => return Vec::new(),
        };

        let mut path = Vec::new();
        let mut current_id = Some(leaf.clone());

        while let Some(id) = current_id {
            if let Some(&idx) = self.index.get(&id) {
                let entry = &self.entries[idx];
                path.push(entry);
                current_id = entry.parent_id.clone();
            } else {
                break;
            }
        }

        path.reverse();
        path
    }

    /// 切换到指定条目（导航树）
    ///
    /// 将指定条目设为当前叶子节点，改变活跃分支。
    pub fn navigate_to(&mut self, entry_id: &EntryId) {
        if self.index.contains_key(entry_id) {
            self.leaf_id = Some(entry_id.clone());
        }
    }

    /// 获取指定条目的子条目
    pub fn children_of(&self, entry_id: &EntryId) -> Vec<&SessionEntry> {
        self.children
            .get(entry_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.index.get(id).map(|&idx| &self.entries[idx]))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 获取条目数量
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 判断树是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 获取当前叶子条目
    pub fn current_leaf(&self) -> Option<&SessionEntry> {
        self.leaf_id
            .as_ref()
            .and_then(|id| self.index.get(id))
            .map(|&idx| &self.entries[idx])
    }

    /// 从活跃分支提取消息条目（用于 AI 上下文）
    ///
    /// 只返回 Message 类型的条目中的 MessageEntry 引用。
    /// 如果遇到压缩条目，从压缩点之后开始提取消息。
    pub fn context_messages(&self) -> Vec<&MessageEntry> {
        let branch = self.active_branch();

        // 查找最后一个压缩条目的位置
        let start_idx = branch
            .iter()
            .rposition(|e| e.as_compaction().is_some())
            .map(|i| i) // 包含压缩条目本身（摘要作为上下文）
            .unwrap_or(0);

        branch[start_idx..]
            .iter()
            .filter_map(|e| e.as_message())
            .collect()
    }

    /// 按 ID 获取条目
    pub fn get_entry(&self, entry_id: &EntryId) -> Option<&SessionEntry> {
        self.index.get(entry_id).map(|&idx| &self.entries[idx])
    }

    /// 获取所有根条目
    pub fn roots(&self) -> Vec<&SessionEntry> {
        self.roots
            .iter()
            .filter_map(|id| self.index.get(id).map(|&idx| &self.entries[idx]))
            .collect()
    }
}

impl Default for SessionTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::entry::{CompactionEntry, EntryId, MessageEntry};
    use chengcoding_core::message::Role;

    /// 创建测试用消息条目（无父条目）
    fn root_msg(content: &str) -> SessionEntry {
        SessionEntry::message(
            None,
            MessageEntry {
                role: Role::User,
                content: content.to_string(),
                tool_calls: vec![],
                tool_call_id: None,
                model: None,
                token_usage: None,
            },
        )
    }

    /// 创建测试用消息条目（有父条目）
    fn child_msg(parent: &EntryId, content: &str, role: Role) -> SessionEntry {
        SessionEntry::message(
            Some(parent.clone()),
            MessageEntry {
                role,
                content: content.to_string(),
                tool_calls: vec![],
                tool_call_id: None,
                model: None,
                token_usage: None,
            },
        )
    }

    #[test]
    fn 测试空树() {
        let tree = SessionTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert!(tree.current_leaf().is_none());
        assert!(tree.active_branch().is_empty());
        assert!(tree.context_messages().is_empty());
    }

    #[test]
    fn 测试从线性条目构建树() {
        let e1 = root_msg("第一条");
        let e1_id = e1.id.clone();
        let e2 = child_msg(&e1_id, "回复", Role::Assistant);
        let e2_id = e2.id.clone();
        let e3 = child_msg(&e2_id, "继续", Role::User);
        let e3_id = e3.id.clone();

        let tree = SessionTree::from_entries(vec![e1, e2, e3]);
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.current_leaf().unwrap().id, e3_id);
    }

    #[test]
    fn 测试活跃分支提取() {
        let e1 = root_msg("你好");
        let e1_id = e1.id.clone();
        let e2 = child_msg(&e1_id, "你好！", Role::Assistant);
        let e2_id = e2.id.clone();
        let e3 = child_msg(&e2_id, "帮我写代码", Role::User);

        let tree = SessionTree::from_entries(vec![e1, e2, e3]);
        let branch = tree.active_branch();
        assert_eq!(branch.len(), 3);
        assert_eq!(branch[0].as_message().unwrap().content, "你好");
        assert_eq!(branch[1].as_message().unwrap().content, "你好！");
        assert_eq!(branch[2].as_message().unwrap().content, "帮我写代码");
    }

    #[test]
    fn 测试分支树结构() {
        // 构建树：e1 -> e2, e1 -> e3 (分支)
        let e1 = root_msg("根");
        let e1_id = e1.id.clone();
        let e2 = child_msg(&e1_id, "分支A", Role::Assistant);
        let e2_id = e2.id.clone();
        let e3 = child_msg(&e1_id, "分支B", Role::Assistant);
        let e3_id = e3.id.clone();

        let tree = SessionTree::from_entries(vec![e1, e2, e3]);

        // 最后添加的是 e3，所以 e3 是叶子
        assert_eq!(tree.current_leaf().unwrap().id, e3_id);

        // e1 应该有两个子条目
        let children = tree.children_of(&e1_id);
        assert_eq!(children.len(), 2);

        // 活跃分支应该是 e1 -> e3
        let branch = tree.active_branch();
        assert_eq!(branch.len(), 2);
        assert_eq!(branch[0].as_message().unwrap().content, "根");
        assert_eq!(branch[1].as_message().unwrap().content, "分支B");

        // e2 没有子条目
        assert!(tree.children_of(&e2_id).is_empty());
    }

    #[test]
    fn 测试导航到不同分支() {
        let e1 = root_msg("根");
        let e1_id = e1.id.clone();
        let e2 = child_msg(&e1_id, "分支A", Role::Assistant);
        let e2_id = e2.id.clone();
        let e3 = child_msg(&e1_id, "分支B", Role::Assistant);
        let _e3_id = e3.id.clone();

        let mut tree = SessionTree::from_entries(vec![e1, e2, e3]);

        // 导航到分支A
        tree.navigate_to(&e2_id);
        let branch = tree.active_branch();
        assert_eq!(branch.len(), 2);
        assert_eq!(branch[1].as_message().unwrap().content, "分支A");
    }

    #[test]
    fn 测试上下文消息提取_只返回消息类型() {
        let e1 = root_msg("问题");
        let e1_id = e1.id.clone();
        let e2 = child_msg(&e1_id, "回答", Role::Assistant);

        let tree = SessionTree::from_entries(vec![e1, e2]);
        let msgs = tree.context_messages();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, Role::User);
        assert_eq!(msgs[1].role, Role::Assistant);
    }

    #[test]
    fn 测试上下文消息提取_带压缩() {
        let e1 = root_msg("旧消息");
        let e1_id = e1.id.clone();

        // 添加压缩条目
        let compaction = SessionEntry::compaction(
            Some(e1_id),
            CompactionEntry {
                summary: "之前的对话摘要".to_string(),
                short_summary: None,
                first_kept_entry_id: EntryId::from_string("kept"),
                tokens_before: 5000,
            },
        );
        let comp_id = compaction.id.clone();

        let e3 = child_msg(&comp_id, "新问题", Role::User);

        let tree = SessionTree::from_entries(vec![e1, compaction, e3]);
        let msgs = tree.context_messages();
        // 应该只有压缩之后的消息（旧消息在压缩之前，被跳过）
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "新问题");
    }

    #[test]
    fn 测试按ID获取条目() {
        let e1 = root_msg("测试");
        let e1_id = e1.id.clone();
        let tree = SessionTree::from_entries(vec![e1]);

        assert!(tree.get_entry(&e1_id).is_some());
        assert!(tree.get_entry(&EntryId::from_string("不存在")).is_none());
    }

    #[test]
    fn 测试导航到不存在的条目无效果() {
        let e1 = root_msg("测试");
        let e1_id = e1.id.clone();
        let mut tree = SessionTree::from_entries(vec![e1]);

        tree.navigate_to(&EntryId::from_string("不存在"));
        // 叶子不变
        assert_eq!(tree.current_leaf().unwrap().id, e1_id);
    }

    #[test]
    fn 测试根条目列表() {
        let e1 = root_msg("根1");
        let e2 = root_msg("根2");
        let tree = SessionTree::from_entries(vec![e1, e2]);

        let roots = tree.roots();
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn 测试添加条目后叶子更新() {
        let mut tree = SessionTree::new();

        let e1 = root_msg("第一条");
        let e1_id = e1.id.clone();
        tree.add_entry(e1);
        assert_eq!(tree.current_leaf().unwrap().id, e1_id);

        let e2 = child_msg(&e1_id, "第二条", Role::Assistant);
        let e2_id = e2.id.clone();
        tree.add_entry(e2);
        assert_eq!(tree.current_leaf().unwrap().id, e2_id);
    }

    #[test]
    fn 测试深层分支导航() {
        // e1 -> e2 -> e3 -> e4
        //     -> e5 -> e6
        let e1 = root_msg("根");
        let e1_id = e1.id.clone();

        let e2 = child_msg(&e1_id, "a1", Role::Assistant);
        let e2_id = e2.id.clone();

        let e3 = child_msg(&e2_id, "a2", Role::User);
        let e3_id = e3.id.clone();

        let e4 = child_msg(&e3_id, "a3", Role::Assistant);
        let _e4_id = e4.id.clone();

        let e5 = child_msg(&e1_id, "b1", Role::Assistant);
        let e5_id = e5.id.clone();

        let e6 = child_msg(&e5_id, "b2", Role::User);
        let e6_id = e6.id.clone();

        let mut tree = SessionTree::from_entries(vec![e1, e2, e3, e4, e5, e6]);

        // 当前叶子是 e6（最后添加的）
        assert_eq!(tree.current_leaf().unwrap().id, e6_id);
        let branch = tree.active_branch();
        assert_eq!(branch.len(), 3); // e1 -> e5 -> e6

        // 导航到 e3 分支
        tree.navigate_to(&e3_id);
        let branch = tree.active_branch();
        assert_eq!(branch.len(), 3); // e1 -> e2 -> e3
        assert_eq!(branch[2].as_message().unwrap().content, "a2");
    }
}
