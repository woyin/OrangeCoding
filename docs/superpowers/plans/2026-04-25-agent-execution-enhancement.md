# Agent Execution Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Strengthen OrangeCoding’s Normal/Plan/Autopilot behavior with instruction anchoring, dynamic step budgets, mode-specific prompts, and `orange.json` model routing.

**Architecture:** Add small guardrail modules to `orangecoding-agent` and wire them into the existing CLI/TUI loops that currently call providers directly. Keep `UltraWork` unchanged. The implementation is deliberately incremental: pure state-machine modules first, then prompt/mode routing, then loop integration and verification.

**Tech Stack:** Rust 2021, serde/serde_json, tokio, ratatui TUI state, existing `orangecoding-ai::ChatMessage` / `ChatOptions`, existing `~/.config/orangecoding` config convention.

---

## File Structure

- Create `crates/orangecoding-agent/src/instruction_anchor.rs`
  - Stores the original instruction, counts execution steps, emits a system re-anchor message every configured interval.
- Create `crates/orangecoding-agent/src/step_budget.rs`
  - Tracks action signatures, extends soft budgets, and hard-stops only on repeated identical actions.
- Create `crates/orangecoding-agent/src/model_router.rs`
  - Loads `~/.config/orangecoding/orange.json`, parses routing rules, classifies task difficulty/type, chooses a model.
- Create `crates/orangecoding-agent/src/execution_prompt.rs`
  - Centralizes strengthened system prompts for Exec, Plan, Autopilot, and unchanged UltraWork.
- Modify `crates/orangecoding-agent/src/lib.rs`
  - Export the four new modules.
- Modify `crates/orangecoding-agent/src/agent_loop.rs`
  - Use the guardrail primitives in the shared agent loop used by `serve`.
- Modify `crates/orangecoding-cli/src/commands/launch.rs`
  - Use mode prompts, route models, inject anchors, and apply soft step budgets in single-shot, TUI, and text loops.
- Modify `crates/orangecoding-tui/src/app.rs`
  - Update mode labels/descriptions and slash menu wording to reflect Normal=Exec, Plan=Plan, Autopilot=long task.
- Modify `crates/orangecoding-cli/src/slash_builtins.rs`
  - Update help text for the clarified modes.

---

### Task 1: Add InstructionAnchor

**Files:**
- Create: `crates/orangecoding-agent/src/instruction_anchor.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Add this test module to the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试按间隔触发回锚消息() {
        let mut anchor = InstructionAnchor::new("修复登录错误", 3);
        assert!(anchor.on_step().is_none());
        assert!(anchor.on_step().is_none());

        let message = anchor.on_step().expect("第三步应该触发回锚");
        assert!(message.contains("修复登录错误"));
        assert!(message.contains("指令回锚"));
    }

    #[test]
    fn 测试零间隔不会触发回锚() {
        let mut anchor = InstructionAnchor::new("任意任务", 0);
        assert!(anchor.on_step().is_none());
        assert!(anchor.on_step().is_none());
    }

    #[test]
    fn 测试重置会替换原始指令并清零计数() {
        let mut anchor = InstructionAnchor::new("旧任务", 2);
        assert!(anchor.on_step().is_none());
        anchor.reset("新任务");
        assert!(anchor.on_step().is_none());

        let message = anchor.on_step().expect("重置后的第二步应该触发");
        assert!(message.contains("新任务"));
        assert!(!message.contains("旧任务"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orangecoding-agent instruction_anchor -- --nocapture`

Expected: FAIL because `InstructionAnchor` is not implemented.

- [ ] **Step 3: Implement InstructionAnchor**

Add:

```rust
//! 指令回锚模块。

/// 按固定步数间隔把原始用户指令重新注入上下文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionAnchor {
    original_instruction: String,
    anchor_interval: u32,
    step_counter: u32,
}

impl InstructionAnchor {
    pub fn new(instruction: &str, interval: u32) -> Self {
        Self {
            original_instruction: instruction.trim().to_string(),
            anchor_interval: interval,
            step_counter: 0,
        }
    }

    pub fn on_step(&mut self) -> Option<String> {
        if self.anchor_interval == 0 || self.original_instruction.is_empty() {
            return None;
        }

        self.step_counter = self.step_counter.saturating_add(1);
        if self.step_counter % self.anchor_interval == 0 {
            Some(format!(
                "[指令回锚]\n原始用户指令：{}\n请静默检查当前行为是否偏离原始指令；如有偏离，立即纠正并继续执行，不要向用户额外汇报。",
                self.original_instruction
            ))
        } else {
            None
        }
    }

    pub fn reset(&mut self, new_instruction: &str) {
        self.original_instruction = new_instruction.trim().to_string();
        self.step_counter = 0;
    }
}
```

