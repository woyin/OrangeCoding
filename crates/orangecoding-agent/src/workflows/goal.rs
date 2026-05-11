//! # Goal 自主迭代循环
//!
//! 实现 Planning → Executing → Verifying → Replan/Done 的循环执行模式。
//! 用户输入目标需求，系统自动规划、执行、验证，直到目标完成。
//!
//! ## 触发方式
//!
//! - CLI: `orangecoding launch --goal "需求描述"`
//! - 斜杠命令: `/goal 需求描述`
//!
//! ## 循环流程
//!
//! ```text
//! Planning → Executing → Verifying → Done
//!     ▲                         │
//!     │      验证失败           │
//!     └─────────────────────────┘
//!                        │ 验证通过
//!                        ▼
//!                      Done
//! ```

use crate::harness::{HarnessAction, MissionContract};
use chrono::Utc;
use serde::{Deserialize, Serialize};

// ============================================================
// Goal 阶段
// ============================================================

/// Goal 执行阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GoalPhase {
    /// 生成/重新生成执行计划
    Planning,
    /// 按计划执行任务
    Executing,
    /// 验证执行结果
    Verifying,
    /// 全部完成
    Done,
}

impl GoalPhase {
    /// 返回阶段的中文显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Planning => "制定计划",
            Self::Executing => "执行任务",
            Self::Verifying => "验证结果",
            Self::Done => "已完成",
        }
    }
}

// ============================================================
// Goal 配置
// ============================================================

/// Goal 模式配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalConfig {
    /// 最大循环次数
    pub max_cycles: u32,
    /// 每个任务内部的 AI 初始迭代软预算
    pub task_max_iterations: u32,
    /// 自定义验证命令
    pub verify_commands: Vec<String>,
    /// 每轮完成后是否自动 git commit
    pub auto_commit_per_cycle: bool,
    /// 完成标记字符串
    pub completion_promise: String,
    /// 是否启用漂移检测
    pub enable_drift_detection: bool,
    /// 是否启用自动回滚（验证失败时）
    pub enable_auto_rollback: bool,
    /// 是否启用不变量验证
    pub enable_invariant_verification: bool,
}

impl Default for GoalConfig {
    fn default() -> Self {
        Self {
            max_cycles: 20,
            task_max_iterations: 30,
            verify_commands: vec!["cargo test".to_string()],
            auto_commit_per_cycle: true,
            completion_promise: "GOAL_COMPLETE".to_string(),
            enable_drift_detection: true,
            enable_auto_rollback: true,
            enable_invariant_verification: true,
        }
    }
}

// ============================================================
// Goal 任务与计划
// ============================================================

/// 任务执行状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalTaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed { reason: String },
    Skipped { reason: String },
}

/// 单个 Goal 任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub target_files: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub depends_on: Vec<String>,
    pub status: GoalTaskStatus,
}

/// Goal 执行计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalPlan {
    pub requirement: String,
    pub tasks: Vec<GoalTask>,
    pub acceptance_criteria: Vec<String>,
    pub cycle: u32,
    pub forbidden_detours: Vec<String>,
    pub context: String,
}

// ============================================================
// 验证结果类型
// ============================================================

/// 单条命令的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub passed: bool,
}

/// 单条验收标准的验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriteriaResult {
    pub criterion: String,
    pub satisfied: bool,
    pub evidence: String,
}

/// 一次验证周期的完整报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub cycle: u32,
    pub command_results: Vec<CommandResult>,
    pub criteria_results: Vec<CriteriaResult>,
    pub passed: bool,
    pub failure_summary: Option<String>,
    pub suggestions: Vec<String>,
}

// ============================================================
// GoalMode 状态机
// ============================================================

/// Goal 自主迭代状态机
///
/// 控制 Planning → Executing → Verifying → Done 的循环流程，
/// 支持验证失败后自动重计划和漂移检测。
#[derive(Debug, Clone)]
pub struct GoalMode {
    is_active: bool,
    phase: GoalPhase,
    config: GoalConfig,
    current_cycle: u32,
    plan: Option<GoalPlan>,
    requirement: String,
    mission_contract: Option<MissionContract>,
    last_verification: Option<VerificationReport>,
}

impl GoalMode {
    /// 创建新的 GoalMode，使用默认配置，初始状态为未激活、Planning 阶段。
    pub fn new(requirement: String) -> Self {
        Self {
            is_active: false,
            phase: GoalPhase::Planning,
            config: GoalConfig::default(),
            current_cycle: 0,
            plan: None,
            requirement,
            mission_contract: None,
            last_verification: None,
        }
    }

