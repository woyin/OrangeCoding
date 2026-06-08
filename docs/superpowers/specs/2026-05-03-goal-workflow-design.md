# Goal Workflow Design

## Overview

为 OrangeCoding 构建一个自主迭代循环机制 `/goal`，类似 Codex 的 `/goal` 指令和 Ralph Loop 技术。混合模式：先规划，再自引用迭代执行，规划可动态调整。完成后移除现有的 Autopilot workflow。

## 设计决策

- **入口**：TUI 斜杠命令 + CLI 参数，两者都支持
- **模式**：混合式（Planning → 自引用 Executing → Verifying → 动态 Replan）
- **完成判定**：自动验证 + Promise 标签 + 迭代上限（三重保障）
- **Agent 架构**：新建轻量 Goal 编排器（GoalMode），复用现有 agents
- **持久化**：支持断点续跑，状态持久化到 `.sisyphus/goal.json`
- **Drift 检测**：集成 HarnessSupervisor，实时检测目标漂移

## 1. Goal Workflow 状态机

```
Planning → Executing → Verifying → (Replan? or Done)
                ^                         |
                |     drift/verify fail   |
                └─────────────────────────┘
```

**Phase 枚举：**

| Phase | 职责 |
|-------|------|
| `Planning` | 分析目标，生成/更新 GoalPlan（任务拆解 + 验收标准） |
| `Executing` | 执行当前任务，agent 自引用之前的文件修改决定下一步 |
| `Verifying` | 运行验证命令（测试、编译），对比验收标准 |
| `Done` | 所有任务完成且验证通过 |

**关键结构：**

```rust
pub struct GoalMode {
    is_active: bool,
    phase: GoalPhase,
    config: GoalConfig,
    plan: Option<GoalPlan>,
    current_cycle: u32,
    requirement: String,
    mission_contract: Option<MissionContract>,
    last_verification: Option<VerificationReport>,
}

pub struct GoalConfig {
    max_cycles: u32,              // 默认 20
    task_max_iterations: u32,     // 默认 30
    verify_commands: Vec<String>, // e.g. ["cargo test", "cargo clippy"]
    auto_commit_per_cycle: bool,  // 默认 true
    completion_promise: String,   // 用户可自定义
    enable_drift_detection: bool, // 默认 true
}
```

**循环逻辑：**

1. `Planning` — 内联规划，生成 `GoalPlan`
2. `Executing` — 复用现有 agent loop，通过 `InstructionAnchor` 注入当前任务目标
3. `Verifying` — 运行 `verify_commands`，比对 `acceptance_criteria`
4. 验证失败或 drift 检测触发 → 回到 `Planning`（带已有上下文，动态调整计划）
5. 全部通过 → 输出 `<promise>{completion_promise}</promise>`，进入 `Done`

**与 Autopilot 的关键区别：**
- 自引用式执行（agent 看到自己的历史修改决定下一步）
- 集成 `HarnessSupervisor` 做实时 drift 检测
- `MissionContract` 约束 forbidden detours，防止目标漂移
- 断点续跑（持久化到 `.sisyphus/goal.json`）

## 2. GoalPlan 与任务结构

```rust
pub struct GoalPlan {
    pub requirement: String,
    pub tasks: Vec<GoalTask>,
    pub acceptance_criteria: Vec<String>,
    pub cycle: u32,
    pub forbidden_detours: Vec<String>,
    pub context: String,  // 项目现状摘要，每次 replan 时更新
}

pub struct GoalTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub target_files: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub depends_on: Vec<String>,
    pub status: GoalTaskStatus,
}

pub enum GoalTaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed { reason: String },
    Skipped { reason: String },
}
```

**Planning 阶段输出 `GoalPlan`：**

1. 扫描项目结构
2. 拆解为 `GoalTask` 列表，每个任务有明确验收标准
3. 识别依赖关系，确定执行顺序
4. 标记 `forbidden_detours`（不允许触碰的文件/目录）

**Replan 行为：**
- 保留 `Completed` 状态的任务
- 将 `Failed` 任务拆解为更小的子任务
- 更新 `context` 为当前项目状态
- `cycle` 递增

**执行顺序：** 拓扑排序，按依赖关系执行。第一版串行，并行留给后续优化。

## 3. 持久化与断点续跑

**存储路径：** `.sisyphus/goal.json`

```rust
#[derive(Serialize, Deserialize)]
pub struct GoalState {
    pub id: String,                          // goal-{timestamp_hex}{random_hex}
    pub requirement: String,
    pub plan: GoalPlan,
    pub config: GoalConfig,
    pub mission_contract: Option<MissionContract>,
    pub current_phase: GoalPhase,
    pub current_task_index: usize,
    pub current_cycle: u32,
    pub session_ids: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub last_checkpoint: Option<String>,
    pub last_verification: Option<VerificationReport>,
}
```

**关键操作：**

| 操作 | 方法 | 说明 |
|------|------|------|
| 创建 | `GoalState::new(requirement, config)` | 初始化状态，写入磁盘 |
| 保存 | `save(&self)` | 每次阶段转换后自动保存 |
| 加载 | `GoalState::load()` | 从 `.sisyphus/goal.json` 恢复 |
| 恢复提示 | `resume_prompt(&self)` | 生成续跑 prompt，包含进度和上下文 |
| 清理 | `GoalState::clear()` | goal 完成或取消后删除文件 |

**断点续跑流程：**