Also add to `crates/orangecoding-agent/src/lib.rs`:

```rust
/// 指令回锚模块
pub mod instruction_anchor;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orangecoding-agent instruction_anchor -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-agent/src/instruction_anchor.rs crates/orangecoding-agent/src/lib.rs
git commit -m "feat: add instruction anchor guardrail"
```

---

### Task 2: Add StepBudgetGuard

**Files:**
- Create: `crates/orangecoding-agent/src/step_budget.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试预算耗尽时自动扩展而不是停止() {
        let mut guard = StepBudgetGuard::new(2, 3);
        assert_eq!(guard.tick("read:a"), BudgetDecision::Continue);
        assert_eq!(guard.tick("read:b"), BudgetDecision::Continue);
        assert_eq!(guard.tick("read:c"), BudgetDecision::BudgetExtended { new_budget: 3 });
        assert_eq!(guard.current(), 3);
        assert_eq!(guard.budget(), 3);
    }

    #[test]
    fn 测试重复动作达到阈值时硬停止() {
        let mut guard = StepBudgetGuard::new(100, 3);
        assert_eq!(guard.tick("bash:cargo-test"), BudgetDecision::Continue);
        assert_eq!(guard.tick("bash:cargo-test"), BudgetDecision::Continue);

        match guard.tick("bash:cargo-test") {
            BudgetDecision::HardStop { reason } => assert!(reason.contains("重复")),
            other => panic!("expected hard stop, got {other:?}"),
        }
    }

    #[test]
    fn 测试不同动作会重置重复检测() {
        let mut guard = StepBudgetGuard::new(100, 3);
        assert_eq!(guard.tick("a"), BudgetDecision::Continue);
        assert_eq!(guard.tick("a"), BudgetDecision::Continue);
        assert_eq!(guard.tick("b"), BudgetDecision::Continue);
        assert_eq!(guard.tick("a"), BudgetDecision::Continue);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orangecoding-agent step_budget -- --nocapture`

Expected: FAIL because `StepBudgetGuard` is not implemented.

- [ ] **Step 3: Implement StepBudgetGuard**

Add a focused implementation:

```rust
//! 步数预算守卫。

use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetDecision {
    Continue,
    BudgetExtended { new_budget: u32 },
    HardStop { reason: String },
}

#[derive(Debug, Clone)]
pub struct StepBudgetGuard {
    budget: u32,
    current: u32,
    loop_threshold: u32,
    recent_actions: VecDeque<String>,
}

impl StepBudgetGuard {
    pub fn new(budget: u32, loop_threshold: u32) -> Self {
        Self {
            budget: budget.max(1),
            current: 0,
            loop_threshold: loop_threshold.max(2),
            recent_actions: VecDeque::new(),
        }
    }

    pub fn tick(&mut self, action_signature: &str) -> BudgetDecision {
        self.current = self.current.saturating_add(1);
        self.recent_actions.push_back(action_signature.to_string());
        while self.recent_actions.len() > self.loop_threshold as usize {
            self.recent_actions.pop_front();
        }

        if self.recent_actions.len() == self.loop_threshold as usize
            && self.recent_actions.iter().all(|item| item == action_signature)
        {
            return BudgetDecision::HardStop {
                reason: format!("检测到动作重复 {} 次：{}", self.loop_threshold, action_signature),
            };
        }

        if self.current > self.budget {
            self.budget = self.budget.saturating_add((self.budget / 2).max(1));
            BudgetDecision::BudgetExtended { new_budget: self.budget }
        } else {
            BudgetDecision::Continue
        }
    }

    pub fn current(&self) -> u32 {
        self.current
    }

    pub fn budget(&self) -> u32 {
        self.budget
    }
}
```

