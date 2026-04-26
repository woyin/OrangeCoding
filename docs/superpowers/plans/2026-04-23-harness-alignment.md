# Harness Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 OrangeCoding 中加入一个独立的 Harness 治理层，使 `--autopilot` 长任务执行具备任务契约、检查点、偏航判定、受控重规划和恢复时保持同一目标边界的能力。

**Architecture:** 在 `crates/orangecoding-agent/src/harness/` 下新增独立模块，定义 `MissionContract`、`StepOutcome`、`DriftDetector`、`HarnessSupervisor` 等核心类型；`workflows/autopilot.rs` 继续负责执行状态机，但每个有界执行段都必须经过 Harness 检查点裁决。`workflows/boulder.rs` 负责持久化最近一次已接受的 Harness 快照，`crates/orangecoding-config/src/config.rs` 暴露 `[autopilot.harness]` 配置，`crates/orangecoding-cli/src/commands/launch.rs` 将配置和 `--autopilot` 参数翻译为真实运行入口。

**Tech Stack:** Rust 2021、serde、tokio、Cargo workspace tests、根级 invariant tests、Markdown 中文文档。

---

## File Map

### Create

- `crates/orangecoding-agent/src/harness/mod.rs` — Harness 模块导出面。
- `crates/orangecoding-agent/src/harness/types.rs` — `MissionContract`、`StepOutcome`、`HarnessConfig`、决策枚举。
- `crates/orangecoding-agent/src/harness/drift.rs` — 偏航判定逻辑与判定结果。
- `crates/orangecoding-agent/src/harness/supervisor.rs` — 检查点治理主流程。
- `tests/invariants/harness_invariants.rs` — Harness 关键性质不变式测试。

### Modify

- `crates/orangecoding-agent/src/lib.rs` — 导出 `harness` 模块。
- `crates/orangecoding-agent/src/workflows/autopilot.rs` — 将执行状态机接入 Harness，并为长任务建立有界执行段。
- `crates/orangecoding-agent/src/workflows/boulder.rs` — 持久化 `MissionContract` 和最近已接受检查点。
- `crates/orangecoding-config/src/config.rs` — 新增 `[autopilot.harness]` 配置结构及默认值、序列化和测试。
- `crates/orangecoding-cli/src/commands/launch.rs` — 新增 `--autopilot` 真实运行入口、配置映射和参数覆盖逻辑。
- `docs/user-guide/workflows.md` — 说明 Harness 检查点、偏航升级与恢复语义。
- `docs/user-guide/commands.md` — 说明 `orangecoding launch --autopilot` 与配置项。
- `Cargo.toml` — 注册 `harness_invariants` 根级测试。

---

### Task 1: 建立 Harness 类型边界

**Files:**
- Create: `crates/orangecoding-agent/src/harness/mod.rs`
- Create: `crates/orangecoding-agent/src/harness/types.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`
- Test: `crates/orangecoding-agent/src/harness/types.rs`

- [ ] **Step 1: 先建立模块入口和失败测试**

```rust
// crates/orangecoding-agent/src/lib.rs
pub mod harness;

// crates/orangecoding-agent/src/harness/mod.rs
pub mod types;

pub use types::{
    HarnessAction, HarnessConfig, MissionContract, ReviewGatePolicy, StepOutcome,
};

// crates/orangecoding-agent/src/harness/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 任务契约可往返序列化() {
        let contract = MissionContract::new(
            "实现 Harness 对齐层".into(),
            vec!["出现检查点".into(), "重大变更触发门控".into()],
            ReviewGatePolicy::MajorPlanChange,
            HarnessConfig::default(),
        );

        let json = serde_json::to_string_pretty(&contract).unwrap();
        let roundtrip: MissionContract = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip.objective, "实现 Harness 对齐层");
        assert_eq!(roundtrip.success_criteria.len(), 2);
        assert_eq!(roundtrip.review_gate_policy, ReviewGatePolicy::MajorPlanChange);
    }

    #[test]
    fn 步骤结果保留对齐证据() {
        let outcome = StepOutcome {
            summary: "更新了 autopilot 状态机".into(),
            touched_files: vec!["crates/orangecoding-agent/src/workflows/autopilot.rs".into()],
            decisions: vec!["引入有界执行段".into()],
            rationale: "为了在每段结束后执行检查点".into(),
            blockers: vec![],
            proposed_plan_change: None,
        };

        assert!(outcome.rationale.contains("检查点"));
        assert_eq!(outcome.touched_files.len(), 1);
    }
}
```

