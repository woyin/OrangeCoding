//! # 任务系统
//!
//! 提供任务生命周期管理：ID 生成、状态机、任务注册表。
//!
//! # 设计思想
//! 参考 reference 中 Task 系统的实现：
//! - 任务 ID 使用前缀标识类型 + 8 位随机字符（36^8 ≈ 2.8 万亿组合）
//! - 状态机保证终态不可逆转
//! - 任务注册表支持并发读写（DashMap）

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// 任务 ID
// ---------------------------------------------------------------------------

/// 任务类型前缀
///
/// 单字符前缀标识任务类型，便于快速区分和日志过滤。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskKind {
    /// Agent 任务（后台 Agent 子进程）
    Agent,
    /// Teammate 任务（进程内 Teammate）
    Teammate,
    /// Bash 任务（Shell 命令执行）
    Bash,
    /// Dream 任务（记忆整理）
    Dream,
}

impl TaskKind {
    /// 获取任务类型的单字符前缀
    pub fn prefix(&self) -> char {
        match self {
            TaskKind::Agent => 'a',
            TaskKind::Teammate => 't',
            TaskKind::Bash => 'b',
            TaskKind::Dream => 'd',
        }
    }

    /// 从前缀字符解析任务类型
    pub fn from_prefix(ch: char) -> Option<Self> {
        match ch {
            'a' => Some(TaskKind::Agent),
            't' => Some(TaskKind::Teammate),
            'b' => Some(TaskKind::Bash),
            'd' => Some(TaskKind::Dream),
            _ => None,
        }
    }
}

/// 任务唯一标识
///
/// 格式: 前缀(1 char) + 8 位随机小写字母数字
/// 示例: "a1k9x3m7p" (agent 任务), "b2j8c4n6q" (bash 任务)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    /// 生成新的任务 ID
    ///
    /// # 参数
    /// - `kind`: 任务类型，决定 ID 前缀
    pub fn new(kind: TaskKind) -> Self {
        // 使用 UUID v4 的前 8 字节（hex 编码）作为随机后缀
        // 16^8 ≈ 40 亿组合，足够避免冲突
        let uuid = Uuid::new_v4();
        let hex = format!("{:032x}", uuid.as_u128());
        let suffix = &hex[..8];

        Self(format!("{}{}", kind.prefix(), suffix))
    }

    /// 从字符串解析任务 ID（验证格式）
    pub fn parse(s: &str) -> Option<Self> {
        if s.len() != 9 {
            return None;
        }
        let prefix = s.chars().next()?;
        // 验证前缀是已知类型
        TaskKind::from_prefix(prefix)?;
        // 验证后 8 位是十六进制字符
        if s[1..]
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            Some(Self(s.to_string()))
        } else {
            None
        }
    }

    /// 获取任务类型
    pub fn kind(&self) -> TaskKind {
        let prefix = self.0.chars().next().unwrap();
        TaskKind::from_prefix(prefix).unwrap()
    }

    /// 获取 ID 字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// 任务状态
// ---------------------------------------------------------------------------

/// 任务状态
///
/// 状态机保证终态（Completed, Failed, Killed）不可逆转。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    Running,
    /// 执行完成
    Completed,
    /// 执行失败
    Failed,
    /// 被终止
    Killed,
}

impl TaskStatus {
    /// 判断是否为终态
    ///
    /// 终态不可逆转：Completed, Failed, Killed
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Killed
        )
    }

    /// 检查状态转换是否合法
    ///
    /// 终态不允许转换为任何其他状态
    pub fn can_transition_to(&self, next: TaskStatus) -> bool {
        if self.is_terminal() {
            return false;
        }
        match (self, next) {
            (TaskStatus::Pending, TaskStatus::Running) => true,
            (TaskStatus::Pending, TaskStatus::Killed) => true,
            (TaskStatus::Running, TaskStatus::Completed) => true,
            (TaskStatus::Running, TaskStatus::Failed) => true,
            (TaskStatus::Running, TaskStatus::Killed) => true,
            _ => false,
        }
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Killed => write!(f, "killed"),
        }
    }
}

// ---------------------------------------------------------------------------
// 任务状态
// ---------------------------------------------------------------------------

/// 获取当前 UNIX 时间戳（秒）
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 任务状态数据
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskState {
    /// 任务 ID
    pub id: TaskId,
    /// 当前状态
    pub status: TaskStatus,
    /// 任务标题/描述
    pub subject: String,
    /// 任务所有者（Agent ID）
    pub owner: Option<String>,
    /// 创建时间戳
    pub created_at: u64,
    /// 最后更新时间戳
    pub updated_at: u64,
    /// 可扩展的元数据
    pub metadata: HashMap<String, String>,
}

