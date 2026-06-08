# Autopilot 模式设计方案

## 1. 概述

Autopilot 是 OrangeCoding 的长任务全自动执行模式。用户输入一个大需求（数百行 prompt），系统在获得所有权限的前提下，通过 **计划 → 执行 → 验证 → 再计划** 的循环，把所有工作做完。

灵感来源：GitHub Copilot 的 Agent Mode、Cursor 的 Auto mode。

## 2. 核心循环

```
┌─────────────────────────────────────────────────────────┐
│                    Autopilot 循环                        │
│                                                         │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐          │
│  │  Plan    │───▶│ Execute  │───▶│ Verify   │          │
│  │ 制定计划  │    │ 执行任务  │    │ 验证结果  │          │
│  └──────────┘    └──────────┘    └────┬─────┘          │
│       ▲                               │                 │
│       │         验证失败               │                 │
│       └───────────────────────────────┘                 │
│       │         验证通过               │                 │
│       ▼                               ▼                 │
│  ┌──────────┐                   ┌──────────┐           │
│  │ 完成/下一  │                   │  Done    │           │
│  │ 轮改进计划 │                   │ 全部完成  │           │
│  └──────────┘                   └──────────┘           │
└─────────────────────────────────────────────────────────┘
```

每轮循环 = 1 次 Plan + 1 次 Execute + 1 次 Verify。默认最多 10 轮。

## 3. 触发方式

### 3.1 CLI 参数

```bash
# 直接指定需求
ceair launch --autopilot "重构整个认证系统，从 JWT 迁移到 OAuth 2.1..."

# 从文件读取需求
ceair launch --autopilot-file requirements.md

# 配置参数
ceair launch --autopilot "需求" --max-cycles 5 --verify-strict
```

### 3.2 斜杠命令

```
/autopilot <需求描述>       # 进入 autopilot 模式
/autopilot-stop            # 手动停止 autopilot
/autopilot-status          # 查看当前循环状态
```

## 4. 阶段详细设计

### 4.1 Plan（计划阶段）

**输入**：用户原始需求 + 上一轮的验证反馈（若有）

**处理**：
1. 分析需求，拆解为有序任务列表
2. 每个任务标注：描述、涉及文件、预期产出、验收标准
3. 识别任务间依赖关系
4. 如果是重计划：对比上一轮计划，标注变更部分

**输出**：`AutopilotPlan` 结构体

```rust
pub struct AutopilotPlan {
    /// 原始用户需求
    pub requirement: String,
    /// 任务列表
    pub tasks: Vec<AutopilotTask>,
    /// 总体验收标准（由 AI 从需求中提取）
    pub acceptance_criteria: Vec<String>,
    /// 当前是第几轮计划
    pub cycle: u32,
    /// 上一轮的验证结果摘要（首轮为 None）
    pub previous_verification: Option<VerificationReport>,
}

pub struct AutopilotTask {
    pub id: String,
    pub description: String,
    pub target_files: Vec<PathBuf>,
    pub expected_outcome: String,
    pub acceptance_criteria: Vec<String>,
    pub depends_on: Vec<String>,
    pub status: TaskStatus,
}
```

### 4.2 Execute（执行阶段）

**输入**：`AutopilotPlan` 中的待执行任务

**处理**：
1. 按依赖顺序执行任务
2. 每个任务内部使用 AgentLoop（auto_approve_tools = true）
3. 执行完毕后更新任务状态
4. 收集执行产物（修改的文件、命令输出、错误信息）

**关键配置**：
- `auto_approve_tools: true` — 全部权限
- `max_iterations` — 每个任务内部的 AI 循环上限（默认 30）
- `timeout` — 每个任务超时（默认 10 分钟）

### 4.3 Verify（验证阶段）

**输入**：执行结果 + 计划中的验收标准

**处理**（两条路径并行）：

**路径 A — 自动化测试**：
1. 运行 `cargo test --workspace`（若为 Rust 项目）
2. 运行 `cargo check --workspace`
3. 运行 AI 生成的专项测试命令
4. 收集测试通过率、失败详情

**路径 B — AI 自评**：
1. 将原始需求 + 执行结果 + 测试结果发给 AI
2. AI 逐条评估验收标准是否满足
3. AI 输出验证报告

**输出**：`VerificationReport`

```rust
pub struct VerificationReport {
    /// 本轮执行的循环编号
    pub cycle: u32,
    /// 测试运行结果
    pub test_results: TestRunResult,
    /// AI 对每条验收标准的评估
    pub criteria_results: Vec<CriterionResult>,
    /// 总体是否通过
    pub passed: bool,
    /// 失败原因汇总（供下一轮计划参考）
    pub failure_summary: Option<String>,
    /// AI 建议的下一步
    pub suggestions: Vec<String>,
}

pub struct TestRunResult {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub failed_details: Vec<String>,
    pub build_passed: bool,
}

pub struct CriterionResult {
    pub criterion: String,
    pub passed: bool,
    pub evidence: String,
}
```

## 5. 状态机

```rust
pub enum AutopilotPhase {
    /// 初始阶段：分析需求，生成第一轮计划
    Analyzing,
    /// 计划阶段：拆解任务，设定验收标准
    Planning,
    /// 执行阶段：按计划执行任务
    Executing,
    /// 验证阶段：运行测试 + AI 评估
    Verifying,
    /// 重计划阶段：基于验证失败生成改进计划
    Replanning,
    /// 全部完成
    Done,
    /// 失败终止（超过最大循环次数或不可恢复错误）
    Failed(String),
}
```