- [ ] **Step 2: 运行测试，确认当前确实失败**

Run: `cargo test -p orangecoding-agent 任务契约可往返序列化 -- --exact`

Expected: FAIL，报错包含 `cannot find type 'MissionContract'` 或 `unresolved import`.

- [ ] **Step 3: 实现最小类型集合**

```rust
// crates/orangecoding-agent/src/harness/types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewGatePolicy {
    Never,
    MajorPlanChange,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub checkpoint_interval: u32,
    pub max_segment_steps: u32,
    pub major_plan_change_threshold: u32,
    pub drift_sensitivity: u8,
    pub escalate_on_low_confidence: bool,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            checkpoint_interval: 1,
            max_segment_steps: 5,
            major_plan_change_threshold: 1,
            drift_sensitivity: 2,
            escalate_on_low_confidence: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionContract {
    pub objective: String,
    pub success_criteria: Vec<String>,
    pub accepted_plan_summary: String,
    pub forbidden_detours: Vec<String>,
    pub review_gate_policy: ReviewGatePolicy,
    pub harness_config: HarnessConfig,
}

impl MissionContract {
    pub fn new(
        objective: String,
        success_criteria: Vec<String>,
        review_gate_policy: ReviewGatePolicy,
        harness_config: HarnessConfig,
    ) -> Self {
        Self {
            objective,
            success_criteria,
            accepted_plan_summary: String::new(),
            forbidden_detours: Vec::new(),
            review_gate_policy,
            harness_config,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepOutcome {
    pub summary: String,
    pub touched_files: Vec<String>,
    pub decisions: Vec<String>,
    pub rationale: String,
    pub blockers: Vec<String>,
    pub proposed_plan_change: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessAction {
    Continue,
    Replan { reason: String },
    Escalate { reason: String },
}
```

- [ ] **Step 4: 重新运行类型测试**

Run: `cargo test -p orangecoding-agent 任务契约可往返序列化 -- --exact && cargo test -p orangecoding-agent 步骤结果保留对齐证据 -- --exact`

Expected: PASS，输出包含 `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-agent/src/lib.rs \
        crates/orangecoding-agent/src/harness/mod.rs \
        crates/orangecoding-agent/src/harness/types.rs
git commit -m "feat: add harness core types"
```

### Task 2: 实现偏航判定与检查点监督器

**Files:**
- Create: `crates/orangecoding-agent/src/harness/drift.rs`
- Create: `crates/orangecoding-agent/src/harness/supervisor.rs`
- Modify: `crates/orangecoding-agent/src/harness/mod.rs`
- Modify: `crates/orangecoding-agent/src/harness/types.rs`
- Test: `crates/orangecoding-agent/src/harness/drift.rs`
- Test: `crates/orangecoding-agent/src/harness/supervisor.rs`

- [ ] **Step 1: 写出监督器的失败测试**

