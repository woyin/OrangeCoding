# Goal Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `/goal` autonomous iterative loop that replaces Autopilot, with mixed planning/execution mode, drift detection, and checkpoint persistence.

**Architecture:** New `GoalMode` workflow in `workflows/goal.rs` as a state machine (Planning → Executing → Verifying → Done), with `GoalState` persistence to `.sisyphus/goal.json`, integration with `HarnessSupervisor` for drift detection, and triple completion guarantee (auto-verify + promise tag + iteration cap).

**Tech Stack:** Rust, serde (Serialize/Deserialize), chrono, tokio, clap

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/orangecoding-agent/src/workflows/goal.rs` | Create | GoalMode state machine, GoalPlan, GoalTask, GoalConfig, GoalPhase, VerificationReport types |
| `crates/orangecoding-agent/src/workflows/mod.rs` | Modify | Add `pub mod goal;` |
| `crates/orangecoding-agent/src/execution_prompt.rs` | Modify | Replace `Autopilot` variant with `Goal`, add GOAL_PROMPT |
| `crates/orangecoding-cli/src/slash.rs` | Modify | Replace autopilot builtins with goal builtins |
| `crates/orangecoding-cli/src/slash_builtins.rs` | Modify | Replace autopilot handlers with goal handlers, update help text |
| `crates/orangecoding-cli/src/commands/launch.rs` | Modify | Replace `--autopilot` args with `--goal` args, update mode mapping |
| `crates/orangecoding-tui/src/app.rs` | Modify | Replace `InteractionMode::Autopilot` with `Goal` |
| `crates/orangecoding-agent/src/workflows/autopilot.rs` | Delete | Removed entirely |

---

### Task 1: Create goal.rs — Types (GoalPhase, GoalConfig, GoalTask, GoalPlan)

**Files:**
- Create: `crates/orangecoding-agent/src/workflows/goal.rs`
- Modify: `crates/orangecoding-agent/src/workflows/mod.rs:8` (replace `mod autopilot` with `mod goal`)

- [ ] **Step 1: Create goal.rs with core types and failing tests**

```rust
//! # Goal 自主迭代循环
//!
//! 混合模式：先规划，再自引用迭代执行，规划可动态调整。
//!
//! ## 触发方式
//!
//! - CLI: `orangecoding --goal "需求描述"`
//! - 斜杠命令: `/goal 需求描述`
//!
//! ## 循环流程
//!
//! ```text
//! Planning → Executing → Verifying → (Replan? or Done)
//!                 ▲                         |
//!                 |     drift/verify fail   |
//!                 └─────────────────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::harness::{HarnessAction, MissionContract};

// ============================================================
// Goal 阶段
// ============================================================

/// Goal 执行阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GoalPhase {
    /// 生成/重新生成执行计划
    Planning,
    /// 按计划自引用迭代执行任务
    Executing,
    /// 验证执行结果（测试运行 + AI 评估）
    Verifying,
    /// 全部完成
    Done,
}