Also add to `lib.rs`:

```rust
/// 步数预算守卫模块
pub mod step_budget;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orangecoding-agent step_budget -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-agent/src/step_budget.rs crates/orangecoding-agent/src/lib.rs
git commit -m "feat: add step budget guardrail"
```

---

### Task 3: Add ModelRouter and orange.json runtime config

**Files:**
- Create: `crates/orangecoding-agent/src/model_router.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Add tests for exact match, wildcard fallback, missing file defaults, and task classification:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试精确匹配优先于通配规则() {
        let router = ModelRouter {
            rules: vec![
                RoutingRule::new(None, None, "fallback-rule"),
                RoutingRule::new(Some(Difficulty::Hard), None, "hard-any"),
                RoutingRule::new(Some(Difficulty::Hard), Some(TaskType::Code), "hard-code"),
            ],
            fallback: "fallback-model".to_string(),
        };

        assert_eq!(router.route(Difficulty::Hard, TaskType::Code), "hard-code");
        assert_eq!(router.route(Difficulty::Hard, TaskType::Chat), "hard-any");
        assert_eq!(router.route(Difficulty::Easy, TaskType::Chat), "fallback-rule");
    }

    #[test]
    fn 测试缺失配置文件使用默认值() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orange.json");
        let config = OrangeRuntimeConfig::load_or_default(&path);
        assert_eq!(config.execution.anchor_interval_steps, 5);
        assert_eq!(config.execution.step_budget_initial, 100);
        assert_eq!(config.model_router().route(Difficulty::Epic, TaskType::Write), "claude-opus-4-7");
    }

    #[test]
    fn 测试从_json_读取路由规则() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orange.json");
        std::fs::write(
            &path,
            r#"{
              "model_routing": {
                "rules": [
                  {"difficulty": "easy", "task_type": "chat", "model": "cheap-chat"},
                  {"difficulty": "epic", "task_type": "*", "model": "strong-model"}
                ],
                "fallback_model": "fallback"
              },
              "execution": {
                "anchor_interval_steps": 7,
                "step_budget_initial": 200,
                "loop_detection_threshold": 4
              }
            }"#,
        )
        .unwrap();

        let config = OrangeRuntimeConfig::load(&path).unwrap();
        assert_eq!(config.execution.anchor_interval_steps, 7);
        assert_eq!(config.model_router().route(Difficulty::Epic, TaskType::Code), "strong-model");
    }

    #[test]
    fn 测试任务文本分类() {
        assert_eq!(TaskType::infer("请修复 Rust 编译错误"), TaskType::Code);
        assert_eq!(TaskType::infer("帮我写一份设计文档"), TaskType::Write);
        assert_eq!(Difficulty::infer("difficulty: epic 请实现完整平台"), Difficulty::Epic);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orangecoding-agent model_router -- --nocapture`

Expected: FAIL because `ModelRouter` is not implemented.

- [ ] **Step 3: Implement ModelRouter**