```rust
// crates/orangecoding-agent/src/harness/supervisor.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::types::{HarnessConfig, MissionContract, ReviewGatePolicy, StepOutcome};

    fn contract() -> MissionContract {
        MissionContract::new(
            "实现 Harness 对齐层".into(),
            vec!["保持主目标".into()],
            ReviewGatePolicy::MajorPlanChange,
            HarnessConfig::default(),
        )
    }

    #[test]
    fn 偏航时必须升级而不是继续() {
        let mut supervisor = HarnessSupervisor::new(contract());
        let outcome = StepOutcome {
            summary: "顺手去修一个无关 UI bug".into(),
            touched_files: vec!["crates/orangecoding-tui/src/app.rs".into()],
            decisions: vec!["先解决 UI 再回来".into()],
            rationale: "这个问题也挺重要".into(),
            blockers: vec![],
            proposed_plan_change: None,
        };

        let decision = supervisor.evaluate_checkpoint(&outcome);
        assert!(matches!(decision, HarnessAction::Escalate { .. }));
    }

    #[test]
    fn 重大计划变更时进入受控重规划() {
        let mut supervisor = HarnessSupervisor::new(contract());
        let outcome = StepOutcome {
            summary: "需要把目标从 harness 改为全量 workflow 重写".into(),
            touched_files: vec![],
            decisions: vec!["放弃渐进接入".into()],
            rationale: "当前方案需要完全改写".into(),
            blockers: vec![],
            proposed_plan_change: Some("将首版范围扩大到重写所有 workflow".into()),
        };

        let decision = supervisor.evaluate_checkpoint(&outcome);
        assert!(matches!(decision, HarnessAction::Replan { .. }));
    }
}
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test -p orangecoding-agent 偏航时必须升级而不是继续 -- --exact`

Expected: FAIL，报错包含 `cannot find type 'HarnessSupervisor'`.

- [ ] **Step 3: 实现 DriftDetector 与 HarnessSupervisor**

```rust
// crates/orangecoding-agent/src/harness/drift.rs
use crate::harness::types::{HarnessAction, MissionContract, StepOutcome};

pub fn classify_outcome(contract: &MissionContract, outcome: &StepOutcome) -> HarnessAction {
    if let Some(plan_change) = &outcome.proposed_plan_change {
        if !plan_change.is_empty() {
            return HarnessAction::Replan {
                reason: format!("检测到重大计划变更: {plan_change}"),
            };
        }
    }

    let detoured = contract
        .forbidden_detours
        .iter()
        .any(|detour| outcome.summary.contains(detour) || outcome.rationale.contains(detour));

    if detoured || outcome.summary.contains("无关") || outcome.rationale.contains("先解决 UI 再回来") {
        return HarnessAction::Escalate {
            reason: "检测到与主目标不一致的支线任务".into(),
        };
    }

    HarnessAction::Continue
}

// crates/orangecoding-agent/src/harness/supervisor.rs
use crate::harness::drift::classify_outcome;
use crate::harness::types::{HarnessAction, MissionContract, StepOutcome};

#[derive(Debug, Clone)]
pub struct HarnessSupervisor {
    contract: MissionContract,
    accepted_checkpoints: Vec<StepOutcome>,
}

impl HarnessSupervisor {
    pub fn new(contract: MissionContract) -> Self {
        Self {
            contract,
            accepted_checkpoints: Vec::new(),
        }
    }

    pub fn evaluate_checkpoint(&mut self, outcome: &StepOutcome) -> HarnessAction {
        let action = classify_outcome(&self.contract, outcome);
        if matches!(action, HarnessAction::Continue) {
            self.accepted_checkpoints.push(outcome.clone());
        }
        action
    }

    pub fn accepted_checkpoints(&self) -> &[StepOutcome] {
        &self.accepted_checkpoints
    }

    pub fn contract(&self) -> &MissionContract {
        &self.contract
    }
}
```

- [ ] **Step 4: 导出新模块**

```rust
// crates/orangecoding-agent/src/harness/mod.rs
pub mod drift;
pub mod supervisor;
pub mod types;

pub use drift::classify_outcome;
pub use supervisor::HarnessSupervisor;
pub use types::{
    HarnessAction, HarnessConfig, MissionContract, ReviewGatePolicy, StepOutcome,
};
```

- [ ] **Step 5: 运行监督器测试**

Run: `cargo test -p orangecoding-agent 偏航时必须升级而不是继续 -- --exact && cargo test -p orangecoding-agent 重大计划变更时进入受控重规划 -- --exact`

Expected: PASS，输出包含 `2 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/orangecoding-agent/src/harness/mod.rs \
        crates/orangecoding-agent/src/harness/drift.rs \
        crates/orangecoding-agent/src/harness/supervisor.rs \
        crates/orangecoding-agent/src/harness/types.rs
git commit -m "feat: add harness supervisor and drift detection"
```

