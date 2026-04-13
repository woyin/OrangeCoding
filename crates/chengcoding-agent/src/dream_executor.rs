//! # AutoDream 整合流程
//!
//! 四阶段记忆整合执行器。
//!
//! # 设计思想
//! 参考 reference 中 AutoDream 的设计：
//! - 四个阶段按序执行：定向 → 收集 → 整合 → 修剪
//! - 使用 Fork Agent 模式隔离执行（沙箱化）
//! - 失败时回滚到原始状态，保证数据安全
//! - DreamPhase 追踪当前进度，支持断点恢复

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Dream 阶段
// ---------------------------------------------------------------------------

/// AutoDream 执行阶段
///
/// 四阶段必须按序执行，每个阶段完成后才能进入下一个
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DreamPhase {
    /// 尚未开始
    NotStarted,
    /// Phase 1 — 定向: 读取 MEMORY.md 和记忆目录
    Orientation,
    /// Phase 2 — 收集: 读取会话日志，提取新信号
    Collection,
    /// Phase 3 — 整合: AI 驱动的合并
    Consolidation,
    /// Phase 4 — 修剪: 删除矛盾记忆，更新索引
    Pruning,
    /// 全部完成
    Completed,
    /// 执行失败（需要回滚）
    Failed(String),
}

impl DreamPhase {
    /// 获取下一个阶段
    ///
    /// NotStarted → Orientation → Collection → Consolidation → Pruning → Completed
    pub fn next(&self) -> Option<Self> {
        match self {
            Self::NotStarted => Some(Self::Orientation),
            Self::Orientation => Some(Self::Collection),
            Self::Collection => Some(Self::Consolidation),
            Self::Consolidation => Some(Self::Pruning),
            Self::Pruning => Some(Self::Completed),
            Self::Completed | Self::Failed(_) => None,
        }
    }

    /// 阶段编号（用于进度显示）
    pub fn phase_number(&self) -> Option<u8> {
        match self {
            Self::Orientation => Some(1),
            Self::Collection => Some(2),
            Self::Consolidation => Some(3),
            Self::Pruning => Some(4),
            _ => None,
        }
    }

    /// 是否可以推进
    pub fn can_advance(&self) -> bool {
        self.next().is_some()
    }

    /// 中文描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::NotStarted => "未开始",
            Self::Orientation => "Phase 1: 定向",
            Self::Collection => "Phase 2: 收集",
            Self::Consolidation => "Phase 3: 整合",
            Self::Pruning => "Phase 4: 修剪",
            Self::Completed => "已完成",
            Self::Failed(_) => "失败",
        }
    }
}

// ---------------------------------------------------------------------------
// Dream 执行器
// ---------------------------------------------------------------------------

/// Dream 执行结果
#[derive(Clone, Debug)]
pub struct DreamResult {
    /// 最终阶段
    pub final_phase: DreamPhase,
    /// 整合的记忆数量
    pub consolidated_count: usize,
    /// 删除的记忆数量
    pub pruned_count: usize,
    /// 索引是否已更新
    pub index_updated: bool,
    /// 执行日志
    pub log: Vec<String>,
}

/// Dream 执行器状态
///
/// 追踪整合流程的进度和中间状态
#[derive(Clone, Debug)]
pub struct DreamExecutor {
    /// 当前阶段
    pub phase: DreamPhase,
    /// 记忆目录路径
    pub memories_dir: String,
    /// 执行日志
    pub log: Vec<String>,
    /// 原始文件快照（用于回滚）
    pub snapshot: Vec<FileSnapshot>,
    /// 统计：整合数量
    pub consolidated_count: usize,
    /// 统计：修剪数量
    pub pruned_count: usize,
}

/// 文件快照（用于回滚）
#[derive(Clone, Debug)]
pub struct FileSnapshot {
    /// 文件路径
    pub path: String,
    /// 文件内容
    pub content: String,
}

impl DreamExecutor {
    /// 创建新的执行器
    pub fn new(memories_dir: &str) -> Self {
        Self {
            phase: DreamPhase::NotStarted,
            memories_dir: memories_dir.to_string(),
            log: Vec::new(),
            snapshot: Vec::new(),
            consolidated_count: 0,
            pruned_count: 0,
        }
    }

    /// 推进到下一个阶段
    ///
    /// 返回新的阶段，或 None 如果无法推进
    pub fn advance(&mut self) -> Option<&DreamPhase> {
        if let Some(next) = self.phase.next() {
            self.log.push(format!(
                "阶段转换: {} → {}",
                self.phase.description(),
                next.description()
            ));
            self.phase = next;
            Some(&self.phase)
        } else {
            None
        }
    }

    /// 标记失败并触发回滚
    pub fn fail(&mut self, reason: String) {
        self.log.push(format!("执行失败: {}", reason));
        self.phase = DreamPhase::Failed(reason);
    }

    /// 保存文件快照（用于后续回滚）
    pub fn save_snapshot(&mut self, path: &str, content: &str) {
        self.snapshot.push(FileSnapshot {
            path: path.to_string(),
            content: content.to_string(),
        });
    }

    /// 获取回滚数据
    ///
    /// 失败时返回所有快照文件，调用方负责写回文件系统
    pub fn get_rollback_data(&self) -> Option<&[FileSnapshot]> {
        match &self.phase {
            DreamPhase::Failed(_) => Some(&self.snapshot),
            _ => None,
        }
    }

