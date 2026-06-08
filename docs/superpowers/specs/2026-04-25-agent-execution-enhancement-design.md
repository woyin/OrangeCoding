# OrangeCoding Agent 执行增强设计文档

**日期**: 2026-04-25  
**状态**: 待实现  
**范围**: 三模式执行增强 + 模型动态路由

---

## 问题陈述

当前 OrangeCoding agent 在长任务执行中存在三类核心痛点：

1. **指令漂移** — agent 执行多步任务时逐渐偏离原始指令，自作主张
2. **中途放弃** — 遇到不确定性时提前停下来问用户，而非继续推进
3. **步数限制** — 固定步数上限导致复杂任务无法完成

此外，现有模式体系（Normal / Plan / Autopilot / UltraWork）缺乏清晰的行为规范和模式间的流转机制，模型选择也是静态的，无法按任务难度动态调整。

---

## 解决方案概述

采用**双轨增强**方案：

- **Prompt 层**：重写 system prompt，嵌入任务宪法（指令锚定、执行规则、自检模板）
- **代码层**：实现三个轻量 Rust 模块强制执行机制

同时增强三个模式的行为规范，并实现基于配置文件的模型动态路由。

---

## 模式体系设计

### 模式对照表

| 模式 | 概念名称 | 行为特征 |
|------|---------|---------|
| Normal | Exec 模式 | 严格执行，只在真实决策分叉点暂停询问用户 |
| Plan | Plan 模式 | 结构化规划，规划完成后询问执行策略 |
| Autopilot | 长任务模式 | 全程自动，静默自纠，不打扰用户 |
| UltraWork | （暂不修改） | 保持现状 |

### Normal（Exec）模式增强

**行为规范**：
- 收到指令后立即执行，不做多余规划
- 严格按照指令字面意思行动，不添加未要求的内容
- **决策分叉点定义**：出现两种以上合理路径，且选择会产生不可逆影响时，才暂停询问
- 非决策分叉点（如轻微歧义、实现细节选择）自行判断后继续

**System Prompt 增强语言**：
```
[EXEC MODE - 严格执行]
你处于 Exec 模式。规则：
1. 收到指令立即执行，不做多余规划或解释
2. 严格遵循指令字面意思
3. 只有在出现真正的决策分叉（多路径且不可逆）时才暂停询问
4. 其他一切情况：自行判断，继续执行
```

### Plan 模式增强

**行为规范**：
1. 收到任务后，先输出结构化计划（分阶段，每阶段有明确目标和验收标准）
2. 向用户展示计划，等待用户回复「ok」「确认」或任何肯定性回复
3. 计划确认后，**询问执行策略**：

> 计划已确认。接下来如何执行？
> - **一步到位**：切换到 Autopilot 模式，全程自动执行，不打扰你
> - **Exec 模式**：严格执行计划，遇到决策分叉时问你

4. 根据用户选择切换到对应模式

**计划输出格式**：
```
## 执行计划

**目标**：[一句话描述]

### 阶段 1：[名称]
- 具体步骤 1
- 具体步骤 2
**验收**：[可验证的完成标准]

### 阶段 2：...

**预估复杂度**：easy / medium / hard / epic
```

### Autopilot（长任务）模式增强

**行为规范**：
- 任务开始时锁定原始指令快照
- 每 N 步（默认 5）静默执行一次指令回锚检查
- 步数超出预算时，评估是否继续（默认继续），只在检测到死循环时中断
- 全程不主动打扰用户
- 根据 `orange.json` 路由选择最适合的模型

---

## Prompt 层设计：任务宪法

重写 OrangeCoding 的 system prompt，核心结构：

```
[MISSION LOCK]
你是 OrangeCoding，一个不间断执行的编码 agent。
原始用户指令是你的最高法律，任何子任务都不得覆盖它。

[EXECUTION RULES - 适用于 Autopilot 模式]
1. 永不在任务中途停止等待用户确认
2. 每完成 5 个步骤，静默执行一次「指令回锚」——
   将当前行为与原始指令对比，发现漂移立即纠正，不向用户汇报
3. 遇到障碍时，先尝试 3 种替代方案，全部失败后才可停止
4. 步数不是硬限制，任务完成才是终止条件

[SELF-CHECK TEMPLATE - 每 5 步内部执行一次]
① 原始指令要求我做什么？
② 我现在正在做什么？
③ 是否偏离？→ 是：立即纠正 / 否：继续

[TASK DIFFICULTY SIGNAL]
用户可在指令中注明 difficulty: easy / medium / hard / epic
未注明时，根据任务特征自行判断后选择模型

[MODE-SPECIFIC RULES]
- Normal/Exec：严格执行，决策分叉点才暂停
- Plan：先规划，确认后询问执行策略
- Autopilot：全程静默，自纠漂移，不打扰
```

---

## 代码层设计：三个 Rust 模块

### 1. `InstructionAnchor`

**文件**：`crates/orangecoding-agent/src/instruction_anchor.rs`

**职责**：存储原始用户指令快照，每 N 步向上下文注入回锚消息。