1. CLI 启动或 `/goal` 命令检测到 `.sisyphus/goal.json` 存在
2. 加载 `GoalState`，调用 `resume_prompt()` 生成上下文
3. 从 `current_phase` 和 `current_task_index` 继续
4. 已完成的任务不会重新执行

**与 Boulder 的关系：** 复用 `BoulderManager` 的存储模式，但 `GoalState` 是独立结构。将来可通过 `HarnessSnapshot` 桥接。

## 4. 入口 — Slash Commands 与 CLI 参数

### Slash Commands

| 命令 | 说明 |
|------|------|
| `/goal <requirement>` | 启动目标循环 |
| `/goal --resume` | 恢复上次未完成的目标 |
| `/goal --status` | 查看当前目标进度 |
| `/goal-stop` | 停止当前目标循环 |

**`/goal <requirement>` 参数解析：**

```
/goal 修复 auth 模块的 token 刷新逻辑
/goal "重构缓存层，使用 Redis 替代内存缓存" --max-cycles 15 --promise REFINISHED
/goal 添加用户注册功能 --verify "cargo test" --verify "cargo clippy"
```

支持 flags：
- `--max-cycles <n>` — 最大循环次数（默认 20）
- `--promise <text>` — 完成信号（默认 `GOAL_COMPLETE`）
- `--verify <cmd>` — 验证命令，可多次使用（默认 `["cargo test"]`）
- `--no-auto-commit` — 禁用每轮自动提交
- `--no-drift-detect` — 禁用 drift 检测
- `--resume` — 恢复上次目标

### CLI 参数

```bash
orangecoding --goal "修复 auth 模块的 token 刷新逻辑"
orangecoding --goal "重构缓存层" --max-cycles 15 --promise REFINISHED
orangecoding --goal --resume
```

在 `clap` 的 `AppArgs` 中新增 `--goal` 参数组。检测到 `--goal` 时走 headless 模式，直接进入 agent loop 执行 goal workflow。

### ExecutionMode 扩展

```rust
pub enum ExecutionMode {
    Exec,
    Plan,
    Goal,      // 新增，替代 Autopilot
    UltraWork,
}
```

`Goal` 模式 system prompt 核心：不等待用户确认，以目标完成为唯一停止条件，每步决策时回顾 `MissionContract`。

## 5. 验证与完成机制

### 验证流程（Verifying 阶段）

每轮执行后自动进入验证：

1. **运行 `verify_commands`** — 依次执行，收集退出码和输出
2. **比对 `acceptance_criteria`** — 让 AI 判断每个 criteria 是否满足
3. **生成 `VerificationReport`**

```rust
pub struct VerificationReport {
    pub cycle: u32,
    pub command_results: Vec<CommandResult>,
    pub criteria_results: Vec<CriteriaResult>,
    pub passed: bool,
    pub failure_summary: Option<String>,
    pub suggestions: Vec<String>,
}

pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub passed: bool,
}

pub struct CriteriaResult {
    pub criterion: String,
    pub satisfied: bool,
    pub evidence: String,
}
```

### 完成判定（三重保障）

| 条件 | 类型 | 说明 |
|------|------|------|
| 所有 criteria 满足 + 所有 verify commands 通过 | 自动验证 | 核心判定 |
| Agent 输出 `<promise>{promise}</promise>` | Promise 信号 | Agent 主动声明完成 |
| `current_cycle >= max_cycles` | 保底止损 | 强制停止，输出当前进度 |

**完成优先级：** 自动验证通过 > Promise 信号 > 迭代上限。

- 自动验证通过 → 直接 Done
- Agent 输出 promise 但验证未通过 → 回到 Executing 再验证
- 到达迭代上限 → 停止，输出未完成任务列表

### Drift 检测集成

每次 `Executing` 阶段任务完成后，调用 `HarnessSupervisor::evaluate_checkpoint()`：

- `Continue` → 推进到下一个任务
- `Replan` → 中断执行，回到 `Planning`，带 drift 原因
- `Escalate` → 暂停，通知用户（CLI 模式下记录日志并停止）

## 6. 移除 Autopilot

Goal 是 Autopilot 的全面升级，移除范围：

### 删除的文件/代码

| 位置 | 操作 |
|------|------|
| `workflows/autopilot.rs` | 删除 |
| `workflows/mod.rs` 中 `mod autopilot` | 移除 |
| `slash_builtins.rs` 中 autopilot 三个 builtin | 移除 |
| `slash.rs` 中对应的三条 register | 移除 |
| `ExecutionMode::Autopilot` variant | 替换为 `ExecutionMode::Goal` |
| 所有引用 `AutopilotMode` 的代码 | 替换为 `GoalMode` |

### 迁移映射

| Autopilot 概念 | Goal 对应 |
|----------------|-----------|
| `AutopilotConfig` | `GoalConfig`（新增 drift、promise 字段） |
| `AutopilotPlan` | `GoalPlan`（新增 forbidden_detours、context） |
| `AutopilotTask` | `GoalTask`（新增 Failed/Skipped 原因） |
| `AutopilotPhase::Analyzing` | 合并到 `GoalPhase::Planning` |
| `AutopilotPhase::Failed` | 移除（失败时 replan 而非终止） |
| `/autopilot <req>` | `/goal <req>` |
| `/autopilot-stop` | `/goal-stop` |
| `/autopilot-status` | `/goal --status` |
| `.sisyphus/autopilot.json` | `.sisyphus/goal.json` |

### 顺序

1. 先实现 `GoalMode`（确保功能完整）
2. 再移除 Autopilot 相关代码
3. 最后清理所有编译错误和未使用的 import