    /// 使用自定义配置创建 GoalMode。
    pub fn with_config(requirement: String, config: GoalConfig) -> Self {
        Self {
            is_active: false,
            phase: GoalPhase::Planning,
            config,
            current_cycle: 0,
            plan: None,
            requirement,
            mission_contract: None,
            last_verification: None,
        }
    }

    /// 激活状态机，进入 Planning 阶段。
    pub fn activate(&mut self) {
        self.is_active = true;
        self.phase = GoalPhase::Planning;
        self.current_cycle = 0;
    }

    /// 停用状态机。
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// 推进到下一阶段。
    ///
    /// 返回 `true` 表示成功推进，`false` 表示无法推进（已完成或未激活）。
    ///
    /// 阶段转换逻辑：
    /// - Planning → Executing
    /// - Executing → Verifying
    /// - Verifying: 通过 → Done；失败 → 延长预算并重计划
    /// - Done → 不推进
    pub fn advance(&mut self) -> bool {
        if !self.is_active {
            return false;
        }

        match self.phase {
            GoalPhase::Planning => {
                self.phase = GoalPhase::Executing;
                true
            }
            GoalPhase::Executing => {
                self.phase = GoalPhase::Verifying;
                true
            }
            GoalPhase::Verifying => {
                let passed = self.last_verification.as_ref().map_or(false, |v| v.passed);

                if passed {
                    self.phase = GoalPhase::Done;
                    self.is_active = false;
                    true
                } else {
                    self.current_cycle += 1;
                    if self.current_cycle > self.config.max_cycles {
                        self.extend_cycle_budget();
                    }
                    self.phase = GoalPhase::Planning;
                    true
                }
            }
            GoalPhase::Done => false,
        }
    }

    /// 处理漂移检测的动作。
    ///
    /// 返回 `true` 表示采取了行动，`false` 表示无操作。
    pub fn handle_drift(&mut self, action: HarnessAction) -> bool {
        if !self.config.enable_drift_detection {
            return false;
        }

        match action {
            HarnessAction::Continue => false,
            HarnessAction::Replan { .. } => {
                self.phase = GoalPhase::Planning;
                self.current_cycle += 1;
                true
            }
            HarnessAction::Escalate { .. } => {
                self.deactivate();
                true
            }
        }
    }

    /// 延长循环预算，增加 `(max_cycles + 1) / 2`，最少 1。
    fn extend_cycle_budget(&mut self) {
        let extension = (self.config.max_cycles + 1) / 2;
        let extension = extension.max(1);
        self.config.max_cycles += extension;
    }

    // ----------------------------------------------------------
    // Getters
    // ----------------------------------------------------------

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn current_phase(&self) -> GoalPhase {
        self.phase
    }

    pub fn current_cycle(&self) -> u32 {
        self.current_cycle
    }

    pub fn config(&self) -> &GoalConfig {
        &self.config
    }

    pub fn requirement(&self) -> &str {
        &self.requirement
    }

    pub fn plan(&self) -> Option<&GoalPlan> {
        self.plan.as_ref()
    }

    pub fn last_verification(&self) -> Option<&VerificationReport> {
        self.last_verification.as_ref()
    }

    pub fn mission_contract(&self) -> Option<&MissionContract> {
        self.mission_contract.as_ref()
    }

    // ----------------------------------------------------------
    // Setters
    // ----------------------------------------------------------

    pub fn set_plan(&mut self, plan: GoalPlan) {
        self.plan = Some(plan);
    }

    pub fn set_verification(&mut self, report: VerificationReport) {
        self.last_verification = Some(report);
    }

    pub fn set_mission_contract(&mut self, contract: MissionContract) {
        self.mission_contract = Some(contract);
    }

    // ----------------------------------------------------------
    // Utility
    // ----------------------------------------------------------