    /// 生成执行结果
    pub fn result(&self) -> DreamResult {
        DreamResult {
            final_phase: self.phase.clone(),
            consolidated_count: self.consolidated_count,
            pruned_count: self.pruned_count,
            index_updated: self.phase == DreamPhase::Completed,
            log: self.log.clone(),
        }
    }

    /// 是否已完成（成功或失败）
    pub fn is_finished(&self) -> bool {
        matches!(self.phase, DreamPhase::Completed | DreamPhase::Failed(_))
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_sequence() {
        let phases = [
            DreamPhase::NotStarted,
            DreamPhase::Orientation,
            DreamPhase::Collection,
            DreamPhase::Consolidation,
            DreamPhase::Pruning,
            DreamPhase::Completed,
        ];
        for i in 0..phases.len() - 1 {
            assert_eq!(phases[i].next(), Some(phases[i + 1].clone()));
        }
        assert_eq!(phases.last().unwrap().next(), None);
    }

    #[test]
    fn test_failed_has_no_next() {
        let failed = DreamPhase::Failed("error".to_string());
        assert!(failed.next().is_none());
        assert!(!failed.can_advance());
    }

    #[test]
    fn test_phase_numbers() {
        assert_eq!(DreamPhase::Orientation.phase_number(), Some(1));
        assert_eq!(DreamPhase::Collection.phase_number(), Some(2));
        assert_eq!(DreamPhase::Consolidation.phase_number(), Some(3));
        assert_eq!(DreamPhase::Pruning.phase_number(), Some(4));
        assert_eq!(DreamPhase::NotStarted.phase_number(), None);
        assert_eq!(DreamPhase::Completed.phase_number(), None);
    }

    #[test]
    fn test_executor_new() {
        let exec = DreamExecutor::new("/tmp/memories");
        assert_eq!(exec.phase, DreamPhase::NotStarted);
        assert!(exec.log.is_empty());
        assert!(exec.snapshot.is_empty());
    }

    #[test]
    fn test_executor_full_advance() {
        let mut exec = DreamExecutor::new("/tmp/memories");
        let mut count = 0;
        while exec.advance().is_some() {
            count += 1;
        }
        assert_eq!(count, 5); // NotStarted → ... → Completed
        assert_eq!(exec.phase, DreamPhase::Completed);
    }

    #[test]
    fn test_executor_advance_logs() {
        let mut exec = DreamExecutor::new("/tmp/memories");
        exec.advance(); // → Orientation
        assert_eq!(exec.log.len(), 1);
        assert!(exec.log[0].contains("定向"));
    }

    #[test]
    fn test_executor_fail() {
        let mut exec = DreamExecutor::new("/tmp/memories");
        exec.advance(); // → Orientation
        exec.fail("测试失败".to_string());
        assert!(matches!(exec.phase, DreamPhase::Failed(_)));
        assert!(exec.is_finished());
    }

    #[test]
    fn test_snapshot_and_rollback() {
        let mut exec = DreamExecutor::new("/tmp/memories");
        exec.save_snapshot("a.md", "原始内容 A");
        exec.save_snapshot("b.md", "原始内容 B");

        // 正常状态下没有回滚数据
        assert!(exec.get_rollback_data().is_none());

        // 失败后可以获取回滚数据
        exec.fail("出错了".to_string());
        let rollback = exec.get_rollback_data().unwrap();
        assert_eq!(rollback.len(), 2);
        assert_eq!(rollback[0].path, "a.md");
        assert_eq!(rollback[0].content, "原始内容 A");
    }

    #[test]
    fn test_result_completed() {
        let mut exec = DreamExecutor::new("/tmp/memories");
        exec.consolidated_count = 3;
        exec.pruned_count = 1;
        // 推进到完成
        while exec.advance().is_some() {}
        let result = exec.result();
        assert!(result.index_updated);
        assert_eq!(result.consolidated_count, 3);
        assert_eq!(result.pruned_count, 1);
    }

    #[test]
    fn test_result_failed() {
        let mut exec = DreamExecutor::new("/tmp/memories");
        exec.fail("错误".to_string());
        let result = exec.result();
        assert!(!result.index_updated);
    }

    #[test]
    fn test_is_finished() {
        let mut exec = DreamExecutor::new("/tmp/memories");
        assert!(!exec.is_finished());
        exec.advance();
        assert!(!exec.is_finished());
        while exec.advance().is_some() {}
        assert!(exec.is_finished());
    }

    #[test]
    fn test_phase_descriptions() {
        let phases = [
            DreamPhase::NotStarted,
            DreamPhase::Orientation,
            DreamPhase::Collection,
            DreamPhase::Consolidation,
            DreamPhase::Pruning,
            DreamPhase::Completed,
            DreamPhase::Failed("err".into()),
        ];
        for phase in &phases {
            assert!(!phase.description().is_empty());
        }
    }

    #[test]
    fn test_cannot_advance_after_completed() {
        let mut exec = DreamExecutor::new("/tmp/memories");
        while exec.advance().is_some() {}
        assert!(exec.advance().is_none());
        assert_eq!(exec.phase, DreamPhase::Completed);
    }
}