**接口**：
```rust
pub struct InstructionAnchor {
    original_instruction: String,
    anchor_interval: u32,       // 从 orange.json 读取，默认 5
    step_counter: u32,
}

impl InstructionAnchor {
    pub fn new(instruction: &str, interval: u32) -> Self;
    pub fn on_step(&mut self) -> Option<String>; // 返回 Some(回锚消息) 或 None
    pub fn reset(&mut self, new_instruction: &str);
}
```

**集成点**：挂在 agent loop 的 `AgentEvent` 流上，作为中间件。

### 2. `StepBudgetGuard`

**文件**：`crates/orangecoding-agent/src/step_budget.rs`

**职责**：步数预算管理，超限时评估是否继续，检测死循环。

**接口**：
```rust
pub struct StepBudgetGuard {
    budget: u32,                        // 从 orange.json 读取，默认 100
    current: u32,
    loop_threshold: u32,                // 默认 3
    recent_actions: VecDeque<String>,   // action_signature = tool名称+主要参数的哈希，用于死循环检测
}

pub enum BudgetDecision {
    Continue,
    HardStop { reason: String },        // 仅死循环时触发
}

impl StepBudgetGuard {
    pub fn new(budget: u32, loop_threshold: u32) -> Self;
    pub fn tick(&mut self, action_signature: &str) -> BudgetDecision;
}
```

**逻辑**：
- `current < budget`：返回 `Continue`
- `current >= budget`：扩展预算（+50%），返回 `Continue`
- 相同 `action_signature` 连续出现 `loop_threshold` 次：返回 `HardStop`

### 3. `ModelRouter`

**文件**：`crates/orangecoding-agent/src/model_router.rs`

**职责**：读取 `orange.json`，按复杂度 × 类型路由模型。

**接口**：
```rust
pub struct ModelRouter {
    rules: Vec<RoutingRule>,
    fallback: String,
}

pub struct RoutingRule {
    pub difficulty: Option<Difficulty>,  // None 表示通配
    pub task_type: Option<TaskType>,     // None 表示通配
    pub model: String,
}

pub enum Difficulty { Easy, Medium, Hard, Epic }
pub enum TaskType { Code, Write, Analyze, Chat }

impl ModelRouter {
    pub fn load(config_path: &Path) -> Result<Self>;        // 失败时使用默认值
    pub fn route(&self, difficulty: Difficulty, task_type: TaskType) -> &str;
}
```

**匹配优先级**：精确匹配（difficulty + type 均指定）> 单维通配 > 全通配 > fallback。

---

## 配置文件设计：`orange.json`

**路径**：`~/.config/orangecoding/orange.json`

```json
{
  "model_routing": {
    "rules": [
      { "difficulty": "easy",   "task_type": "chat",    "model": "deepseek-chat" },
      { "difficulty": "easy",   "task_type": "code",    "model": "deepseek-chat" },
      { "difficulty": "medium", "task_type": "code",    "model": "deepseek-coder" },
      { "difficulty": "medium", "task_type": "analyze", "model": "deepseek-coder" },
      { "difficulty": "hard",   "task_type": "code",    "model": "claude-sonnet-4-5" },
      { "difficulty": "hard",   "task_type": "*",       "model": "claude-sonnet-4-5" },
      { "difficulty": "epic",   "task_type": "*",       "model": "claude-opus-4-7" }
    ],
    "fallback_model": "deepseek-chat"
  },
  "execution": {
    "anchor_interval_steps": 5,
    "step_budget_initial": 100,
    "loop_detection_threshold": 3
  }
}
```

**规则**：
- 文件缺失时静默使用默认值，不报错
- `task_type: "*"` 表示通配所有类型
- `fallback_model` 在查表失败时使用

---

## 给外部 AI 的长 Prompt 结构

最终交付物是一份给外部 AI 助手的完整开发指令，包含以下五节：

1. **项目背景**：OrangeCoding 架构（15 crates、依赖流、关键路径）
2. **问题陈述**：三大痛点 + 三模式现状
3. **交付要求**：
   - 重写后的 system prompt 文本（可直接替换）
   - `instruction_anchor.rs`（完整实现）
   - `step_budget.rs`（完整实现）
   - `model_router.rs`（完整实现）
   - `orange.json` 示例
   - 对应的单元测试（中文函数名）
4. **代码约束**：OrangeError 用法、中文注释、`partial_cmp().unwrap_or`、axum `:id` 语法
5. **成功标准**：`cargo check --workspace` 通过，核心路径有测试覆盖

---

## 成功标准

- [ ] `cargo check --workspace` 通过（无新增编译错误）
- [ ] `cargo test -p orangecoding-agent` 通过
- [ ] `InstructionAnchor` 有单元测试：验证回锚触发时机
- [ ] `StepBudgetGuard` 有单元测试：验证预算扩展和死循环检测
- [ ] `ModelRouter` 有单元测试：验证精确匹配和通配回退
- [ ] Plan 模式完成规划后，正确展示执行策略选择
- [ ] `orange.json` 缺失时 agent 正常启动，使用默认值