impl GoalPhase {
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
    /// 初始循环软预算（默认 20，耗尽后自动扩展）
    pub max_cycles: u32,
    /// 每个任务内部的 AI 初始迭代软预算
    pub task_max_iterations: u32,
    /// 自定义验证命令
    pub verify_commands: Vec<String>,
    /// 每轮完成后是否自动 git commit
    pub auto_commit_per_cycle: bool,
    /// 完成信号短语
    pub completion_promise: String,
    /// 是否启用 drift 检测
    pub enable_drift_detection: bool,
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
// 测试：类型与默认值
// ============================================================

#[cfg(test)]
mod type_tests {
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
            reason: "编译错误".into(),
        };
        let json = serde_json::to_string(&status).unwrap();
        let roundtrip: GoalTaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, status);
    }

    #[test]
    fn 计划可序列化() {
        let plan = GoalPlan {
            requirement: "实现用户认证".into(),
            tasks: vec![GoalTask {
                id: "T1".into(),
                title: "创建 User 模型".into(),
                description: "定义 User struct".into(),
                target_files: vec!["src/models/user.rs".into()],
                acceptance_criteria: vec!["编译通过".into()],
                depends_on: vec![],
                status: GoalTaskStatus::Pending,
            }],
            acceptance_criteria: vec!["所有测试通过".into()],
            cycle: 1,
            forbidden_detours: vec!["src/config.rs".into()],
            context: "初始规划".into(),
        };

        let json = serde_json::to_string(&plan).unwrap();
        let roundtrip: GoalPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.requirement, "实现用户认证");
        assert_eq!(roundtrip.tasks.len(), 1);
        assert_eq!(roundtrip.forbidden_detours.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --package orangecoding-agent workflows::goal::type_tests`
Expected: 4 tests PASS

- [ ] **Step 3: Update workflows/mod.rs**

Replace the autopilot module declaration with goal:

```rust
//! # 编排工作流模块
//!
//! 实现多Agent协作的工作流引擎，包含规划、执行、会话连续性等核心流程。

/// Atlas 执行编排
pub mod atlas;
/// Goal 自主迭代循环（Planning → Executing → Verifying → Replan/Done）
pub mod goal;
/// Boulder 会话连续性系统
pub mod boulder;
/// Prometheus 规划工作流
pub mod prometheus;
/// UltraWork 全自动模式
pub mod ultrawork;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --package orangecoding-agent`
Expected: BUILD SUCCEEDS

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-agent/src/workflows/goal.rs crates/orangecoding-agent/src/workflows/mod.rs
git commit -m "feat(workflow): add Goal types — GoalPhase, GoalConfig, GoalTask, GoalPlan"
```

---

### Task 2: Add verification types to goal.rs

**Files:**
- Modify: `crates/orangecoding-agent/src/workflows/goal.rs` (append after `GoalPlan`)

- [ ] **Step 1: Add CommandResult, CriteriaResult, VerificationReport structs and tests**

Append these types after the `GoalPlan` struct definition (before the `#[cfg(test)]` block):

```rust
// ============================================================
// 验证报告
// ============================================================

/// 单条验证命令执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub passed: bool,
}

/// 单条验收标准评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriteriaResult {
    pub criterion: String,
    pub satisfied: bool,
    pub evidence: String,
}

/// 验证报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub cycle: u32,
    pub command_results: Vec<CommandResult>,
    pub criteria_results: Vec<CriteriaResult>,
    pub passed: bool,
    pub failure_summary: Option<String>,
    pub suggestions: Vec<String>,
}
```

Add these tests inside the `mod type_tests` block:

```rust
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
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package orangecoding-agent workflows::goal::type_tests`
Expected: 6 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/orangecoding-agent/src/workflows/goal.rs
git commit -m "feat(workflow): add VerificationReport types to goal module"
```

---

### Task 3: Implement GoalMode state machine

**Files:**
- Modify: `crates/orangecoding-agent/src/workflows/goal.rs` (append after verification types)

- [ ] **Step 1: Add GoalMode state machine and tests**

Append the GoalMode struct and implementation after the `VerificationReport` struct (before the `#[cfg(test)]` block):

```rust
// ============================================================
// Goal 状态机
// ============================================================

/// Goal 自主迭代循环控制器
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

    /// 激活 Goal 模式
    pub fn activate(&mut self) {
        self.is_active = true;
        self.phase = GoalPhase::Planning;
        self.current_cycle = 0;
    }

    /// 停止 Goal
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// 推进到下一阶段，返回是否成功
    pub fn advance(&mut self) -> bool {
        if !self.is_active {
            return false;
        }

        let next = match self.phase {
            GoalPhase::Planning => Some(GoalPhase::Executing),
            GoalPhase::Executing => Some(GoalPhase::Verifying),
            GoalPhase::Verifying => {
                if let Some(ref report) = self.last_verification {
                    if report.passed {
                        Some(GoalPhase::Done)
                    } else {
                        if self.current_cycle >= self.config.max_cycles {
                            self.extend_cycle_budget();
                        }
                        self.current_cycle += 1;
                        Some(GoalPhase::Planning)
                    }
                } else {
                    Some(GoalPhase::Done)
                }
            }
            GoalPhase::Done => None,
        };

        match next {
            Some(phase) => {
                self.phase = phase;
                if phase == GoalPhase::Done {
                    self.is_active = false;
                }
                true
            }
            None => false,
        }
    }

    /// 处理 drift 检测结果，返回是否应 replan
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

    fn extend_cycle_budget(&mut self) {
        let extension = (self.config.max_cycles.saturating_add(1) / 2).max(1);
        self.config.max_cycles = self.config.max_cycles.saturating_add(extension);
    }

    // -- Getters & Setters --

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

    pub fn set_plan(&mut self, plan: GoalPlan) {
        self.plan = Some(plan);
    }

    pub fn last_verification(&self) -> Option<&VerificationReport> {
        self.last_verification.as_ref()
    }

    pub fn set_verification(&mut self, report: VerificationReport) {
        self.last_verification = Some(report);
    }

    pub fn mission_contract(&self) -> Option<&MissionContract> {
        self.mission_contract.as_ref()
    }

    pub fn set_mission_contract(&mut self, contract: MissionContract) {
        self.mission_contract = Some(contract);
    }

    /// 生成状态摘要
    pub fn status_summary(&self) -> String {
        format!(
            "Goal [{}] Cycle {}/{} — {}",
            self.phase.display_name(),
            self.current_cycle,
            self.config.max_cycles,
            if let Some(ref plan) = self.plan {
                let done = plan
                    .tasks
                    .iter()
                    .filter(|t| matches!(t.status, GoalTaskStatus::Completed))
                    .count();
                let total = plan.tasks.len();
                format!("Tasks: {done}/{total}")
            } else {
                "No plan yet".to_string()
            }
        )
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.phase, GoalPhase::Done)
    }
}
```

Replace the entire `#[cfg(test)]` block with comprehensive tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::{HarnessConfig, ReviewGatePolicy};

    // ---- 类型测试 ----

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

    // ---- 状态机测试 ----

    #[test]
    fn 新建模式默认未激活() {
        let mode = GoalMode::new("test".into());
        assert!(!mode.is_active());
        assert_eq!(mode.current_phase(), GoalPhase::Planning);
        assert_eq!(mode.current_cycle(), 0);
    }

    #[test]
    fn 激活后可推进() {
        let mut mode = GoalMode::new("test".into());
        mode.activate();
        assert!(mode.is_active());
        assert!(mode.advance()); // Planning → Executing
        assert_eq!(mode.current_phase(), GoalPhase::Executing);
    }

    #[test]
    fn 未激活时无法推进() {
        let mut mode = GoalMode::new("test".into());
        assert!(!mode.advance());
    }

    #[test]
    fn 完整循环_验证通过() {
        let mut mode = GoalMode::new("test".into());
        mode.activate();

        mode.advance(); // → Executing
        mode.advance(); // → Verifying

        mode.set_verification(VerificationReport {
            cycle: 1,
            command_results: vec![],
            criteria_results: vec![],
            passed: true,
            failure_summary: None,
            suggestions: vec![],
        });

        mode.advance(); // → Done
        assert!(mode.is_complete());
        assert!(!mode.is_active());
    }

    #[test]
    fn 验证失败触发重计划() {
        let mut mode = GoalMode::new("test".into());
        mode.activate();

        mode.advance(); // → Executing
        mode.advance(); // → Verifying

        mode.set_verification(VerificationReport {
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
        });

        mode.advance(); // → Planning (replan)
        assert_eq!(mode.current_phase(), GoalPhase::Planning);
        assert_eq!(mode.current_cycle(), 1);
        assert!(mode.is_active());
    }

    #[test]
    fn 超过初始循环预算会扩展并继续() {
        let mut config = GoalConfig::default();
        config.max_cycles = 2;
        let mut mode = GoalMode::with_config("test".into(), config);
        mode.activate();

        // Cycle 1
        mode.advance(); // → Executing
        mode.advance(); // → Verifying
        mode.set_verification(VerificationReport {
            cycle: 1,
            command_results: vec![],
            criteria_results: vec![],
            passed: false,
            failure_summary: Some("fail".into()),
            suggestions: vec![],
        });
        mode.advance(); // → Planning (replan, cycle=1)

        // Cycle 2
        mode.advance(); // → Executing
        mode.advance(); // → Verifying
        mode.set_verification(VerificationReport {
            cycle: 2,
            command_results: vec![],
            criteria_results: vec![],
            passed: false,
            failure_summary: Some("fail again".into()),
            suggestions: vec![],
        });
        mode.advance(); // → Planning (replan, cycle=2, extends budget)

        assert!(mode.is_active());
        assert!(mode.config().max_cycles > 2);
    }

    #[test]
    fn drift_检测触发重计划() {
        let mut mode = GoalMode::new("test".into());
        mode.activate();
        mode.advance(); // → Executing

        let replanned = mode.handle_drift(HarnessAction::Replan {
            reason: "偏离目标".into(),
        });
        assert!(replanned);
        assert_eq!(mode.current_phase(), GoalPhase::Planning);
    }

    #[test]
    fn drift_升级停止执行() {
        let mut mode = GoalMode::new("test".into());
        mode.activate();

        let escalated = mode.handle_drift(HarnessAction::Escalate {
            reason: "安全边界".into(),
        });
        assert!(escalated);
        assert!(!mode.is_active());
    }

    #[test]
    fn 禁用_drift_检测时不触发() {
        let mut config = GoalConfig::default();
        config.enable_drift_detection = false;
        let mut mode = GoalMode::with_config("test".into(), config);
        mode.activate();

        let result = mode.handle_drift(HarnessAction::Replan {
            reason: "偏离".into(),
        });
        assert!(!result);
    }

    #[test]
    fn 状态摘要包含关键信息() {
        let mut mode = GoalMode::new("Build auth system".into());
        mode.activate();

        let summary = mode.status_summary();
        assert!(summary.contains("制定计划"));
        assert!(summary.contains("0/20"));
    }

    #[test]
    fn 停止后不再推进() {
        let mut mode = GoalMode::new("test".into());
        mode.activate();
        mode.deactivate();
        assert!(!mode.advance());
    }

    #[test]
    fn 完成后不再推进() {
        let mut mode = GoalMode::new("test".into());
        mode.activate();
        mode.advance(); // → Executing
        mode.advance(); // → Verifying
        mode.set_verification(VerificationReport {
            cycle: 1,
            command_results: vec![],
            criteria_results: vec![],
            passed: true,
            failure_summary: None,
            suggestions: vec![],
        });
        mode.advance(); // → Done
        assert!(!mode.advance());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package orangecoding-agent workflows::goal::tests`
Expected: 13 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/orangecoding-agent/src/workflows/goal.rs
git commit -m "feat(workflow): implement GoalMode state machine with drift detection"
```

---

### Task 4: Add GoalState persistence

**Files:**
- Modify: `crates/orangecoding-agent/src/workflows/goal.rs` (append GoalState after GoalMode)

- [ ] **Step 1: Add GoalState struct, file operations, and tests**

Append after `GoalMode` impl block (before the `#[cfg(test)]` block):

```rust
// ============================================================
// Goal 持久化状态
// ============================================================

/// Goal 状态文件路径
pub const GOAL_FILE_PATH: &str = ".sisyphus/goal.json";

/// Goal 持久化状态（序列化到 goal.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalState {
    /// Goal 唯一标识
    pub id: String,
    /// 用户需求描述
    pub requirement: String,
    /// 当前计划
    pub plan: GoalPlan,
    /// 运行配置
    pub config: GoalConfig,
    /// 任务契约（drift 检测）
    pub mission_contract: Option<MissionContract>,
    /// 当前阶段
    pub current_phase: GoalPhase,
    /// 当前任务索引
    pub current_task_index: usize,
    /// 当前循环次数
    pub current_cycle: u32,
    /// 关联的会话 ID 列表
    pub session_ids: Vec<String>,
    /// 开始时间
    pub started_at: String,
    /// 上次完成的任务摘要
    pub last_checkpoint: Option<String>,
    /// 上次验证报告
    pub last_verification: Option<VerificationReport>,
}

impl GoalState {
    /// 创建新的 Goal 状态
    pub fn new(requirement: String, config: GoalConfig, plan: GoalPlan) -> Self {
        let now = chrono::Utc::now();
        let timestamp_hex = format!("{:012x}", now.timestamp());
        let random_hex = format!("{:08x}", rand::random::<u32>());
        Self {
            id: format!("goal-{timestamp_hex}{random_hex}"),
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

    /// 序列化为 JSON
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化 goal 状态失败: {e}"))
    }

    /// 从 JSON 反序列化
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("解析 goal.json 失败: {e}"))
    }

    /// 生成恢复提示
    pub fn resume_prompt(&self) -> String {
        let done = self
            .plan
            .tasks
            .iter()
            .filter(|t| matches!(t.status, GoalTaskStatus::Completed))
            .count();
        let total = self.plan.tasks.len();

        let checkpoint_note = self
            .last_checkpoint
            .as_ref()
            .map(|c| format!("\n上次检查点: {c}"))
            .unwrap_or_default();

        let verification_note = self
            .last_verification
            .as_ref()
            .filter(|v| !v.passed)
            .map(|v| {
                format!(
                    "\n上次验证失败: {}",
                    v.failure_summary.as_deref().unwrap_or("未知原因")
                )
            })
            .unwrap_or_default();

        format!(
            "恢复执行目标「{}」。\n\
             当前进度: {done}/{total}（第 {} 轮）\n\
             阶段: {}{}{}",
            self.requirement,
            self.current_cycle + 1,
            self.current_phase.display_name(),
            checkpoint_note,
            verification_note,
        )
    }
}
```

Note: Since `rand` may not be in dependencies, use a simpler ID generation. If `rand` is not available, replace with:

```rust
        let random_hex = format!("{:08x}", now.timestamp_subsec_nanos());
```

Add these tests inside the `mod tests` block:

```rust
    // ---- 持久化测试 ----

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
```

- [ ] **Step 2: Check if `rand` crate is available; if not, use nano-based ID**

Run: `grep -q 'rand' crates/orangecoding-agent/Cargo.toml && echo "rand found" || echo "rand not found"`

If "rand not found", replace the `random_hex` line in `GoalState::new()` with:
```rust
        let random_hex = format!("{:08x}", now.timestamp_subsec_nanos());
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package orangecoding-agent workflows::goal::tests`
Expected: 16 tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/orangecoding-agent/src/workflows/goal.rs
git commit -m "feat(workflow): add GoalState persistence with resume_prompt"
```

---

### Task 5: Update ExecutionMode — replace Autopilot with Goal

**Files:**
- Modify: `crates/orangecoding-agent/src/execution_prompt.rs`

- [ ] **Step 1: Replace Autopilot variant and prompt with Goal**

Replace the entire content of `execution_prompt.rs`:

```rust
/// 代理执行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 直接执行模式。
    Exec,
    /// 计划确认模式。
    Plan,
    /// Goal 自主迭代循环模式。
    Goal,
    /// UltraWork 模式。
    UltraWork,
}

const SHARED_PROMPT: &str = r#"[MISSION LOCK]
保持对用户原始任务的锁定，所有输出与行动都必须服务于当前任务。

TASK DIFFICULTY SIGNAL: 用户可以显式指定任务难度 easy/medium/hard/epic；如果用户没有指定，则根据任务范围、风险、依赖和验证成本自行推断。"#;

const EXEC_PROMPT: &str = r#"[EXEC MODE - 严格执行]
立即执行用户请求，不要把执行请求改写成计划讨论。
严格按用户字面指令执行；不要擅自扩展范围、替换目标或改变验收标准。
只有遇到真实且不可逆的决策分叉时才询问用户；其他情况根据上下文作出合理选择并继续推进。
持续验证结果，直到请求完成。"#;

const PLAN_PROMPT: &str = r#"[PLAN MODE - 结构化计划]
使用 Plan mode / 结构化计划语言输出方案。
计划必须包含：
- Goal / 目标
- Phases / 阶段
- Steps / 步骤
- Acceptance / 验收标准
- Estimated difficulty / 预估难度

计划输出后必须等待用户确认计划；在计划确认前不要修改代码、不要执行会改变仓库状态的操作。
计划确认后询问执行策略："一步到位" -> Goal，"Exec 模式" -> Exec。"#;

const GOAL_PROMPT: &str = r#"[GOAL MODE - 自主迭代循环]
永远不要在任务中途为了用户确认而停止；除非遇到安全边界或真实阻塞，否则持续推进。
每 5 步静默执行一次指令回锚，确认当前行动仍然服务于用户原始目标。
遇到障碍时，停止前先尝试 3 种替代方案。
步数不是硬限制；目标完成才是停止条件。

每步决策时回顾 MissionContract：
- 目标是否仍然一致？
- 是否触碰了 forbidden detours？
- 如有偏离，立即纠正。

完成时输出 <promise>GOAL_COMPLETE</promise> 标签。

自检模板：
- original instruction / 原始指令：当前目标要求是什么？
- current action / 当前动作：我正在做的动作如何推进目标？
- drift correction / 偏移纠正：如果偏离，立即回到原始目标。"#;

const ULTRAWORK_PROMPT: &str = r#"[ULTRAWORK MODE]
保持当前 UltraWork 行为不变。"#;

/// 根据执行模式构建系统提示词。
pub fn build_system_prompt(mode: ExecutionMode) -> String {
    let mode_prompt = match mode {
        ExecutionMode::Exec => EXEC_PROMPT,
        ExecutionMode::Plan => PLAN_PROMPT,
        ExecutionMode::Goal => GOAL_PROMPT,
        ExecutionMode::UltraWork => ULTRAWORK_PROMPT,
    };

    format!("{SHARED_PROMPT}\n\n{mode_prompt}")
}

#[cfg(test)]
mod tests {
    use super::{build_system_prompt, ExecutionMode};

    #[test]
    fn 测试_exec_prompt_包含严格执行规则() {
        let prompt = build_system_prompt(ExecutionMode::Exec);

        assert!(prompt.contains("[EXEC MODE - 严格执行]"));
        assert!(prompt.contains("决策分叉"));
    }

    #[test]
    fn 测试_plan_prompt_包含结构化计划与确认要求() {
        let prompt = build_system_prompt(ExecutionMode::Plan);

        assert!(prompt.contains("结构化计划"));
        assert!(prompt.contains("一步到位"));
        assert!(prompt.contains("Exec 模式"));
        assert!(prompt.contains("不要修改代码"));
    }

    #[test]
    fn 测试_goal_prompt_包含自主执行规则() {
        let prompt = build_system_prompt(ExecutionMode::Goal);

        assert!(prompt.contains("[MISSION LOCK]"));
        assert!(prompt.contains("指令回锚"));
        assert!(prompt.contains("步数不是硬限制"));
        assert!(prompt.contains("MissionContract"));
        assert!(prompt.contains("<promise>GOAL_COMPLETE</promise>"));
    }

    #[test]
    fn 测试_shared_prompt_包含任务难度信号() {
        let prompt = build_system_prompt(ExecutionMode::Exec);

        assert!(prompt.contains("TASK DIFFICULTY SIGNAL"));
        assert!(prompt.contains("easy/medium/hard/epic"));
    }

    #[test]
    fn 测试_ultrawork_prompt_保持当前行为() {
        let prompt = build_system_prompt(ExecutionMode::UltraWork);

        assert!(prompt.contains("UltraWork"));
        assert!(prompt.contains("保持当前 UltraWork 行为不变"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package orangecoding-agent execution_prompt::tests`
Expected: 5 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/orangecoding-agent/src/execution_prompt.rs
git commit -m "feat(agent): replace ExecutionMode::Autopilot with ExecutionMode::Goal"
```

---

### Task 6: Update TUI InteractionMode — replace Autopilot with Goal

**Files:**
- Modify: `crates/orangecoding-tui/src/app.rs:60-131`

- [ ] **Step 1: Replace Autopilot variant with Goal**

Replace the `InteractionMode` enum and all its methods. Change every `Autopilot` reference to `Goal`:

In the enum (line 66):
```rust
    Autopilot,  →  Goal,
```

In `all()` (line 77):
```rust
            InteractionMode::Autopilot,  →  InteractionMode::Goal,
```

In `next()` (line 86-87):
```rust
            InteractionMode::Plan => InteractionMode::Autopilot,         →  InteractionMode::Plan => InteractionMode::Goal,
            InteractionMode::Autopilot => InteractionMode::UltraWork,    →  InteractionMode::Goal => InteractionMode::UltraWork,
```

In `label()` (line 97):
```rust
            InteractionMode::Autopilot => "Autopilot",  →  InteractionMode::Goal => "Goal",
```

In `description()` (line 107):
```rust
            InteractionMode::Autopilot => "长任务模式 - 全程自动执行并静默自纠",  →  InteractionMode::Goal => "Goal 模式 - 自主迭代循环，自动规划、执行、验证",
```

In `from_str_name()` (line 117):
```rust
            "autopilot" | "auto" => Some(InteractionMode::Autopilot),  →  "goal" | "auto" => Some(InteractionMode::Goal),
```

- [ ] **Step 2: Update launch.rs mode_to_execution_mode mapping**

In `crates/orangecoding-cli/src/commands/launch.rs`, line 399:
```rust
        orangecoding_tui::app::InteractionMode::Autopilot => ExecutionMode::Autopilot,  →  orangecoding_tui::app::InteractionMode::Goal => ExecutionMode::Goal,
```

In `is_mode_system_prompt` (line 422):
```rust
        ExecutionMode::Autopilot,  →  ExecutionMode::Goal,
```

- [ ] **Step 3: Run build to verify compilation**

Run: `cargo build`
Expected: BUILD SUCCEEDS (may have warnings about unused imports, but no errors)

- [ ] **Step 4: Commit**

```bash
git add crates/orangecoding-tui/src/app.rs crates/orangecoding-cli/src/commands/launch.rs
git commit -m "feat(tui): replace InteractionMode::Autopilot with Goal"
```

---

### Task 7: Update slash commands — replace autopilot with goal

**Files:**
- Modify: `crates/orangecoding-cli/src/slash.rs:86-98` (replace autopilot builtins)
- Modify: `crates/orangecoding-cli/src/slash_builtins.rs` (replace autopilot handlers)

- [ ] **Step 1: Update slash.rs register_builtins**

In the `builtins` array, replace the autopilot-related entries (around line 86-98):

Remove:
```rust
            (
                "ralph-loop",
                "Ralph 循环：持续改进循环（plan → implement → review → refine）",
            ),
```

Add/replace with:
```rust
            (
                "goal",
                "Goal 自主迭代循环：规划 → 执行 → 验证 → 动态调整",
            ),
            ("goal-stop", "停止当前 Goal 循环"),
```

Keep `ralph-loop` if desired, but add the new `goal` and `goal-stop` entries.

- [ ] **Step 2: Update slash_builtins.rs execute_builtin match arms**

Remove the autopilot match arms (lines 52-54):
```rust
        "autopilot" => SlashCommandResult::Prompt(format_autopilot(args)),
        "autopilot-stop" => SlashCommandResult::Executed,
        "autopilot-status" => SlashCommandResult::Prompt(format_autopilot_status()),
```

Replace with:
```rust
        "goal" => SlashCommandResult::Prompt(format_goal(args)),
        "goal-stop" => SlashCommandResult::Executed,
```

- [ ] **Step 3: Replace format_autopilot and format_autopilot_status functions**

Remove `format_autopilot()` and `format_autopilot_status()` functions. Add:

```rust
fn format_goal(args: &str) -> String {
    let requirement = if args.is_empty() {
        "未指定需求（将在首次交互中收集）".to_string()
    } else {
        args.to_string()
    };
    format!(
        "Goal 自主迭代循环已启动\n\
         需求: {}\n\
         循环流程: Planning → Executing → Verifying → Replan/Done\n\
         使用 /goal-stop 可随时停止",
        requirement
    )
}
```

- [ ] **Step 4: Update help text**

In `format_help()`, replace the "Autopilot:" section:

```rust
Autopilot:
  /autopilot [需求] 启动长任务全自动模式
  /autopilot-stop  停止 Autopilot 循环
  /autopilot-status 查看 Autopilot 状态
```

With:
```rust
Goal:
  /goal [需求]     启动自主迭代循环（规划→执行→验证）
  /goal-stop       停止 Goal 循环
```

- [ ] **Step 5: Update tests in slash_builtins.rs**

Remove `test_execute_autopilot`, `test_execute_autopilot_no_args`, `test_execute_autopilot_stop`, `test_execute_autopilot_status`, `test_help_contains_autopilot`.

Add:

```rust
    #[test]
    fn test_execute_goal() {
        let result = execute_builtin("goal", "实现用户认证系统");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("Goal"));
                assert!(text.contains("实现用户认证系统"));
                assert!(text.contains("Planning → Executing → Verifying"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_goal_no_args() {
        let result = execute_builtin("goal", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("Goal"));
                assert!(text.contains("未指定需求"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_goal_stop() {
        let result = execute_builtin("goal-stop", "");
        assert!(matches!(result, SlashCommandResult::Executed));
    }

    #[test]
    fn test_help_contains_goal() {
        let result = execute_builtin("help", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("/goal"));
                assert!(text.contains("/goal-stop"));
                assert!(text.contains("/doctor"));
                assert!(text.contains("/cost"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }
```

- [ ] **Step 6: Run tests**

Run: `cargo test --package orangecoding-cli slash`
Expected: All slash command tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/orangecoding-cli/src/slash.rs crates/orangecoding-cli/src/slash_builtins.rs
git commit -m "feat(cli): replace autopilot slash commands with goal commands"
```

---

### Task 8: Update CLI args — replace --autopilot with --goal

**Files:**
- Modify: `crates/orangecoding-cli/src/commands/launch.rs`

- [ ] **Step 1: Replace LaunchArgs autopilot fields with goal fields**

Replace fields from line 59-84:

```rust
    /// 启用 Goal 自主迭代循环模式
    ///
    /// 系统将自动执行 Planning → Executing → Verifying 循环，
    /// 直到所有任务完成或达到最大循环轮次。
    #[arg(long)]
    pub goal: bool,

    /// Goal 模式的需求文件路径
    #[arg(long)]
    pub goal_file: Option<String>,

    /// Goal 最大循环轮次（默认 20）
    #[arg(long)]
    pub max_cycles: Option<u32>,

    /// Goal 完成信号短语（默认 GOAL_COMPLETE）
    #[arg(long)]
    pub promise: Option<String>,

    /// Goal 验证命令（可多次使用，默认 cargo test）
    #[arg(long)]
    pub verify: Option<String>,

    /// 禁用 drift 检测
    #[arg(long, default_value_t = false)]
    pub no_drift_detect: bool,

    /// 禁用每轮自动 git commit
    #[arg(long, default_value_t = false)]
    pub no_auto_commit: bool,
```

- [ ] **Step 2: Update execute() function**

In the `execute()` function (around line 121-128), replace:

```rust
            args.autopilot || args.autopilot_file.is_some(),
```

With:
```rust
            args.goal || args.goal_file.is_some(),
```

- [ ] **Step 3: Update run_single_shot signature and body**

Change the `autopilot: bool` parameter to `goal: bool`:

```rust
async fn run_single_shot(
    provider: &dyn AiProvider,
    registry: &ToolRegistry,
    prompt: &str,
    model: &str,
    explicit_model: bool,
    goal: bool,
    config: &OrangeConfig,
) -> Result<()> {
```

Update the execution_mode selection:

```rust
    let execution_mode = if goal {
        ExecutionMode::Goal
    } else {
        ExecutionMode::Exec
    };
```

- [ ] **Step 4: Run build**

Run: `cargo build`
Expected: BUILD SUCCEEDS

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-cli/src/commands/launch.rs
git commit -m "feat(cli): replace --autopilot CLI args with --goal args"
```

---

### Task 9: Delete Autopilot

**Files:**
- Delete: `crates/orangecoding-agent/src/workflows/autopilot.rs`

- [ ] **Step 1: Delete the autopilot file**

Run: `rm crates/orangecoding-agent/src/workflows/autopilot.rs`

- [ ] **Step 2: Search for remaining autopilot references**

Run: `grep -rn "autopilot\|Autopilot\|AUTOPILOT" crates/ --include="*.rs" | grep -v "test"` 

Expected: Zero hits. If any remain, update them to use Goal equivalents.

- [ ] **Step 3: Run full build and test suite**

Run: `cargo build && cargo test`
Expected: BUILD SUCCEEDS, all tests PASS

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: remove Autopilot workflow — replaced by Goal"
```

---

### Task 10: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 2: Verify no Autopilot references remain**

Run: `grep -rn "autopilot\|Autopilot" crates/ docs/ --include="*.rs" --include="*.md" | grep -v "target/" | grep -v ".sisyphus/"`
Expected: Zero hits (or only historical references in design docs)

- [ ] **Step 3: Verify Goal module compiles and tests pass**

Run: `cargo test --package orangecoding-agent workflows::goal`
Expected: All 16 goal tests PASS

- [ ] **Step 4: Final commit if any cleanup needed**

```bash
git add -A
git commit -m "chore: final cleanup after Goal/Autopilot migration"
```