impl TaskState {
    /// 创建新的任务状态
    pub fn new(id: TaskId, subject: impl Into<String>) -> Self {
        let now = now_secs();
        Self {
            id,
            status: TaskStatus::Pending,
            subject: subject.into(),
            owner: None,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    /// 尝试转换到新状态
    ///
    /// 如果转换合法则更新状态并返回 Ok，否则返回错误信息。
    pub fn transition(&mut self, next: TaskStatus) -> Result<(), String> {
        if self.status.can_transition_to(next) {
            self.status = next;
            self.updated_at = now_secs();
            Ok(())
        } else {
            Err(format!(
                "非法状态转换: {} -> {}（任务 {}）",
                self.status, next, self.id
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// 任务注册表
// ---------------------------------------------------------------------------

/// 任务注册表 — 管理所有活跃和已完成的任务
///
/// 使用 DashMap 支持并发读写，适用于多 Agent 同时操作任务的场景。
pub struct TaskRegistry {
    tasks: DashMap<String, TaskState>,
}

impl TaskRegistry {
    /// 创建新的空注册表
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
        }
    }

    /// 注册新任务
    pub fn register(&self, state: TaskState) {
        self.tasks.insert(state.id.as_str().to_string(), state);
    }

    /// 获取任务状态
    pub fn get(&self, id: &str) -> Option<TaskState> {
        self.tasks.get(id).map(|entry| entry.value().clone())
    }

    /// 更新任务状态（状态转换）
    ///
    /// 返回转换是否成功
    pub fn transition(&self, id: &str, next: TaskStatus) -> Result<(), String> {
        match self.tasks.get_mut(id) {
            Some(mut entry) => entry.value_mut().transition(next),
            None => Err(format!("任务未找到: {}", id)),
        }
    }

    /// 列出所有任务
    pub fn list_all(&self) -> Vec<TaskState> {
        self.tasks
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 列出指定状态的任务
    pub fn list_by_status(&self, status: TaskStatus) -> Vec<TaskState> {
        self.tasks
            .iter()
            .filter(|entry| entry.value().status == status)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 删除已终止的任务，返回删除数量
    pub fn cleanup_terminal(&self) -> usize {
        let terminal_ids: Vec<String> = self
            .tasks
            .iter()
            .filter(|entry| entry.value().status.is_terminal())
            .map(|entry| entry.key().clone())
            .collect();

        let count = terminal_ids.len();
        for id in terminal_ids {
            self.tasks.remove(&id);
        }
        count
    }

    /// 获取注册表中的任务数量
    pub fn count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // -----------------------------------------------------------------------
    // TaskId 测试
    // -----------------------------------------------------------------------

    /// 测试 TaskId 格式正确：前缀 + 8 字符
    #[test]
    fn test_task_id_format() {
        let id = TaskId::new(TaskKind::Agent);
        let s = id.as_str();

        assert_eq!(s.len(), 9, "TaskId 应为 9 个字符");
        assert_eq!(s.chars().next().unwrap(), 'a', "Agent 前缀应为 'a'");

        // 后 8 位应为小写十六进制字符
        for ch in s[1..].chars() {
            assert!(
                ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase(),
                "字符 '{}' 不是小写十六进制字符",
                ch
            );
        }
    }

    /// 测试各种任务类型的前缀
    #[test]
    fn test_task_id_prefixes() {
        assert!(TaskId::new(TaskKind::Agent).as_str().starts_with('a'));
        assert!(TaskId::new(TaskKind::Teammate).as_str().starts_with('t'));
        assert!(TaskId::new(TaskKind::Bash).as_str().starts_with('b'));
        assert!(TaskId::new(TaskKind::Dream).as_str().starts_with('d'));
    }

    /// 测试 TaskId 唯一性（1000 次生成无重复）
    #[test]
    fn test_task_id_uniqueness() {
        let mut ids = HashSet::new();
        for _ in 0..1000 {
            let id = TaskId::new(TaskKind::Agent);
            assert!(ids.insert(id.as_str().to_string()), "TaskId 应唯一");
        }
    }

    /// 测试 TaskId 解析 — 合法输入
    #[test]
    fn test_task_id_parse_valid() {
        let id = TaskId::parse("a12345678");
        assert!(id.is_some());
        assert_eq!(id.unwrap().kind(), TaskKind::Agent);

        let id = TaskId::parse("babcdef01");
        assert!(id.is_some());
        assert_eq!(id.unwrap().kind(), TaskKind::Bash);
    }

    /// 测试 TaskId 解析 — 非法输入
    #[test]
    fn test_task_id_parse_invalid() {
        assert!(TaskId::parse("").is_none(), "空字符串");
        assert!(TaskId::parse("a1234567").is_none(), "太短");
        assert!(TaskId::parse("a123456789").is_none(), "太长");
        assert!(TaskId::parse("x12345678").is_none(), "未知前缀");
        assert!(TaskId::parse("aABCDEFGH").is_none(), "大写字母");
        assert!(TaskId::parse("azzzzzzgg").is_none(), "g 不是十六进制");
    }

    /// 测试 TaskId 的 kind() 方法
    #[test]
    fn test_task_id_kind() {
        let id = TaskId::new(TaskKind::Dream);
        assert_eq!(id.kind(), TaskKind::Dream);
    }

    /// 测试 TaskId 的 Display 实现
    #[test]
    fn test_task_id_display() {
        let id = TaskId::new(TaskKind::Agent);
        let s = format!("{}", id);
        assert_eq!(s, id.as_str());
    }

    // -----------------------------------------------------------------------
    // TaskKind 测试
    // -----------------------------------------------------------------------

    /// 测试 TaskKind 前缀往返转换
    #[test]
    fn test_task_kind_roundtrip() {
        let kinds = [
            TaskKind::Agent,
            TaskKind::Teammate,
            TaskKind::Bash,
            TaskKind::Dream,
        ];
        for kind in kinds {
            let prefix = kind.prefix();
            let parsed = TaskKind::from_prefix(prefix);
            assert_eq!(parsed, Some(kind));
        }
    }

    /// 测试未知前缀返回 None
    #[test]
    fn test_task_kind_unknown_prefix() {
        assert!(TaskKind::from_prefix('z').is_none());
        assert!(TaskKind::from_prefix('A').is_none());
    }

    // -----------------------------------------------------------------------
    // TaskStatus 测试
    // -----------------------------------------------------------------------

    /// 测试终态判断
    #[test]
    fn test_task_status_terminal() {
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Running.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Killed.is_terminal());
    }

    /// 测试合法状态转换
    #[test]
    fn test_valid_transitions() {
        assert!(TaskStatus::Pending.can_transition_to(TaskStatus::Running));
        assert!(TaskStatus::Pending.can_transition_to(TaskStatus::Killed));
        assert!(TaskStatus::Running.can_transition_to(TaskStatus::Completed));
        assert!(TaskStatus::Running.can_transition_to(TaskStatus::Failed));
        assert!(TaskStatus::Running.can_transition_to(TaskStatus::Killed));
    }

    /// 测试终态不可逆转
    #[test]
    fn test_terminal_no_transition() {
        let terminals = [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Killed,
        ];
        let all = [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Killed,
        ];

        for terminal in terminals {
            for target in all {
                assert!(
                    !terminal.can_transition_to(target),
                    "{} -> {} 不应允许",
                    terminal,
                    target
                );
            }
        }
    }

    /// 测试非法状态转换
    #[test]
    fn test_invalid_transitions() {
        // Pending 不能直接到 Completed/Failed
        assert!(!TaskStatus::Pending.can_transition_to(TaskStatus::Completed));
        assert!(!TaskStatus::Pending.can_transition_to(TaskStatus::Failed));

        // Running 不能回到 Pending
        assert!(!TaskStatus::Running.can_transition_to(TaskStatus::Pending));
    }

    /// 测试 TaskStatus 的 Display 实现
    #[test]
    fn test_task_status_display() {
        assert_eq!(format!("{}", TaskStatus::Pending), "pending");
        assert_eq!(format!("{}", TaskStatus::Running), "running");
        assert_eq!(format!("{}", TaskStatus::Completed), "completed");
        assert_eq!(format!("{}", TaskStatus::Failed), "failed");
        assert_eq!(format!("{}", TaskStatus::Killed), "killed");
    }

    // -----------------------------------------------------------------------
    // TaskState 测试
    // -----------------------------------------------------------------------

    /// 测试 TaskState 创建
    #[test]
    fn test_task_state_new() {
        let id = TaskId::new(TaskKind::Agent);
        let state = TaskState::new(id.clone(), "测试任务");

        assert_eq!(state.id, id);
        assert_eq!(state.status, TaskStatus::Pending);
        assert_eq!(state.subject, "测试任务");
        assert!(state.owner.is_none());
        assert!(state.created_at > 0);
        assert_eq!(state.created_at, state.updated_at);
    }

    /// 测试合法的状态转换
    #[test]
    fn test_task_state_transition_valid() {
        let id = TaskId::new(TaskKind::Agent);
        let mut state = TaskState::new(id, "测试");

        assert!(state.transition(TaskStatus::Running).is_ok());
        assert_eq!(state.status, TaskStatus::Running);

        assert!(state.transition(TaskStatus::Completed).is_ok());
        assert_eq!(state.status, TaskStatus::Completed);
    }

    /// 测试非法的状态转换
    #[test]
    fn test_task_state_transition_invalid() {
        let id = TaskId::new(TaskKind::Agent);
        let mut state = TaskState::new(id, "测试");

        // Pending -> Completed 不合法
        let err = state.transition(TaskStatus::Completed);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("非法状态转换"));

        // 状态不应改变
        assert_eq!(state.status, TaskStatus::Pending);
    }

    /// 测试终态后不允许转换
    #[test]
    fn test_task_state_terminal_no_transition() {
        let id = TaskId::new(TaskKind::Agent);
        let mut state = TaskState::new(id, "测试");

        state.transition(TaskStatus::Running).unwrap();
        state.transition(TaskStatus::Failed).unwrap();

        // 终态后任何转换都不应被允许
        assert!(state.transition(TaskStatus::Running).is_err());
        assert!(state.transition(TaskStatus::Pending).is_err());
    }

    // -----------------------------------------------------------------------
    // TaskRegistry 测试
    // -----------------------------------------------------------------------

    /// 测试注册和查询任务
    #[test]
    fn test_registry_register_and_get() {
        let registry = TaskRegistry::new();
        let id = TaskId::new(TaskKind::Agent);
        let id_str = id.as_str().to_string();
        let state = TaskState::new(id, "测试任务");

        registry.register(state);

        let retrieved = registry.get(&id_str);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().subject, "测试任务");
    }

    /// 测试查询不存在的任务
    #[test]
    fn test_registry_get_nonexistent() {
        let registry = TaskRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    /// 测试状态转换
    #[test]
    fn test_registry_transition() {
        let registry = TaskRegistry::new();
        let id = TaskId::new(TaskKind::Agent);
        let id_str = id.as_str().to_string();
        let state = TaskState::new(id, "测试");

        registry.register(state);

        assert!(registry.transition(&id_str, TaskStatus::Running).is_ok());
        assert_eq!(registry.get(&id_str).unwrap().status, TaskStatus::Running);
    }

    /// 测试对不存在的任务进行转换
    #[test]
    fn test_registry_transition_nonexistent() {
        let registry = TaskRegistry::new();
        let result = registry.transition("nonexistent", TaskStatus::Running);
        assert!(result.is_err());
    }

    /// 测试列出所有任务
    #[test]
    fn test_registry_list_all() {
        let registry = TaskRegistry::new();

        for i in 0..3 {
            let id = TaskId::new(TaskKind::Agent);
            let state = TaskState::new(id, format!("任务 {}", i));
            registry.register(state);
        }

        assert_eq!(registry.list_all().len(), 3);
    }

    /// 测试按状态过滤
    #[test]
    fn test_registry_list_by_status() {
        let registry = TaskRegistry::new();

        let id1 = TaskId::new(TaskKind::Agent);
        let id1_str = id1.as_str().to_string();
        registry.register(TaskState::new(id1, "任务1"));

        let id2 = TaskId::new(TaskKind::Agent);
        let id2_str = id2.as_str().to_string();
        registry.register(TaskState::new(id2, "任务2"));

        // 转换第一个任务为 Running
        registry.transition(&id1_str, TaskStatus::Running).unwrap();

        let pending = registry.list_by_status(TaskStatus::Pending);
        assert_eq!(pending.len(), 1);

        let running = registry.list_by_status(TaskStatus::Running);
        assert_eq!(running.len(), 1);
    }

    /// 测试清理终态任务
    #[test]
    fn test_registry_cleanup_terminal() {
        let registry = TaskRegistry::new();

        let id1 = TaskId::new(TaskKind::Agent);
        let id1_str = id1.as_str().to_string();
        registry.register(TaskState::new(id1, "活跃任务"));

        let id2 = TaskId::new(TaskKind::Agent);
        let id2_str = id2.as_str().to_string();
        registry.register(TaskState::new(id2, "已完成任务"));

        // 完成第二个任务
        registry.transition(&id2_str, TaskStatus::Running).unwrap();
        registry
            .transition(&id2_str, TaskStatus::Completed)
            .unwrap();

        assert_eq!(registry.count(), 2);

        let cleaned = registry.cleanup_terminal();
        assert_eq!(cleaned, 1);
        assert_eq!(registry.count(), 1);

        // 剩余的应是活跃任务
        assert!(registry.get(&id1_str).is_some());
        assert!(registry.get(&id2_str).is_none());
    }

    /// 测试注册表计数
    #[test]
    fn test_registry_count() {
        let registry = TaskRegistry::new();
        assert_eq!(registry.count(), 0);

        registry.register(TaskState::new(TaskId::new(TaskKind::Agent), "任务"));
        assert_eq!(registry.count(), 1);
    }
}