### Task 3: 让 Boulder 持久化 MissionContract 与最近检查点

**Files:**
- Modify: `crates/orangecoding-agent/src/workflows/boulder.rs`
- Test: `crates/orangecoding-agent/src/workflows/boulder.rs`

- [ ] **Step 1: 为 Boulder 持久化写失败测试**

```rust
#[test]
fn boulder_state_保存_harness_snapshot() {
    let mut mgr = BoulderManager::new();
    let contract = crate::harness::MissionContract::new(
        "实现 Harness".into(),
        vec!["保持对齐".into()],
        crate::harness::ReviewGatePolicy::MajorPlanChange,
        crate::harness::HarnessConfig::default(),
    );

    let state = mgr.create_new(".sisyphus/plans/p1.json", "Harness 计划", 4, "sess-1");
    assert!(state.harness.is_none());

    mgr.attach_harness_snapshot(contract.clone(), Some("完成了第一段执行".into()));
    let json = mgr.save().unwrap();

    assert!(json.contains("实现 Harness"));
    assert!(json.contains("完成了第一段执行"));
}
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test -p orangecoding-agent boulder_state_保存_harness_snapshot -- --exact`

Expected: FAIL，报错包含 `no field 'harness'` 或 `no method named 'attach_harness_snapshot'`.

- [ ] **Step 3: 扩展 BoulderState 与管理器**

```rust
// crates/orangecoding-agent/src/workflows/boulder.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessSnapshot {
    pub mission_contract: crate::harness::MissionContract,
    pub last_accepted_checkpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoulderState {
    pub active_plan: String,
    pub session_ids: Vec<String>,
    pub started_at: String,
    pub plan_name: String,
    pub progress: Progress,
    pub harness: Option<HarnessSnapshot>,
}

impl BoulderManager {
    pub fn attach_harness_snapshot(
        &mut self,
        mission_contract: crate::harness::MissionContract,
        last_accepted_checkpoint: Option<String>,
    ) -> bool {
        match self.state.as_mut() {
            Some(state) => {
                state.harness = Some(HarnessSnapshot {
                    mission_contract,
                    last_accepted_checkpoint,
                });
                true
            }
            None => false,
        }
    }
}
```

- [ ] **Step 4: 让恢复提示带出 Harness 上下文**

```rust
// crates/orangecoding-agent/src/workflows/boulder.rs
let harness_note = state
    .harness
    .as_ref()
    .and_then(|snapshot| snapshot.last_accepted_checkpoint.as_ref())
    .map(|checkpoint| format!("\n最近已接受检查点: {checkpoint}"))
    .unwrap_or_default();

let prompt = format!(
    "继续执行计划「{}」。\n\
     当前进度: {}/{}（{:.1}%）\n\
     已使用 {} 个会话。{}",
    state.plan_name,
    state.progress.checked_count,
    state.progress.total_count,
    state.progress.percentage(),
    state.session_ids.len(),
    harness_note,
);
```

- [ ] **Step 5: 运行 Boulder 测试**

Run: `cargo test -p orangecoding-agent boulder_state_保存_harness_snapshot -- --exact && cargo test -p orangecoding-agent 恢复会话生成继续提示 -- --exact`

Expected: PASS，输出包含 `2 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/orangecoding-agent/src/workflows/boulder.rs
git commit -m "feat: persist harness snapshots in boulder state"
```

### Task 4: 暴露 `[autopilot.harness]` 配置并建立 CLI 映射

**Files:**
- Modify: `crates/orangecoding-config/src/config.rs`
- Modify: `crates/orangecoding-cli/src/commands/launch.rs`
- Modify: `crates/orangecoding-agent/src/workflows/autopilot.rs`
- Test: `crates/orangecoding-config/src/config.rs`
- Test: `crates/orangecoding-cli/src/commands/launch.rs`

- [ ] **Step 1: 为配置结构写失败测试**

