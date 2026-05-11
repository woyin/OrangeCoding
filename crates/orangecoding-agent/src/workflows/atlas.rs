//! # Atlas 执行编排工作流
//!
//! 负责将 Prometheus 生成的计划拆解为可执行的任务单元，
//! 委派给合适的 Agent 执行，并在执行过程中积累智慧。
//!
//! ## 核心职责
//!
//! - 加载并解析计划文档
//! - 分析任务间依赖关系
//! - 按类别+技能委派任务
//! - 跨任务积累与应用学习成果
//! - 生成最终执行报告

use serde::{Deserialize, Serialize};

use super::prometheus::PlanDocument;

// ============================================================
// 智慧积累
// ============================================================

/// 跨任务积累的智慧——约定、经验、教训
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wisdom {
    /// 发现的项目约定
    pub conventions: Vec<String>,
    /// 成功经验
    pub successes: Vec<String>,
    /// 失败教训
    pub failures: Vec<String>,
    /// 需要注意的陷阱
    pub gotchas: Vec<String>,
    /// 有用的命令
    pub commands: Vec<String>,
}

impl Wisdom {
    /// 创建空的智慧实例
    pub fn new() -> Self {
        Self::default()
    }

    /// 智慧条目总数
    pub fn total_entries(&self) -> usize {
        self.conventions.len()
            + self.successes.len()
            + self.failures.len()
            + self.gotchas.len()
            + self.commands.len()
    }
}

// ============================================================
// 记事本
// ============================================================

/// 执行过程中的五区域记事本
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notepad {
    /// 学到的知识
    pub learnings: Vec<String>,
    /// 做出的决策
    pub decisions: Vec<String>,
    /// 发现的问题
    pub issues: Vec<String>,
    /// 验证结果
    pub verification: Vec<String>,
    /// 待解决的难题
    pub problems: Vec<String>,
}

impl Notepad {
    /// 创建空记事本
    pub fn new() -> Self {
        Self::default()
    }

    /// 记事本条目总数
    pub fn total_entries(&self) -> usize {
        self.learnings.len()
            + self.decisions.len()
            + self.issues.len()
            + self.verification.len()
            + self.problems.len()
    }
}

// ============================================================
// 任务委派
// ============================================================

/// 任务委派描述
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDelegation {
    /// 对应的计划任务 ID
    pub task_id: String,
    /// 任务类别（如 "code", "test", "docs"）
    pub category: String,
    /// 所需技能列表
    pub skills: Vec<String>,
    /// 委派给 Agent 的提示词
    pub prompt: String,
    /// 附加上下文
    pub context: Vec<String>,
}

// ============================================================
// 任务执行状态
// ============================================================

/// 单个任务的执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    InProgress,
    /// 执行成功
    Completed,
    /// 执行失败
    Failed,
    /// 被跳过（依赖未满足）
    Skipped,
}

// ============================================================
// Atlas 编排器
// ============================================================

/// Atlas 执行编排器
///
/// 管理计划的逐任务执行，委派给合适的 Agent，
/// 并在整个过程中积累智慧和记录关键信息。
#[derive(Debug, Clone)]
pub struct AtlasOrchestrator {
    /// 当前活跃的计划文档
    active_plan: Option<PlanDocument>,
    /// 当前执行到的任务索引
    current_task_index: usize,
    /// 跨任务积累的智慧
    accumulated_wisdom: Wisdom,
    /// 执行记事本
    notepad: Notepad,
    /// 各任务执行状态
    task_statuses: Vec<TaskStatus>,
}

impl AtlasOrchestrator {
    /// 创建新的 Atlas 编排器
    pub fn new() -> Self {
        Self {
            active_plan: None,
            current_task_index: 0,
            accumulated_wisdom: Wisdom::new(),
            notepad: Notepad::new(),
            task_statuses: Vec::new(),
        }
    }

    /// 加载计划文档
    pub fn load_plan(&mut self, plan: PlanDocument) {
        let task_count = plan.tasks.len();
        self.active_plan = Some(plan);
        self.current_task_index = 0;
        self.task_statuses = vec![TaskStatus::Pending; task_count];
    }

    /// 获取当前活跃的计划
    pub fn active_plan(&self) -> Option<&PlanDocument> {
        self.active_plan.as_ref()
    }

    /// 获取当前任务索引
    pub fn current_task_index(&self) -> usize {
        self.current_task_index
    }