    /// 返回状态摘要字符串。
    ///
    /// 格式：`"Goal [phase] Cycle cycle/max_cycles — Tasks: done/total"`
    /// 或 `"Goal [phase] Cycle cycle/max_cycles — No plan yet"`
    pub fn status_summary(&self) -> String {
        let phase_name = self.phase.display_name();
        if let Some(ref plan) = self.plan {
            let done = plan
                .tasks
                .iter()
                .filter(|t| t.status == GoalTaskStatus::Completed)
                .count();
            let total = plan.tasks.len();
            format!(
                "Goal [{}] Cycle {}/{} — Tasks: {}/{}",
                phase_name, self.current_cycle, self.config.max_cycles, done, total
            )
        } else {
            format!(
                "Goal [{}] Cycle {}/{} — No plan yet",
                phase_name, self.current_cycle, self.config.max_cycles
            )
        }
    }

    /// 检查是否已完成（阶段为 Done）。
    pub fn is_complete(&self) -> bool {
        self.phase == GoalPhase::Done
    }
}

// ============================================================
// GoalState 持久化
// ============================================================

/// Goal 状态持久化文件路径
pub const GOAL_FILE_PATH: &str = ".sisyphus/goal.json";

/// Goal 持久化状态
///
/// 保存完整的 Goal 运行状态，支持跨会话恢复执行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalState {
    pub id: String,
    pub requirement: String,
    pub plan: GoalPlan,
    pub config: GoalConfig,
    pub mission_contract: Option<MissionContract>,
    pub current_phase: GoalPhase,
    pub current_task_index: usize,
    pub current_cycle: u32,
    pub session_ids: Vec<String>,
    pub started_at: String,
    pub last_checkpoint: Option<String>,
    pub last_verification: Option<VerificationReport>,
}

impl GoalState {
    /// 创建新的 GoalState。
    ///
    /// ID 格式: `goal-{timestamp_hex_12chars}{random_hex_8chars}`
    /// started_at 使用 RFC 3339 时间戳。
    pub fn new(requirement: String, config: GoalConfig, plan: GoalPlan) -> Self {
        let now = Utc::now();
        let timestamp_hex = format!("{:012x}", now.timestamp());
        let random_hex = format!("{:08x}", now.timestamp_subsec_nanos());
        let id = format!("goal-{}{}", timestamp_hex, random_hex);

        Self {
            id,
            requirement,
            plan,
            config,
            mission_contract: None,
            current_phase: GoalPhase::Planning,
            current_task_index: 0,
            current_cycle: 0,
            session_ids: Vec::new(),
            started_at: now.to_rfc3339(),
            last_checkpoint: None,
            last_verification: None,
        }
    }