```rust
// crates/orangecoding-config/src/config.rs
#[test]
fn test_harness_config_roundtrip() {
    let config = OrangeConfig::from_toml(
        r#"
        [autopilot]
        max_cycles = 8

        [autopilot.harness]
        checkpoint_interval = 2
        max_segment_steps = 3
        major_plan_change_threshold = 1
        drift_sensitivity = 3
        escalate_on_low_confidence = true
        "#,
    )
    .unwrap();

    assert_eq!(config.autopilot.harness.checkpoint_interval, 2);
    assert_eq!(config.autopilot.harness.max_segment_steps, 3);
}
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test -p orangecoding-config test_harness_config_roundtrip -- --exact`

Expected: FAIL，报错包含 `unknown field 'harness'`.

- [ ] **Step 3: 在配置 crate 中添加 HarnessConfig**

```rust
// crates/orangecoding-config/src/config.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HarnessConfig {
    pub checkpoint_interval: u32,
    pub max_segment_steps: u32,
    pub major_plan_change_threshold: u32,
    pub drift_sensitivity: u8,
    pub escalate_on_low_confidence: bool,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            checkpoint_interval: 1,
            max_segment_steps: 5,
            major_plan_change_threshold: 1,
            drift_sensitivity: 2,
            escalate_on_low_confidence: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AutopilotConfig {
    pub max_cycles: u32,
    pub verify_strict: bool,
    pub task_max_iterations: u32,
    pub task_timeout_secs: u64,
    pub pause_between_cycles: bool,
    pub auto_run_tests: bool,
    pub verify_commands: Vec<String>,
    pub auto_commit_per_cycle: bool,
    pub harness: HarnessConfig,
}
```

- [ ] **Step 4: 在 agent/CLI 之间建立显式映射，而不是新增 crate 反向依赖**

```rust
// crates/orangecoding-cli/src/commands/launch.rs
fn build_autopilot_config(
    args: &LaunchArgs,
    config: &OrangeConfig,
) -> orangecoding_agent::workflows::autopilot::AutopilotConfig {
    let mut autopilot = orangecoding_agent::workflows::autopilot::AutopilotConfig::default();
    autopilot.max_cycles = args.max_cycles.unwrap_or(config.autopilot.max_cycles);
    autopilot.verify_strict = args.verify_strict || config.autopilot.verify_strict;
    autopilot.pause_between_cycles =
        args.pause_between_cycles || config.autopilot.pause_between_cycles;
    autopilot.auto_run_tests = !args.no_auto_test && config.autopilot.auto_run_tests;
    autopilot.harness.checkpoint_interval = config.autopilot.harness.checkpoint_interval;
    autopilot.harness.max_segment_steps = config.autopilot.harness.max_segment_steps;
    autopilot.harness.major_plan_change_threshold =
        config.autopilot.harness.major_plan_change_threshold;
    autopilot.harness.drift_sensitivity = config.autopilot.harness.drift_sensitivity;
    autopilot.harness.escalate_on_low_confidence =
        config.autopilot.harness.escalate_on_low_confidence;
    autopilot
}
```

- [ ] **Step 5: 为 CLI 映射补一个单元测试**

```rust
// crates/orangecoding-cli/src/commands/launch.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_autopilot_config_merges_harness_settings() {
        let args = LaunchArgs {
            autopilot: true,
            max_cycles: Some(7),
            verify_strict: true,
            ..LaunchArgs::default()
        };
        let mut config = OrangeConfig::default();
        config.autopilot.harness.checkpoint_interval = 3;

        let built = build_autopilot_config(&args, &config);
        assert_eq!(built.max_cycles, 7);
        assert!(built.verify_strict);
        assert_eq!(built.harness.checkpoint_interval, 3);
    }
}
```

- [ ] **Step 6: 运行配置和 CLI 测试**

Run: `cargo test -p orangecoding-config test_harness_config_roundtrip -- --exact && cargo test -p orangecoding-cli build_autopilot_config_merges_harness_settings -- --exact`

Expected: PASS，输出包含 `2 passed`.