    /// 获取智慧引用
    pub fn wisdom(&self) -> &Wisdom {
        &self.accumulated_wisdom
    }

    /// 获取记事本引用
    pub fn notepad(&self) -> &Notepad {
        &self.notepad
    }

    /// 分析任务依赖关系，返回当前可执行的任务索引列表
    pub fn analyze_dependencies(&self) -> Vec<usize> {
        let plan = match &self.active_plan {
            Some(p) => p,
            None => return vec![],
        };

        let mut ready = Vec::new();
        for (i, task) in plan.tasks.iter().enumerate() {
            if self.task_statuses.get(i) != Some(&TaskStatus::Pending) {
                continue;
            }

            // 检查所有依赖是否已完成
            let deps_met = task.depends_on.iter().all(|dep_id| {
                plan.tasks.iter().enumerate().any(|(j, t)| {
                    t.id == *dep_id && self.task_statuses.get(j) == Some(&TaskStatus::Completed)
                })
            });

            if deps_met {
                ready.push(i);
            }
        }
        ready
    }

    /// 为指定任务创建委派描述
    pub fn delegate_task(
        &self,
        task_index: usize,
        category: impl Into<String>,
        skills: Vec<String>,
    ) -> Option<TaskDelegation> {
        let plan = self.active_plan.as_ref()?;
        let task = plan.tasks.get(task_index)?;

        Some(TaskDelegation {
            task_id: task.id.clone(),
            category: category.into(),
            skills,
            prompt: format!("执行任务「{}」：{}", task.title, task.description),
            context: task.file_references.clone(),
        })
    }

    /// 记录任务执行结果并推进
    pub fn verify_result(&mut self, task_index: usize, success: bool) -> bool {
        if task_index >= self.task_statuses.len() {
            return false;
        }

        self.task_statuses[task_index] = if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };

        // 自动推进到下一个待执行的任务
        if success && task_index == self.current_task_index {
            self.current_task_index += 1;
        }

        success
    }

    /// 从任务执行结果中提取智慧
    pub fn extract_wisdom(
        &mut self,
        convention: Option<String>,
        success: Option<String>,
        failure: Option<String>,
        gotcha: Option<String>,
        command: Option<String>,
    ) {
        if let Some(c) = convention {
            self.accumulated_wisdom.conventions.push(c);
        }
        if let Some(s) = success {
            self.accumulated_wisdom.successes.push(s);
        }
        if let Some(f) = failure {
            self.accumulated_wisdom.failures.push(f);
        }
        if let Some(g) = gotcha {
            self.accumulated_wisdom.gotchas.push(g);
        }
        if let Some(cmd) = command {
            self.accumulated_wisdom.commands.push(cmd);
        }
    }

    /// 生成执行报告
    pub fn generate_report(&self) -> String {
        let plan_name = self.active_plan.as_ref().map_or("无计划", |p| &p.name);

        let total = self.task_statuses.len();
        let completed = self
            .task_statuses
            .iter()
            .filter(|s| **s == TaskStatus::Completed)
            .count();
        let failed = self
            .task_statuses
            .iter()
            .filter(|s| **s == TaskStatus::Failed)
            .count();

        format!(
            "# Atlas 执行报告\n\n\
             计划: {plan_name}\n\
             总任务数: {total}\n\
             已完成: {completed}\n\
             失败: {failed}\n\
             智慧条目: {wisdom}\n\
             记事本条目: {notes}",
            wisdom = self.accumulated_wisdom.total_entries(),
            notes = self.notepad.total_entries(),
        )
    }

    /// 所有任务是否已完成或失败（无待执行任务）
    pub fn is_all_done(&self) -> bool {
        self.task_statuses.iter().all(|s| {
            *s == TaskStatus::Completed || *s == TaskStatus::Failed || *s == TaskStatus::Skipped
        })
    }
}