Implement these public types and methods:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty { Easy, Medium, Hard, Epic }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskType { Code, Write, Analyze, Chat }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingRule {
    pub difficulty: Option<Difficulty>,
    pub task_type: Option<TaskType>,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct ModelRouter {
    pub rules: Vec<RoutingRule>,
    pub fallback: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExecutionRuntimeConfig {
    pub anchor_interval_steps: u32,
    pub step_budget_initial: u32,
    pub loop_detection_threshold: u32,
}

#[derive(Debug, Clone)]
pub struct OrangeRuntimeConfig {
    pub routing: ModelRouter,
    pub execution: ExecutionRuntimeConfig,
}
```

Parsing requirements:
- JSON input accepts `task_type: "*"` and `difficulty: "*"` as wildcard values.
- `OrangeRuntimeConfig::default()` matches the design doc schema.
- `OrangeRuntimeConfig::load(path)` returns `orangecoding_core::Result<Self>` and surfaces invalid JSON as `OrangeError::config`.
- `OrangeRuntimeConfig::load_or_default(path)` returns defaults if the file is missing or invalid, with a `tracing::warn!` for invalid files.
- `TaskType::infer()` uses simple keyword heuristics: code/build/test/fix/编译/代码 -> Code, doc/write/文档/撰写 -> Write, analyze/分析/排查 -> Analyze, otherwise Chat.
- `Difficulty::infer()` first honors `difficulty: easy|medium|hard|epic`, then uses length and keywords as heuristics.

Also add to `lib.rs`:

```rust
/// 模型路由模块
pub mod model_router;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orangecoding-agent model_router -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-agent/src/model_router.rs crates/orangecoding-agent/src/lib.rs
git commit -m "feat: add orange runtime model router"
```

---

### Task 4: Add mode-specific execution prompts

**Files:**
- Create: `crates/orangecoding-agent/src/execution_prompt.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试_exec_prompt_强调严格执行和决策点询问() {
        let prompt = build_system_prompt(ExecutionMode::Exec);
        assert!(prompt.contains("[EXEC MODE - 严格执行]"));
        assert!(prompt.contains("决策分叉"));
    }

    #[test]
    fn 测试_plan_prompt_包含执行策略询问() {
        let prompt = build_system_prompt(ExecutionMode::Plan);
        assert!(prompt.contains("结构化计划"));
        assert!(prompt.contains("一步到位"));
        assert!(prompt.contains("Exec 模式"));
    }

    #[test]
    fn 测试_autopilot_prompt_包含任务宪法() {
        let prompt = build_system_prompt(ExecutionMode::Autopilot);
        assert!(prompt.contains("[MISSION LOCK]"));
        assert!(prompt.contains("指令回锚"));
        assert!(prompt.contains("步数不是硬限制"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orangecoding-agent execution_prompt -- --nocapture`

Expected: FAIL because prompt module is missing.

- [ ] **Step 3: Implement execution_prompt**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Exec,
    Plan,
    Autopilot,
    UltraWork,
}

pub fn build_system_prompt(mode: ExecutionMode) -> String {
    let mission = "[MISSION LOCK]\n你是 OrangeCoding，一个不间断执行的编码 agent。\n原始用户指令是你的最高法律，任何子任务都不得覆盖它。\n";
    let shared = "\n[TASK DIFFICULTY SIGNAL]\n用户可在指令中注明 difficulty: easy / medium / hard / epic。\n未注明时，根据任务特征自行判断后选择模型。\n";

    match mode {
        ExecutionMode::Exec => format!("{mission}\n[EXEC MODE - 严格执行]\n1. 收到指令立即执行，不做多余规划或解释\n2. 严格遵循指令字面意思\n3. 只有在出现真正的决策分叉（多路径且不可逆）时才暂停询问\n4. 其他一切情况：自行判断，继续执行\n{shared}"),
        ExecutionMode::Plan => format!("{mission}\n[PLAN MODE - 先规划再执行]\n1. 先输出结构化计划：目标、阶段、步骤、验收标准、预估复杂度\n2. 等待用户确认计划\n3. 计划确认后询问执行策略：一步到位（Autopilot）或 Exec 模式\n4. 未确认前不要修改代码\n{shared}"),
        ExecutionMode::Autopilot => format!("{mission}\n[EXECUTION RULES - 适用于 Autopilot 模式]\n1. 永不在任务中途停止等待用户确认\n2. 每完成 5 个步骤，静默执行一次「指令回锚」\n3. 遇到障碍时，先尝试 3 种替代方案，全部失败后才可停止\n4. 步数不是硬限制，任务完成才是终止条件\n\n[SELF-CHECK TEMPLATE]\n① 原始指令要求我做什么？\n② 我现在正在做什么？\n③ 是否偏离？→ 是：立即纠正 / 否：继续\n{shared}"),
        ExecutionMode::UltraWork => "你是 OrangeCoding，保持当前 UltraWork 极限工作模式行为。".to_string(),
    }
}
```

Also add to `lib.rs`:

```rust
/// 执行模式提示词模块
pub mod execution_prompt;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orangecoding-agent execution_prompt -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-agent/src/execution_prompt.rs crates/orangecoding-agent/src/lib.rs
git commit -m "feat: add execution mode prompts"
```

---

### Task 5: Wire guardrails into AgentLoop

**Files:**
- Modify: `crates/orangecoding-agent/src/agent_loop.rs`

- [ ] **Step 1: Write failing tests in `agent_loop.rs`**

Add tests:

```rust
#[tokio::test]
async fn 测试_agent_loop_会注入回锚消息() {
    let provider = Arc::new(MockToolCallProvider::new());
    let executor = create_test_executor();
    let mut context = AgentContext::new(SessionId::new(), PathBuf::from("."));
    context.set_system_prompt("系统提示");
    context.add_user_message("执行长任务");

    let config = AgentLoopConfig {
        max_iterations: 10,
        timeout: Duration::from_secs(30),
        auto_approve_tools: true,
        anchor_interval_steps: 1,
        step_budget_initial: 100,
        loop_detection_threshold: 3,
    };

    let mut agent_loop = AgentLoop::new(AgentId::new(), provider, executor, context, config);
    let (tx, _rx) = mpsc::channel(100);
    let result = agent_loop
        .run(&ChatOptions::with_model("mock-model"), CancellationToken::new(), tx)
        .await
        .unwrap();

    assert!(result.messages.iter().any(|m| {
        m.content.as_deref().unwrap_or_default().contains("[指令回锚]")
    }));
}
```

Update existing struct literals in tests to include the new config fields or use `..Default::default()`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orangecoding-agent agent_loop -- --nocapture`

Expected: FAIL until config fields and loop integration are added.

- [ ] **Step 3: Implement loop integration**

Modify `AgentLoopConfig`:

```rust
pub struct AgentLoopConfig {
    pub max_iterations: u32,
    pub timeout: Duration,
    pub auto_approve_tools: bool,
    pub anchor_interval_steps: u32,
    pub step_budget_initial: u32,
    pub loop_detection_threshold: u32,
}
```

Default values:

```rust
anchor_interval_steps: 5,
step_budget_initial: 100,
loop_detection_threshold: 3,
```

In `run()`:
- Build `InstructionAnchor` from the first user message in context.
- Build `StepBudgetGuard` from config.
- Before each provider call, call `anchor.on_step()` and append returned content as `Message::system(...)`.
- For each tool call, construct `action_signature = format!("{}:{}", tc.function_name, tc.arguments)`.
- If `BudgetDecision::HardStop`, send `AgentEvent::error(...)` and break.
- If `BudgetDecision::BudgetExtended`, log `info!` and continue.
- Keep timeout and cancellation as hard stops.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orangecoding-agent agent_loop -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-agent/src/agent_loop.rs
git commit -m "feat: wire execution guardrails into agent loop"
```

---

### Task 6: Wire mode prompts, model routing, and budgets into CLI/TUI loops

**Files:**
- Modify: `crates/orangecoding-cli/src/commands/launch.rs`

- [ ] **Step 1: Write lightweight unit-testable helpers**

Create private helpers in `launch.rs`:

```rust
fn orange_runtime_config_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".config/orangecoding/orange.json"))
}

fn routed_model<'a>(
    router: &'a orangecoding_agent::model_router::ModelRouter,
    requested_model: &'a str,
    prompt: &str,
) -> &'a str {
    if !requested_model.trim().is_empty() {
        return requested_model;
    }
    let difficulty = orangecoding_agent::model_router::Difficulty::infer(prompt);
    let task_type = orangecoding_agent::model_router::TaskType::infer(prompt);
    router.route(difficulty, task_type)
}
```

Because private async loops are hard to unit-test without extracting many provider mocks into CLI, verify behavior through `cargo check` plus agent module tests.

- [ ] **Step 2: Integrate single-shot mode**

In `run_single_shot()`:
- Load `OrangeRuntimeConfig::load_or_default(orange_runtime_config_path())`.
- Use `ExecutionMode::Exec` prompt by default; use `ExecutionMode::Autopilot` when `args.autopilot` eventually reaches this path.
- Replace the hardcoded system prompt with `build_system_prompt(mode)`.
- Use `InstructionAnchor` and `StepBudgetGuard` in the loop.
- When budget extends, print concise status: `♻️ 步数预算已扩展到 N，继续执行...`.
- When hard-stop triggers, return an error with the reason.
- Select the model using route result only when CLI did not explicitly pass `--model`.

- [ ] **Step 3: Integrate TUI mode**

In `run_tui_mode()`:
- Load `OrangeRuntimeConfig` once before the main loop.
- When sending a user message, map `app.interaction_mode`:
  - `Normal` -> `ExecutionMode::Exec`
  - `Plan` -> `ExecutionMode::Plan`
  - `Autopilot` -> `ExecutionMode::Autopilot`
  - `UltraWork` -> `ExecutionMode::UltraWork`
- Ensure the first message of each conversation includes the mode-specific system prompt.
- In Plan mode, rely on the Plan prompt to ask the user whether to continue with “一步到位” or “Exec 模式”.
- Route model per user message unless the user has manually chosen a model in `/model`.
- Use `InstructionAnchor` and `StepBudgetGuard` in the tool loop.

- [ ] **Step 4: Integrate text mode**

In `run_text_loop()`:
- Use Exec prompt as default system prompt.
- Load runtime config and apply dynamic step budgets.
- Use model routing for each new user input unless explicit model was configured.

- [ ] **Step 5: Run compile check**

Run: `cargo check -p orangecoding-cli -p orangecoding-agent`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/orangecoding-cli/src/commands/launch.rs
git commit -m "feat: apply execution guardrails in launch loops"
```

---

### Task 7: Update TUI and slash command wording

**Files:**
- Modify: `crates/orangecoding-tui/src/app.rs`
- Modify: `crates/orangecoding-cli/src/slash_builtins.rs`

- [ ] **Step 1: Update TUI mode descriptions**

Change `InteractionMode::description()` to:

```rust
InteractionMode::Normal => "Exec 模式 - 严格执行，决策分叉才询问",
InteractionMode::Plan => "Plan 模式 - 先规划，确认后选择执行策略",
InteractionMode::Autopilot => "长任务模式 - 全程自动执行并静默自纠",
InteractionMode::UltraWork => "极限模式 - 暂保持现状",
```

- [ ] **Step 2: Update slash help**

In `format_help()` and TUI `/help`, update mode descriptions:

```text
/mode          切换模式：normal=Exec, plan=先规划, autopilot=长任务
/plan          切换 Plan 模式，规划后选择一步到位或 Exec
```

- [ ] **Step 3: Add or update tests**

Update existing TUI tests that assert descriptions. Add:

```rust
#[test]
fn 测试交互模式描述反映新语义() {
    assert!(InteractionMode::Normal.description().contains("Exec"));
    assert!(InteractionMode::Plan.description().contains("选择执行策略"));
    assert!(InteractionMode::Autopilot.description().contains("长任务"));
    assert!(InteractionMode::UltraWork.description().contains("保持现状"));
}
```

- [ ] **Step 4: Run TUI tests**

Run: `cargo test -p orangecoding-tui 测试交互模式描述反映新语义`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/orangecoding-tui/src/app.rs crates/orangecoding-cli/src/slash_builtins.rs
git commit -m "docs: clarify execution mode wording"
```

---

### Task 8: Full verification and targeted cleanup

**Files:**
- Review all modified files.

- [ ] **Step 1: Run focused tests**

Run:

```bash
cargo test -p orangecoding-agent instruction_anchor step_budget model_router execution_prompt -- --nocapture
cargo test -p orangecoding-tui 测试交互模式描述反映新语义
```

Expected: PASS.

- [ ] **Step 2: Run workspace checks**

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 3: If formatting fails, format and re-check**

Run:

```bash
cargo fmt --all
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 4: Run agent crate tests**

Run:

```bash
cargo test -p orangecoding-agent -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Final commit**

If formatting or cleanup changed files:

```bash
git add crates/orangecoding-agent crates/orangecoding-cli crates/orangecoding-tui
git commit -m "chore: verify execution enhancement integration"
```

---

## Self-Review

**Spec coverage:** The plan covers the design doc’s three modes, task constitution prompt, `InstructionAnchor`, `StepBudgetGuard`, `ModelRouter`, `orange.json`, model difficulty/type routing, and core tests.

**Placeholder scan:** No `TBD`, `TODO`, or unspecified “add tests” steps remain; each task includes concrete files, snippets, commands, and expected results.

**Type consistency:** `ExecutionMode`, `InstructionAnchor`, `StepBudgetGuard`, `BudgetDecision`, `Difficulty`, `TaskType`, `ModelRouter`, and `OrangeRuntimeConfig` are introduced before later tasks reference them.