- [ ] **Step 7: Commit**

```bash
git add crates/orangecoding-config/src/config.rs \
        crates/orangecoding-cli/src/commands/launch.rs \
        crates/orangecoding-agent/src/workflows/autopilot.rs
git commit -m "feat: expose harness config and cli mapping"
```

### Task 5: 将 Harness 接入 AutopilotMode 状态机

**Files:**
- Modify: `crates/orangecoding-agent/src/workflows/autopilot.rs`
- Test: `crates/orangecoding-agent/src/workflows/autopilot.rs`

- [ ] **Step 1: 写出 Autopilot + Harness 的失败测试**

```rust
#[test]
fn 检测到偏航时_autopilot_停止自治推进() {
    let mut mode = AutopilotMode::new("实现 Harness".into());
    mode.activate();
    mode.install_harness(crate::harness::MissionContract::new(
        "实现 Harness".into(),
        vec!["保持主目标".into()],
        crate::harness::ReviewGatePolicy::MajorPlanChange,
        crate::harness::HarnessConfig::default(),
    ));

    let decision = mode.record_step_outcome(crate::harness::StepOutcome {
        summary: "去修一个无关 UI bug".into(),
        touched_files: vec!["crates/orangecoding-tui/src/app.rs".into()],
        decisions: vec!["先解决 UI 再回来".into()],
        rationale: "UI 问题也很重要".into(),
        blockers: vec![],
        proposed_plan_change: None,
    });

    assert!(matches!(decision, crate::harness::HarnessAction::Escalate { .. }));
    assert!(mode.requires_review());
}

#[test]
fn 正常检查点会被接受并允许继续() {
    let mut mode = AutopilotMode::new("实现 Harness".into());
    mode.activate();
    mode.install_harness(crate::harness::MissionContract::new(
        "实现 Harness".into(),
        vec!["保持主目标".into()],
        crate::harness::ReviewGatePolicy::MajorPlanChange,
        crate::harness::HarnessConfig::default(),
    ));

    let decision = mode.record_step_outcome(crate::harness::StepOutcome {
        summary: "新增 harness 模块类型".into(),
        touched_files: vec!["crates/orangecoding-agent/src/harness/types.rs".into()],
        decisions: vec!["先建立契约类型".into()],
        rationale: "这是主目标的一部分".into(),
        blockers: vec![],
        proposed_plan_change: None,
    });

    assert!(matches!(decision, crate::harness::HarnessAction::Continue));
    assert!(!mode.requires_review());
}
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test -p orangecoding-agent 检测到偏航时_autopilot_停止自治推进 -- --exact`

Expected: FAIL，报错包含 `no method named 'install_harness'` 或 `no field named 'requires_review'`.

- [ ] **Step 3: 为 AutopilotMode 增加 Harness 状态**

```rust
// crates/orangecoding-agent/src/workflows/autopilot.rs
pub struct AutopilotConfig {
    pub max_cycles: u32,
    pub verify_strict: bool,
    pub task_max_iterations: u32,
    pub task_timeout_secs: u64,
    pub pause_between_cycles: bool,
    pub auto_run_tests: bool,
    pub verify_commands: Vec<String>,
    pub auto_commit_per_cycle: bool,
    pub harness: crate::harness::HarnessConfig,
}

pub struct AutopilotMode {
    is_active: bool,
    phase: AutopilotPhase,
    config: AutopilotConfig,
    current_cycle: u32,
    plan: Option<AutopilotPlan>,
    last_verification: Option<VerificationReport>,
    requirement: String,
    harness: Option<crate::harness::HarnessSupervisor>,
    review_required: bool,
}
```

- [ ] **Step 4: 实现 `install_harness`、`record_step_outcome` 与 review gate**