impl Default for AtlasOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::super::prometheus::PlanTask;
    use super::*;

    /// 辅助函数：创建包含依赖关系的测试计划
    fn test_plan() -> PlanDocument {
        let mut plan = PlanDocument::new("测试执行计划");
        plan.add_task(PlanTask {
            id: "setup".to_string(),
            title: "环境搭建".to_string(),
            description: "初始化开发环境".to_string(),
            file_references: vec![],
            acceptance_criteria: vec!["环境可用".to_string()],
            depends_on: vec![],
        });
        plan.add_task(PlanTask {
            id: "impl".to_string(),
            title: "功能实现".to_string(),
            description: "实现核心功能".to_string(),
            file_references: vec!["src/core.rs".to_string()],
            acceptance_criteria: vec!["编译通过".to_string()],
            depends_on: vec!["setup".to_string()],
        });
        plan.add_task(PlanTask {
            id: "test".to_string(),
            title: "测试编写".to_string(),
            description: "编写单元测试".to_string(),
            file_references: vec!["tests/".to_string()],
            acceptance_criteria: vec!["测试通过".to_string()],
            depends_on: vec!["impl".to_string()],
        });
        plan
    }

    #[test]
    fn 新建编排器无活跃计划() {
        let orch = AtlasOrchestrator::new();
        assert!(orch.active_plan().is_none());
        assert_eq!(orch.current_task_index(), 0);
    }

    #[test]
    fn 加载计划后状态初始化() {
        let mut orch = AtlasOrchestrator::new();
        orch.load_plan(test_plan());
        assert!(orch.active_plan().is_some());
        assert_eq!(orch.active_plan().unwrap().tasks.len(), 3);
    }

    #[test]
    fn 依赖分析_只有根任务可执行() {
        let mut orch = AtlasOrchestrator::new();
        orch.load_plan(test_plan());
        let ready = orch.analyze_dependencies();
        assert_eq!(ready, vec![0]); // 只有 "setup" 可执行
    }

    #[test]
    fn 依赖分析_完成前置后解锁后继() {
        let mut orch = AtlasOrchestrator::new();
        orch.load_plan(test_plan());
        orch.verify_result(0, true);
        let ready = orch.analyze_dependencies();
        assert_eq!(ready, vec![1]); // "impl" 解锁
    }

    #[test]
    fn 任务委派生成正确() {
        let mut orch = AtlasOrchestrator::new();
        orch.load_plan(test_plan());
        let delegation = orch
            .delegate_task(0, "code", vec!["rust".to_string()])
            .unwrap();
        assert_eq!(delegation.task_id, "setup");
        assert_eq!(delegation.category, "code");
        assert_eq!(delegation.skills, vec!["rust"]);
    }

    #[test]
    fn 超出范围的委派返回空() {
        let mut orch = AtlasOrchestrator::new();
        orch.load_plan(test_plan());
        assert!(orch.delegate_task(99, "code", vec![]).is_none());
    }

    #[test]
    fn 验证结果更新状态() {
        let mut orch = AtlasOrchestrator::new();
        orch.load_plan(test_plan());
        assert!(orch.verify_result(0, true));
        assert!(!orch.verify_result(1, false));
        assert_eq!(orch.current_task_index(), 1);
    }

    #[test]
    fn 智慧积累() {
        let mut orch = AtlasOrchestrator::new();
        orch.extract_wisdom(
            Some("使用 snake_case".to_string()),
            Some("并行测试有效".to_string()),
            None,
            Some("注意生命周期".to_string()),
            Some("cargo clippy".to_string()),
        );
        assert_eq!(orch.wisdom().total_entries(), 4);
        assert_eq!(orch.wisdom().conventions.len(), 1);
    }

    #[test]
    fn 生成报告包含关键信息() {
        let mut orch = AtlasOrchestrator::new();
        orch.load_plan(test_plan());
        orch.verify_result(0, true);
        orch.verify_result(1, false);
        let report = orch.generate_report();
        assert!(report.contains("测试执行计划"));
        assert!(report.contains("已完成: 1"));
        assert!(report.contains("失败: 1"));
    }

    #[test]
    fn 无计划时依赖分析返回空() {
        let orch = AtlasOrchestrator::new();
        assert!(orch.analyze_dependencies().is_empty());
    }

    #[test]
    fn 所有任务完成判定() {
        let mut orch = AtlasOrchestrator::new();
        orch.load_plan(test_plan());
        assert!(!orch.is_all_done());
        orch.verify_result(0, true);
        orch.verify_result(1, true);
        orch.verify_result(2, true);
        assert!(orch.is_all_done());
    }

    #[test]
    fn 默认编排器等同于新建() {
        let default_orch = AtlasOrchestrator::default();
        assert!(default_orch.active_plan().is_none());
        assert_eq!(default_orch.current_task_index(), 0);
    }
}