状态转换规则：
- `Analyzing` → `Planning`（需求分析完成）
- `Planning` → `Executing`（计划生成成功）
- `Executing` → `Verifying`（所有任务执行完毕）
- `Verifying` → `Done`（验证全部通过）
- `Verifying` → `Replanning`（验证存在失败）
- `Replanning` → `Executing`（新计划生成成功）
- 任何阶段 → `Failed`（不可恢复错误或超过 max_cycles）

## 6. 配置

### 6.1 AutopilotConfig

```rust
pub struct AutopilotConfig {
    /// 最大循环轮次（默认 10）
    pub max_cycles: u32,
    /// 是否严格验证模式（默认 false）
    /// true: 所有验收标准必须通过 + 测试全部通过
    /// false: AI 判定核心目标达成即可
    pub verify_strict: bool,
    /// 每个任务内部的 AI 最大迭代次数（默认 30）
    pub task_max_iterations: u32,
    /// 每个任务超时秒数（默认 600）
    pub task_timeout_secs: u64,
    /// 是否在每轮结束时暂停等待用户确认（默认 false）
    pub pause_between_cycles: bool,
    /// 是否自动运行测试（默认 true）
    pub auto_run_tests: bool,
    /// 自定义验证命令（覆盖默认的 cargo test）
    pub verify_commands: Vec<String>,
}
```

### 6.2 配置文件支持

在 `config.toml` 中：

```toml
[autopilot]
max_cycles = 10
verify_strict = false
task_max_iterations = 30
task_timeout_secs = 600
pause_between_cycles = false
auto_run_tests = true
verify_commands = []
```

### 6.3 CLI 参数映射

```bash
--autopilot <REQ>          # 需求描述
--autopilot-file <PATH>    # 需求文件
--max-cycles <N>           # 覆盖 max_cycles
--verify-strict            # 启用严格验证
--pause-between-cycles     # 每轮暂停
--no-auto-test             # 禁用自动测试
```

## 7. 文件结构

```
crates/orangecoding-agent/src/
├── workflows/
│   ├── mod.rs                  # 新增 pub mod autopilot
│   ├── autopilot.rs            # 新增：Autopilot 状态机 + 配置
│   ├── autopilot_plan.rs       # 新增：计划生成逻辑
│   ├── autopilot_verify.rs     # 新增：验证逻辑（测试运行 + AI 评估）
│   ├── ultrawork.rs            # 已有：复用阶段管理
│   ├── prometheus.rs           # 已有：复用计划结构
│   ├── atlas.rs                # 已有：复用执行编排
│   └── boulder.rs              # 已有：复用状态持久化

crates/orangecoding-cli/src/
├── commands/
│   └── launch.rs               # 修改：添加 --autopilot 参数处理
├── slash_builtins.rs           # 修改：添加 /autopilot 命令
```

## 8. 事件与日志

Autopilot 通过现有的 AgentEvent 系统上报状态：

```rust
// 新增事件变体（扩展 AgentEvent）
AgentEvent::AutopilotPhaseChanged {
    cycle: u32,
    phase: AutopilotPhase,
    summary: String,
}

AgentEvent::AutopilotTaskCompleted {
    cycle: u32,
    task_id: String,
    success: bool,
    duration_secs: u64,
}

AgentEvent::AutopilotCycleComplete {
    cycle: u32,
    verification_passed: bool,
    total_tasks: u32,
    completed_tasks: u32,
}
```

TUI / WebSocket 端接收这些事件后可实时显示进度。

## 9. 安全与边界

### 9.1 权限

Autopilot 模式要求 `auto_approve_tools = true`。启动时明确告知用户：
- 所有工具调用自动批准
- 文件读写、命令执行无需确认
- 建议在版本控制的项目中运行（可随时 git revert）

### 9.2 资源限制

- `max_cycles` 防止无限循环
- `task_timeout_secs` 防止单任务卡死
- 每轮最多消耗 token 预算提示（配置中可设预算上限）
- 用户可随时 Ctrl+C 终止

### 9.3 Git 安全网

每轮 Execute 完成后可选自动 git commit：
```toml
[autopilot]
auto_commit_per_cycle = true
commit_message_template = "autopilot: cycle {cycle} - {summary}"
```

## 10. 实现计划

### Phase 1：核心循环（预估 4 个文件）

1. `autopilot.rs` — AutopilotConfig, AutopilotPhase, AutopilotMode 状态机
2. `autopilot_plan.rs` — AutopilotPlan, AutopilotTask, 计划生成 prompt 模板
3. `autopilot_verify.rs` — VerificationReport, TestRunResult, 验证逻辑
4. `workflows/mod.rs` — 导出新模块

### Phase 2：CLI 集成（预估 2 个文件）

5. `launch.rs` — 添加 --autopilot 参数解析，autopilot 运行入口
6. `slash_builtins.rs` — 添加 /autopilot, /autopilot-stop, /autopilot-status

### Phase 3：配置与事件（预估 2 个文件）

7. `config.rs` (ceair-config) — 添加 AutopilotConfig 到 OrangeConfig
8. `event.rs` (ceair-core) — 添加 Autopilot 相关事件变体

### 验收标准

- [ ] `ceair launch --autopilot "创建一个 hello world 函数"` 能完成三轮以内循环
- [ ] 验证阶段自动运行 cargo test 并收集结果
- [ ] 验证失败后自动重计划并继续
- [ ] 超过 max_cycles 后优雅停止并输出进度报告
- [ ] /autopilot 斜杠命令可在交互模式中触发
- [ ] 全量测试通过