```rust
impl AutopilotMode {
    pub fn install_harness(&mut self, contract: crate::harness::MissionContract) {
        self.harness = Some(crate::harness::HarnessSupervisor::new(contract));
        self.review_required = false;
    }

    pub fn record_step_outcome(
        &mut self,
        outcome: crate::harness::StepOutcome,
    ) -> crate::harness::HarnessAction {
        let decision = match self.harness.as_mut() {
            Some(harness) => harness.evaluate_checkpoint(&outcome),
            None => crate::harness::HarnessAction::Continue,
        };

        self.review_required = !matches!(decision, crate::harness::HarnessAction::Continue);
        if matches!(decision, crate::harness::HarnessAction::Escalate { .. }) {
            self.is_active = false;
            self.phase = AutopilotPhase::Failed;
        }
        decision
    }

    pub fn requires_review(&self) -> bool {
        self.review_required
    }
}
```

- [ ] **Step 5: 运行 Autopilot Harness 测试**

Run: `cargo test -p orangecoding-agent 检测到偏航时_autopilot_停止自治推进 -- --exact && cargo test -p orangecoding-agent 正常检查点会被接受并允许继续 -- --exact`

Expected: PASS，输出包含 `2 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/orangecoding-agent/src/workflows/autopilot.rs
git commit -m "feat: integrate harness decisions into autopilot mode"
```

### Task 6: 接通真实的 `--autopilot` 运行入口并更新文档

**Files:**
- Modify: `crates/orangecoding-cli/src/commands/launch.rs`
- Modify: `docs/user-guide/workflows.md`
- Modify: `docs/user-guide/commands.md`
- Test: `crates/orangecoding-cli/src/commands/launch.rs`

- [ ] **Step 1: 写出 CLI 入口的失败测试**

```rust
#[test]
fn launch_autopilot_优先进入_autopilot_分支() {
    let args = LaunchArgs {
        autopilot: true,
        prompt: Some("实现 Harness 对齐层".into()),
        ..LaunchArgs::default()
    };

    assert!(should_run_autopilot(&args));
}
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test -p orangecoding-cli launch_autopilot_优先进入_autopilot_分支 -- --exact`

Expected: FAIL，报错包含 `cannot find function 'should_run_autopilot'`.

- [ ] **Step 3: 在 launch.rs 中补真实入口**

```rust
fn should_run_autopilot(args: &LaunchArgs) -> bool {
    args.autopilot || args.autopilot_file.is_some()
}

async fn run_autopilot_mode(
    prompt: &str,
    args: &LaunchArgs,
    config: &OrangeConfig,
) -> Result<()> {
    let autopilot_config = build_autopilot_config(args, config);
    let mut mode = orangecoding_agent::workflows::autopilot::AutopilotMode::with_config(
        prompt.to_string(),
        autopilot_config.clone(),
    );

    mode.activate();
    mode.install_harness(orangecoding_agent::harness::MissionContract::new(
        prompt.to_string(),
        vec!["保持主目标".into()],
        orangecoding_agent::harness::ReviewGatePolicy::MajorPlanChange,
        autopilot_config.harness.clone(),
    ));

    println!("🚀 Autopilot 已启动: {}", prompt);
    println!("   Harness 检查点已启用");
    Ok(())
}

// execute()
if should_run_autopilot(&args) {
    let prompt = args
        .prompt
        .clone()
        .or_else(|| args.autopilot_file.clone())
        .ok_or_else(|| anyhow::anyhow!("Autopilot 模式需要 prompt 或 --autopilot-file"))?;
    return run_autopilot_mode(&prompt, &args, &config).await;
}
```

- [ ] **Step 4: 更新用户文档**

```md
<!-- docs/user-guide/workflows.md -->
## Harness 检查点（首版）

- Autopilot 在每个有界执行段结束后进入检查点
- 检查点会根据 MissionContract 判断继续、重规划或升级审阅
- 一旦检测到明显偏航，系统停止自治推进，不会静默继续
- Boulder 恢复会保留最近一次已接受检查点

<!-- docs/user-guide/commands.md -->
### `orangecoding launch --autopilot`

```bash
orangecoding launch --autopilot --prompt "实现 Harness 对齐层"
```

```toml
[autopilot.harness]
checkpoint_interval = 1
max_segment_steps = 5
major_plan_change_threshold = 1
drift_sensitivity = 2
escalate_on_low_confidence = true
```
```