    /// 序列化为 JSON 字符串。
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("序列化失败: {}", e))
    }

    /// 从 JSON 字符串反序列化。
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("反序列化失败: {}", e))
    }

    /// 生成恢复执行的提示文本。
    pub fn resume_prompt(&self) -> String {
        let done = self
            .plan
            .tasks
            .iter()
            .filter(|t| t.status == GoalTaskStatus::Completed)
            .count();
        let total = self.plan.tasks.len();

        let mut prompt = format!(
            "恢复执行目标「{}」。\n当前进度: {}/{}（第 {} 轮）\n阶段: {}",
            self.requirement,
            done,
            total,
            self.current_cycle + 1,
            self.current_phase.display_name()
        );

        if let Some(ref checkpoint) = self.last_checkpoint {
            prompt.push_str(&format!("\n上次检查点: {}", checkpoint));
        }

        if let Some(ref verification) = self.last_verification {
            if !verification.passed {
                if let Some(ref summary) = verification.failure_summary {
                    prompt.push_str(&format!("\n上次验证失败: {}", summary));
                }
            }
        }

        prompt
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 默认配置合理() {
        let config = GoalConfig::default();
        assert_eq!(config.max_cycles, 20);
        assert_eq!(config.task_max_iterations, 30);
        assert_eq!(config.verify_commands, vec!["cargo test"]);
        assert!(config.auto_commit_per_cycle);
        assert_eq!(config.completion_promise, "GOAL_COMPLETE");
        assert!(config.enable_drift_detection);
    }

    #[test]
    fn 阶段显示名称正确() {
        assert_eq!(GoalPhase::Planning.display_name(), "制定计划");
        assert_eq!(GoalPhase::Executing.display_name(), "执行任务");
        assert_eq!(GoalPhase::Verifying.display_name(), "验证结果");
        assert_eq!(GoalPhase::Done.display_name(), "已完成");
    }

    #[test]
    fn 任务状态序列化往返() {
        let status = GoalTaskStatus::Failed {
            reason: "编译错误".to_string(),
        };
        let json = serde_json::to_string(&status).expect("序列化失败");
        let deserialized: GoalTaskStatus = serde_json::from_str(&json).expect("反序列化失败");
        assert_eq!(status, deserialized);
    }

    #[test]
    fn 计划可序列化() {
        let plan = GoalPlan {
            requirement: "实现用户登录功能".to_string(),
            tasks: vec![GoalTask {
                id: "task-1".to_string(),
                title: "创建登录页面".to_string(),
                description: "实现用户登录界面".to_string(),
                target_files: vec!["src/login.rs".to_string()],
                acceptance_criteria: vec!["页面可渲染".to_string()],
                depends_on: vec![],
                status: GoalTaskStatus::Pending,
            }],
            acceptance_criteria: vec!["用户可以成功登录".to_string()],
            cycle: 1,
            forbidden_detours: vec!["不要修改数据库模式".to_string()],
            context: "项目使用 Actix-web 框架".to_string(),
        };
        let json = serde_json::to_string(&plan).expect("序列化失败");
        let deserialized: GoalPlan = serde_json::from_str(&json).expect("反序列化失败");
        assert_eq!(plan.requirement, deserialized.requirement);
        assert_eq!(plan.tasks.len(), deserialized.tasks.len());
        assert_eq!(plan.tasks[0].id, deserialized.tasks[0].id);
        assert_eq!(plan.cycle, deserialized.cycle);
        assert_eq!(plan.forbidden_detours, deserialized.forbidden_detours);
        assert_eq!(plan.context, deserialized.context);
    }

    #[test]
    fn 验证报告可序列化() {
        let report = VerificationReport {
            cycle: 2,
            command_results: vec![CommandResult {
                command: "cargo test".into(),
                exit_code: 0,
                stdout: "all passed".into(),
                stderr: String::new(),
                passed: true,
            }],
            criteria_results: vec![CriteriaResult {
                criterion: "编译通过".into(),
                satisfied: true,
                evidence: "cargo build 成功".into(),
            }],
            passed: true,
            failure_summary: None,
            suggestions: vec![],
        };

        let json = serde_json::to_string(&report).unwrap();
        let roundtrip: VerificationReport = serde_json::from_str(&json).unwrap();
        assert!(roundtrip.passed);
        assert_eq!(roundtrip.command_results.len(), 1);
        assert_eq!(roundtrip.criteria_results.len(), 1);
    }

    #[test]
    fn 验证失败报告包含摘要() {
        let report = VerificationReport {
            cycle: 1,
            command_results: vec![CommandResult {
                command: "cargo test".into(),
                exit_code: 1,
                stdout: String::new(),
                stderr: "2 tests failed".into(),
                passed: false,
            }],
            criteria_results: vec![],
            passed: false,
            failure_summary: Some("2 tests failed".into()),
            suggestions: vec!["修复 test_a".into()],
        };

        assert!(!report.passed);
        assert!(report.failure_summary.is_some());
        assert_eq!(report.suggestions.len(), 1);
    }

    // ===========================================================
    // GoalMode 状态机测试
    // ===========================================================

    #[test]
    fn 新建模式默认未激活() {
        let mode = GoalMode::new("实现功能".to_string());
        assert!(!mode.is_active());
        assert_eq!(mode.current_phase(), GoalPhase::Planning);
        assert_eq!(mode.current_cycle(), 0);
        assert!(mode.plan().is_none());
        assert!(mode.last_verification().is_none());
        assert!(mode.mission_contract().is_none());
        assert_eq!(mode.requirement(), "实现功能");
    }

    #[test]
    fn 激活后可推进() {
        let mut mode = GoalMode::new("测试目标".to_string());
        mode.activate();
        assert!(mode.is_active());
        assert_eq!(mode.current_phase(), GoalPhase::Planning);

        assert!(mode.advance());
        assert_eq!(mode.current_phase(), GoalPhase::Executing);
    }

    #[test]
    fn 未激活时无法推进() {
        let mut mode = GoalMode::new("测试目标".to_string());
        assert!(!mode.is_active());
        assert!(!mode.advance());
    }

    #[test]
    fn 完整循环_验证通过() {
        let mut mode = GoalMode::new("完整测试".to_string());
        mode.activate();

        // Planning → Executing
        assert!(mode.advance());
        assert_eq!(mode.current_phase(), GoalPhase::Executing);

        // Executing → Verifying
        assert!(mode.advance());
        assert_eq!(mode.current_phase(), GoalPhase::Verifying);

        // 设置验证通过
        mode.set_verification(VerificationReport {
            cycle: 0,
            command_results: vec![],
            criteria_results: vec![],
            passed: true,
            failure_summary: None,
            suggestions: vec![],
        });

        // Verifying → Done
        assert!(mode.advance());
        assert_eq!(mode.current_phase(), GoalPhase::Done);
        assert!(!mode.is_active());
        assert!(mode.is_complete());
    }

    #[test]
    fn 验证失败触发重计划() {
        let mut mode = GoalMode::new("失败测试".to_string());
        mode.activate();

        // Planning → Executing → Verifying
        mode.advance();
        mode.advance();
        assert_eq!(mode.current_phase(), GoalPhase::Verifying);

        // 设置验证失败
        mode.set_verification(VerificationReport {
            cycle: 0,
            command_results: vec![],
            criteria_results: vec![],
            passed: false,
            failure_summary: Some("测试失败".into()),
            suggestions: vec![],
        });

        // Verifying → Planning (replan)
        assert!(mode.advance());
        assert_eq!(mode.current_phase(), GoalPhase::Planning);
        assert_eq!(mode.current_cycle(), 1);
    }

    #[test]
    fn 超过初始循环预算会扩展并继续() {
        let config = GoalConfig {
            max_cycles: 2,
            ..GoalConfig::default()
        };
        let mut mode = GoalMode::with_config("预算测试".to_string(), config);
        mode.activate();

        let failed_report = VerificationReport {
            cycle: 0,
            command_results: vec![],
            criteria_results: vec![],
            passed: false,
            failure_summary: Some("失败".into()),
            suggestions: vec![],
        };

        // 第一次失败: cycle 0 → 1
        mode.advance(); // Planning → Executing
        mode.advance(); // Executing → Verifying
        mode.set_verification(failed_report.clone());
        mode.advance(); // Verifying → Planning
        assert_eq!(mode.current_cycle(), 1);

        // 第二次失败: cycle 1 → 2
        mode.advance(); // Planning → Executing
        mode.advance(); // Executing → Verifying
        mode.set_verification(failed_report.clone());
        mode.advance(); // Verifying → Planning, cycle=2 equals max_cycles
        assert_eq!(mode.current_cycle(), 2);

        // 第三次失败: cycle 2 → 3, should extend budget
        let old_max = mode.config().max_cycles;
        mode.advance(); // Planning → Executing
        mode.advance(); // Executing → Verifying
        mode.set_verification(failed_report);
        mode.advance(); // Verifying → Planning, cycle=3 > max_cycles=2 → extend
        assert_eq!(mode.current_cycle(), 3);
        assert!(mode.config().max_cycles > old_max);
    }

    #[test]
    fn drift_检测触发重计划() {
        let mut mode = GoalMode::new("drift 测试".to_string());
        mode.activate();
        mode.advance(); // Planning → Executing

        let result = mode.handle_drift(HarnessAction::Replan {
            reason: "偏离目标".into(),
        });
        assert!(result);
        assert_eq!(mode.current_phase(), GoalPhase::Planning);
        assert_eq!(mode.current_cycle(), 1);
    }

    #[test]
    fn drift_升级停止执行() {
        let mut mode = GoalMode::new("escalate 测试".to_string());
        mode.activate();

        let result = mode.handle_drift(HarnessAction::Escalate {
            reason: "无法恢复".into(),
        });
        assert!(result);
        assert!(!mode.is_active());
    }

    #[test]
    fn 禁用_drift_检测时不触发() {
        let config = GoalConfig {
            enable_drift_detection: false,
            ..GoalConfig::default()
        };
        let mut mode = GoalMode::with_config("禁用 drift".to_string(), config);
        mode.activate();

        let result = mode.handle_drift(HarnessAction::Replan {
            reason: "不应触发".into(),
        });
        assert!(!result);
    }

    #[test]
    fn 状态摘要包含关键信息() {
        let mut mode = GoalMode::new("摘要测试".to_string());
        mode.activate();

        let summary = mode.status_summary();
        assert!(summary.contains("制定计划"));
        assert!(summary.contains("0/20"));
        assert!(summary.contains("No plan yet"));

        // 设置一个计划
        mode.set_plan(GoalPlan {
            requirement: "摘要测试".to_string(),
            tasks: vec![
                GoalTask {
                    id: "t1".into(),
                    title: "任务1".into(),
                    description: "描述".into(),
                    target_files: vec![],
                    acceptance_criteria: vec![],
                    depends_on: vec![],
                    status: GoalTaskStatus::Completed,
                },
                GoalTask {
                    id: "t2".into(),
                    title: "任务2".into(),
                    description: "描述".into(),
                    target_files: vec![],
                    acceptance_criteria: vec![],
                    depends_on: vec![],
                    status: GoalTaskStatus::Pending,
                },
            ],
            acceptance_criteria: vec![],
            cycle: 0,
            forbidden_detours: vec![],
            context: String::new(),
        });

        let summary = mode.status_summary();
        assert!(summary.contains("Tasks: 1/2"));
    }

    #[test]
    fn 停止后不再推进() {
        let mut mode = GoalMode::new("停止测试".to_string());
        mode.activate();
        mode.deactivate();
        assert!(!mode.is_active());
        assert!(!mode.advance());
    }

    #[test]
    fn 完成后不再推进() {
        let mut mode = GoalMode::new("完成测试".to_string());
        mode.activate();

        // 推进到 Done
        mode.advance(); // Planning → Executing
        mode.advance(); // Executing → Verifying
        mode.set_verification(VerificationReport {
            cycle: 0,
            command_results: vec![],
            criteria_results: vec![],
            passed: true,
            failure_summary: None,
            suggestions: vec![],
        });
        mode.advance(); // Verifying → Done
        assert_eq!(mode.current_phase(), GoalPhase::Done);

        // Done 之后无法推进
        assert!(!mode.advance());
    }

    // ===========================================================
    // GoalState 持久化测试
    // ===========================================================

    #[test]
    fn goal_state_序列化往返() {
        let config = GoalConfig::default();
        let plan = GoalPlan {
            requirement: "实现认证".into(),
            tasks: vec![GoalTask {
                id: "T1".into(),
                title: "创建模型".into(),
                description: "定义 User".into(),
                target_files: vec!["src/user.rs".into()],
                acceptance_criteria: vec!["编译通过".into()],
                depends_on: vec![],
                status: GoalTaskStatus::Pending,
            }],
            acceptance_criteria: vec!["测试通过".into()],
            cycle: 1,
            forbidden_detours: vec![],
            context: "初始".into(),
        };
        let state = GoalState::new("实现认证".into(), config, plan);

        let json = state.to_json().unwrap();
        let roundtrip = GoalState::from_json(&json).unwrap();

        assert_eq!(roundtrip.requirement, "实现认证");
        assert_eq!(roundtrip.plan.tasks.len(), 1);
        assert_eq!(roundtrip.current_phase, GoalPhase::Planning);
    }

    #[test]
    fn goal_state_恢复提示包含关键信息() {
        let config = GoalConfig::default();
        let plan = GoalPlan {
            requirement: "重构缓存层".into(),
            tasks: vec![GoalTask {
                id: "T1".into(),
                title: "完成".into(),
                description: "已完成".into(),
                target_files: vec![],
                acceptance_criteria: vec![],
                depends_on: vec![],
                status: GoalTaskStatus::Completed,
            }],
            acceptance_criteria: vec![],
            cycle: 2,
            forbidden_detours: vec![],
            context: "replan".into(),
        };
        let mut state = GoalState::new("重构缓存层".into(), config, plan);
        state.current_cycle = 2;
        state.current_phase = GoalPhase::Executing;
        state.last_checkpoint = Some("完成了缓存接口定义".into());

        let prompt = state.resume_prompt();
        assert!(prompt.contains("重构缓存层"));
        assert!(prompt.contains("1/1"));
        assert!(prompt.contains("执行任务"));
        assert!(prompt.contains("完成了缓存接口定义"));
    }

    #[test]
    fn goal_state_恢复提示包含验证失败信息() {
        let config = GoalConfig::default();
        let plan = GoalPlan {
            requirement: "测试".into(),
            tasks: vec![],
            acceptance_criteria: vec![],
            cycle: 1,
            forbidden_detours: vec![],
            context: String::new(),
        };
        let mut state = GoalState::new("测试".into(), config, plan);
        state.last_verification = Some(VerificationReport {
            cycle: 1,
            command_results: vec![],
            criteria_results: vec![],
            passed: false,
            failure_summary: Some("2 tests failed".into()),
            suggestions: vec![],
        });

        let prompt = state.resume_prompt();
        assert!(prompt.contains("上次验证失败"));
        assert!(prompt.contains("2 tests failed"));
    }
}