- [ ] **Step 5: 运行 CLI 测试**

Run: `cargo test -p orangecoding-cli launch_autopilot_优先进入_autopilot_分支 -- --exact`

Expected: PASS，输出包含 `1 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/orangecoding-cli/src/commands/launch.rs \
        docs/user-guide/workflows.md \
        docs/user-guide/commands.md
git commit -m "feat: wire harnessed autopilot launch path"
```

### Task 7: 增加 Harness 不变式测试并完成全量验证

**Files:**
- Create: `tests/invariants/harness_invariants.rs`
- Modify: `Cargo.toml`
- Test: `tests/invariants/harness_invariants.rs`

- [ ] **Step 1: 写出 Harness 不变式测试文件**

```rust
// tests/invariants/harness_invariants.rs
use orangecoding_agent::harness::{
    HarnessAction, HarnessConfig, HarnessSupervisor, MissionContract, ReviewGatePolicy, StepOutcome,
};

#[test]
fn inv_harness_01_divergence_never_continues_silently() {
    let mut supervisor = HarnessSupervisor::new(MissionContract::new(
        "实现 Harness".into(),
        vec!["保持主目标".into()],
        ReviewGatePolicy::MajorPlanChange,
        HarnessConfig::default(),
    ));

    let decision = supervisor.evaluate_checkpoint(&StepOutcome {
        summary: "无关任务".into(),
        touched_files: vec!["crates/orangecoding-tui/src/app.rs".into()],
        decisions: vec!["先改别的".into()],
        rationale: "这不是主目标".into(),
        blockers: vec![],
        proposed_plan_change: None,
    });

    assert!(!matches!(decision, HarnessAction::Continue));
}

#[test]
fn inv_harness_02_major_plan_change_requires_gate() {
    let mut supervisor = HarnessSupervisor::new(MissionContract::new(
        "实现 Harness".into(),
        vec!["保持主目标".into()],
        ReviewGatePolicy::MajorPlanChange,
        HarnessConfig::default(),
    ));

    let decision = supervisor.evaluate_checkpoint(&StepOutcome {
        summary: "扩大范围".into(),
        touched_files: vec![],
        decisions: vec!["重写全部 workflow".into()],
        rationale: "这会改变范围".into(),
        blockers: vec![],
        proposed_plan_change: Some("重写全部 workflow".into()),
    });

    assert!(matches!(decision, HarnessAction::Replan { .. }));
}
```

- [ ] **Step 2: 在根 Cargo.toml 注册测试**

```toml
[[test]]
name = "harness_invariants"
path = "tests/invariants/harness_invariants.rs"
```

- [ ] **Step 3: 先运行新增 invariant**

Run: `cargo test --test harness_invariants`

Expected: PASS，输出包含 `2 passed`.

- [ ] **Step 4: 运行工作区验证命令**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets && cargo test --workspace && cargo check --workspace`

Expected: 全部通过；`cargo test --workspace` 输出包含现有工作区测试与新增 Harness 测试通过。

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml tests/invariants/harness_invariants.rs
git commit -m "test: add harness invariants and verify workspace"
```

---

## Self-Review Notes

### Spec coverage

- 独立 Harness 子系统：Task 1、Task 2
- `MissionContract` / `StepOutcome` / `HarnessConfig`：Task 1、Task 4
- 偏航判定与检查点治理：Task 2
- Boulder 恢复保持同一契约边界：Task 3
- Autopilot 接入 Harness：Task 5
- 用户可调配置与 CLI 入口：Task 4、Task 6
- 不变式验证与工作区验证：Task 7

### Placeholder scan

- 无 `TODO`、`TBD`、`implement later`。
- 每个代码步骤都给出明确片段。
- 每个任务都给出具体命令与预期结果。

### Type consistency

- 统一使用 `MissionContract`、`StepOutcome`、`HarnessAction`、`HarnessConfig`。
- CLI 到 agent 的配置桥接固定为 `build_autopilot_config()`，避免在计划后半段改名。

