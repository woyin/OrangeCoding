# Agent OS 架构设计

> OrangeCoding Agent OS 是一个 AI 驱动的软件工程系统，具备**学习、验证、防护、修复、进化**五大核心能力。它不仅是一个代码生成工具，更是一个拥有完整生命周期管理的智能操作系统。

## 1. 概述

Agent OS 由 15 个 Rust crate 组成，构建了从底层运行时到高层进化引擎的完整架构。系统围绕**不变量驱动**（Invariant-Driven）的核心理念设计：

- **18 条系统不变量**定义了系统必须遵守的安全与正确性约束
- **运行时防护**在操作执行前拦截违规行为
- **自动回滚**在违规发生后恢复系统状态
- **自我修复**检测问题并生成修复方案
- **自我进化**从历史模式中学习并优化策略

```text
┌─────────────────────────────────────────────────────────────┐
│                    Control Plane（控制面）                    │
│              Browser ←→ Server ←→ Worker                     │
├─────────────────────────────────────────────────────────────┤
│                   Agent Layer（代理层）                       │
│         Planner → Executor → Verifier → Healer               │
├─────────────────────────────────────────────────────────────┤
│                 Orchestration（编排层）                       │
│        TaskOrchestrator · MessageBus · ModelRouter            │
├─────────────────────────────────────────────────────────────┤
│              Evolution Engine（进化引擎）                     │
│       Pattern Learning → Strategy Generation → Snapshot       │
├─────────────────────────────────────────────────────────────┤
│              Verification Layer（验证层）                     │
│     Design · Invariant · Bypass · Security · Regression       │
├─────────────────────────────────────────────────────────────┤
│                Guard System（防护系统）                       │
│         PreCheckGate · RuntimeGuard · AutoRollback            │
├─────────────────────────────────────────────────────────────┤
│             Invariant Engine（不变量引擎）                    │
│          Rules (18) · Checker · ViolationReport               │
├─────────────────────────────────────────────────────────────┤
│              Runtime Kernel（运行时内核）                     │
│     Tools · Session · Audit · MCP · Context · EventBus       │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 架构层次

### 2.1 Runtime Kernel（运行时内核）

运行时内核提供 Agent OS 的基础设施，包括工具执行、会话持久化、审计追踪和模型通信。

#### 2.1.1 核心类型 — `orangecoding-core`

所有 crate 共享的基础类型定义：

| 类型 | 说明 |
|------|------|
| `AgentId(Uuid)` | 代理唯一标识 |
| `SessionId(Uuid)` | 会话唯一标识 |
| `AgentEvent` | 核心事件枚举（8 个变体） |
| `EventBus` / `EventHandler` | 事件发布/订阅接口 |
| `Message` / `Conversation` | 会话消息模型 |
| `ToolCall` / `ToolResult` | 工具调用请求与结果 |

`AgentEvent` 的 8 个变体覆盖了代理生命周期的所有阶段：

```text
Started → MessageReceived → ToolCallRequested → ToolCallCompleted
                                                       ↓
StreamChunk ← TokenUsageUpdated ← Completed / Error
```

#### 2.1.2 工具系统 — `orangecoding-tools`

工具是 Agent 与外部世界交互的唯一接口。每个工具必须实现 `Tool` trait：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn metadata(&self) -> ToolMetadata;
    async fn validate_input(&self, args: &Value) -> ToolResult<()>;
    async fn check_permissions(&self, context: &Context) -> ToolResult<()>;
    async fn execute(&self, args: Value) -> ToolResult<String>;
}
```

工具执行遵循严格的三阶段流水线：

```text
validate_input() → check_permissions() → execute()
     ↓ Err              ↓ Deny              ↓
   拒绝执行            拒绝执行          返回结果
```

**24+ 内置工具**，分为 6 个类别：

| 类别 | 工具 |
|------|------|
| 文件操作 | `ReadFile`, `WriteFile`, `EditFile`, `ListDirectory`, `SearchFiles`, `DeleteFile` |
| 代码分析 | `Ast`, `Grep`, `Find`, `Lsp` |
| 执行环境 | `Bash`, `Python`, `Ssh` |
| 网络访问 | `Fetch`, `Browser`, `WebSearch` |
| 会话管理 | `SessionList`, `SessionRead`, `SessionSearch`, `SessionInfo` |
| 任务管理 | `TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate` |

安全子系统包括：
- **`PermissionChecker`** — 权限验证（Read/Write/Execute/Delete/Network）
- **`PathValidator`** — 路径安全校验（防止目录穿越）
- **`FileOperationGuard`** — 文件操作防护
- **`PostToolPipeline`** — 执行后钩子链

#### 2.1.3 会话管理 — `orangecoding-session`

会话使用 **MessagePack 二进制格式**持久化，支持 Git 风格的分支与历史导航。

| 组件 | 说明 |
|------|------|
| `SessionManager` | 会话生命周期管理（创建/恢复/列表/分支） |
| `SessionStorage` | MessagePack 持久化（Header + Entries + Blobs） |
| `SessionTree` | 分支与历史导航 |
| `BlobStore` | 大对象存储 |

`SessionEntry` 支持 8 种条目类型：

```text
Message · ToolCall · ThinkingLevel · ModelChange
Compaction · BranchSummary · Label · ModeChange
```

#### 2.1.4 审计系统 — `orangecoding-audit`

审计系统提供**不可篡改的操作记录链**：

```text
AuditEntry {
    id: UUID,
    timestamp: DateTime<Utc>,
    action: String,          // "tool_call", "ai_request", ...
    actor: String,           // 操作者
    target: Option<String>,  // 操作目标
    details: JSON,           // 完整上下文
    hash: String,            // SHA-256 当前哈希
    previous_hash: String    // 前一条记录哈希
}
```

- **`HashChain`** — SHA-256 哈希链（genesis hash = 64 个零）
- **`Sanitizer`** — 敏感数据脱敏（token、密码等）
- **`AuditLogger`** — 异步批量写入 + JSON Lines 格式 + 自动文件轮转

#### 2.1.5 MCP 协议 — `orangecoding-mcp`

实现 [Model Context Protocol](https://modelcontextprotocol.io/) 标准，支持与外部 MCP 服务器交互：

| 组件 | 说明 |
|------|------|
| `McpServer` | MCP 服务端（工具注册、初始化、工具调用） |
| `McpClient` | MCP 客户端（工具发现、工具调用） |
| `StdioTransport` | Stdio 传输层 |
| `MemoryTransport` | 内存传输（测试用） |

协议流程：

```text
Client → initialize → Server
Client ← capabilities ← Server
Client → notifications/initialized
Client ↔ tools/list, tools/call ↔ Server
Client → shutdown
```

---

### 2.2 Invariant Engine（不变量引擎）

> 路径：`crates/orangecoding-invariant/src/rules.rs`, `checker.rs`, `report.rs`

不变量引擎是 Agent OS 的核心保障机制。它定义了系统必须遵守的约束，并在运行时验证这些约束是否被满足。

#### 2.2.1 规则定义 — `rules.rs`

18 条系统不变量，覆盖 8 个类别：

| 类别 | 规则数 | 严重级别分布 |
|------|--------|-------------|
| Auth（认证） | 3 | Critical ×3 |
| Cancellation（取消） | 2 | High ×2 |
| Session（会话） | 3 | High ×2, Medium ×1 |
| ToolPermission（工具权限） | 3 | Critical ×2, High ×1 |
| Context（上下文） | 2 | High ×1, Medium ×1 |
| Audit（审计） | 2 | High ×1, Medium ×1 |
| Approval（审批） | 2 | High ×2 |
| Event（事件） | 1 | Medium ×1 |

**Critical 级别规则**（6 条，违反即阻断）：

| ID | 规则名称 | 描述 |
|----|---------|------|
| INV-AUTH-01 | WebSocket 连接必须鉴权 | 每个 WS 升级请求必须在握手阶段验证 token |
| INV-AUTH-02 | HTTP API 必须通过认证中间件 | 除 /health 外所有端点必须要求 Bearer token |
| INV-AUTH-03 | Token 不得出现在日志中 | 认证 token 不得以明文出现在日志输出中 |
| INV-TOOL-01 | 高危工具执行前必须权限检查 | destructive 工具必须先 check_permissions |
| INV-TOOL-02 | Deny 决策必须阻止执行 | check_permissions 返回 Deny 时 execute 不得被调用 |

**High 级别规则**（8 条，触发警告并可能阻断）：

| ID | 规则名称 |
|----|---------|
| INV-CANCEL-01 | 取消信号必须向下传播 |
| INV-CANCEL-02 | 取消后必须可重置 |
| INV-SESSION-01 | 会话上下文必须跨 turn 持久化 |
| INV-SESSION-02 | 关闭的会话不可继续使用 |
| INV-TOOL-03 | 输入验证必须在执行前完成 |
| INV-CTX-01 | 压缩后系统提示不得丢失 |
| INV-AUDIT-01 | 高危操作必须有审计记录 |
| INV-APPROVAL-01/02 | 审批请求必须可等待 / 审批结果必须送达请求方 |

**Medium 级别规则**（4 条）：

| ID | 规则名称 |
|----|---------|
| INV-SESSION-03 | 会话 ID 必须全局唯一 |
| INV-CTX-02 | Token 预算不得为负 |
| INV-AUDIT-02 | 审计链哈希必须连续 |
| INV-EVENT-01 | 事件序列必须保持时间顺序 |

#### 2.2.2 运行时检查器 — `checker.rs`

`InvariantChecker` 接受 `CheckContext` 进行运行时验证：

```rust
pub struct CheckContext {
    pub active_sessions: usize,
    pub auth_enabled: bool,
    pub pending_approvals: usize,
    pub events_monotonic: bool,
    pub tool_permission_enforced: bool,
    pub audit_chain_valid: bool,
    pub custom_checks: Vec<(String, bool, String)>,
}
```

检查流程：

```text
CheckContext → InvariantChecker.check() → ViolationReport
                     │
                     ├── 逐条评估 18 条规则
                     ├── 生成 Pass / Fail / Skip 结果
                     └── 汇总为 ViolationReport
```

#### 2.2.3 违规报告 — `report.rs`

`ViolationReport` 提供结构化的检查结果：

| 字段 | 说明 |
|------|------|
| `total_rules` | 总规则数 |
| `passed` / `failed` / `skipped` | 通过/失败/跳过数 |
| `violations` | 违规详情列表 |
| `has_critical` | 是否存在 Critical 级违规 |

支持 **Markdown 渲染**（`to_markdown()`），可直接输出为人类可读的报告。

---

### 2.3 Guard System（防护系统）

> 路径：`crates/orangecoding-invariant/src/gate.rs`, `runtime_guard.rs`, `rollback.rs`

防护系统在三个层面拦截违规行为：

```text
                ┌──────────────┐
                │ Git Diff     │
                └──────┬───────┘
                       ↓
              ┌────────────────┐
              │ PreCheckGate   │ ← 提交前分析
              │ Allow/Warn/Block│
              └────────┬───────┘
                       ↓
              ┌────────────────┐
              │ RuntimeGuard   │ ← 操作执行时拦截
              │ Allow/Deny/    │
              │ RequireApproval│
              └────────┬───────┘
                       ↓
              ┌────────────────┐
              │ AutoRollback   │ ← 违规后回滚
              │ git revert     │
              └────────────────┘
```

#### 2.3.1 预检门 — `gate.rs`

`PreCheckGate` 分析 Git diff，将文件变更映射到不变量类别，并做出决策：

```text
Git Diff → parse_changed_files() → map_file_to_categories()
                                          ↓
                              find_affected_rules()
                                          ↓
                              decide(max_severity)
                                          ↓
                              GateDecision:
                                Critical → Block
                                High     → Warn
                                其他     → Allow
```

`GateReport` 包含：
- `decision` — Allow / Warn / Block
- `impacts` — 每个文件的影响分析（affected_categories, lines_changed）
- `affected_rules` — 受影响的规则 ID 列表
- `max_severity` — 最高严重级别

#### 2.3.2 运行时守卫 — `runtime_guard.rs`

`RuntimeGuard` 在四个检查点拦截操作：

| 检查点 | 上下文 | 拦截规则 |
|--------|--------|---------|
| `check_ws_connection` | `WsConnectionContext` | 未认证连接 → Deny |
| `check_tool_call` | `ToolCallContext` | 未授权工具 / 高危工具 → Deny / RequireApproval |
| `check_session_op` | `SessionOpContext` | 已关闭会话 → Deny |
| `check_cancel_propagation` | `CancelContext` | 取消未传播 → Deny |

`GuardAction` 三种决策：

| 决策 | 含义 |
|------|------|
| `Allow` | 允许操作继续 |
| `Deny(reason)` | 拒绝操作并返回原因 |
| `RequireApproval(reason)` | 需要人工审批后继续 |

#### 2.3.3 自动回滚 — `rollback.rs`

`AutoRollback` 在以下情况自动执行 `git revert`：

| 触发条件 | 类型 |
|---------|------|
| 测试失败 | `TestFailure { test_name, output }` |
| 不变量违规 | `InvariantViolation { rule_id, message }` |
| 运行时违规 | `RuntimeViolation { guard_action, context }` |

回滚日志（`RollbackLog`）记录每次回滚的：
- 触发原因
- 回滚结果（Success / Failure / DryRun）
- 回滚前的 HEAD commit
- 时间戳

---

### 2.4 Verification Layer（验证层）

> 路径：`crates/orangecoding-invariant/src/verification.rs`

`VerificationAgent` 实现 **5 项检查流水线**，在每个 TODO 完成后执行：

```text
┌─────────────────────────────────────────────────────┐
│               VerificationAgent.verify()             │
│                                                      │
│  1. DesignConformance  — 设计一致性（diff 分析）      │
│  2. InvariantCompliance — 不变量合规（checker 验证）   │
│  3. BypassDetection    — 绕过检测（模式匹配）         │
│  4. SecurityCheck      — 安全检查（敏感数据检测）      │
│  5. RegressionCheck    — 回归测试（测试通过状态）      │
│                                                      │
│  → VerificationVerdict: Approved / NeedsWork / Rejected │
└─────────────────────────────────────────────────────┘
```

**绕过检测**扫描的模式：
- `#[allow(unused)]` — 忽略编译器警告
- `unsafe` — 不安全代码
- `.unwrap()` — 未处理的 panic 风险
- `#[ignore]` / `skip` — 跳过测试

**安全检测**扫描的模式：
- `password = "` / `token = "` / `secret = "` — 硬编码凭据
- `TODO: security` / `FIXME: security` — 未解决的安全问题

---

### 2.5 Orchestration（编排层）

> 路径：`crates/orangecoding-mesh/src/`, `crates/orangecoding-worker/src/`

#### 2.5.1 任务编排 — `orangecoding-mesh`

`TaskOrchestrator` 管理基于 DAG（有向无环图）的任务调度：

```text
Task A ──→ Task C ──→ Task E
Task B ──→ Task D ──↗
```

| 组件 | 说明 |
|------|------|
| `TaskOrchestrator` | DAG 任务调度（依赖追踪、状态管理） |
| `MessageBus` | 发布/订阅消息系统 |
| `AgentRegistry` | 代理注册与发现 |
| `RoleSystem` | 角色定义（系统提示、能力、权限） |
| `ModelRouter` | 基于任务类型的动态模型选择 |
| `SharedState` | 线程安全 KV 存储（带 TTL） |
| `NegotiationProtocol` | 任务协商（Request → Offer → Accept/Reject） |
| `HandoffManager` | 任务在代理间的交接 |

#### 2.5.2 Worker 运行时 — `orangecoding-worker`

`WorkerRuntime` 是核心运行时编排器：

```text
WorkerRuntime
├── SessionSupervisor   ← 会话生命周期管理
│   ├── create_session()
│   ├── get_session()
│   ├── cancel_task()
│   └── reset_cancel_token()
├── ApprovalBridge      ← 审批请求/响应配对
│   ├── request_approval() → (Request, Receiver)
│   └── resolve(id, decision)
├── EventBridge         ← AgentEvent → ServerEvent 转换
└── AgentExecutor       ← 代理执行接口
    └── execute_turn(session_id, message, tx, cancel)
```

**审批流程**：

```text
工具请求 → 风险评估 → ApprovalBridge.request_approval()
                              ↓
                     ApprovalRequest → WebSocket → 浏览器 UI
                              ↓
                     用户决策 → ApprovalDecision
                              ↓
                     ApprovalBridge.resolve() → oneshot channel → 工具继续/拒绝
```

---

### 2.6 Agent Layer（代理层）

> 路径：`crates/orangecoding-agent/src/`

#### 2.6.1 代理执行引擎

`AgentLoop` 是代理的核心事件循环：

```text
AgentLoop {
    config: AgentLoopConfig,       // max_iterations, timeout, auto_approve
    context: AgentContext,         // 会话上下文
    tool_executor: ToolExecutor,   // 工具调用协调
    cancellation: CancellationToken // 层级取消
}
```

执行流程：

```text
用户输入 → AgentLoop
              ↓
         AI 模型调用
              ↓
         解析工具调用请求
              ↓
    ┌─── validate_input ───┐
    │         ↓             │
    ├── check_permissions ──┤
    │         ↓             │
    └───── execute ─────────┘
              ↓
         结果反馈给 AI
              ↓
         循环直到完成或达到 max_iterations
```

#### 2.6.2 关键子系统

| 子系统 | 说明 |
|--------|------|
| `CancellationToken` | 层级取消传播（父 → 子，AtomicBool 无锁检查） |
| `Memory` / `MemoryRecall` | 长期记忆存储与检索 |
| `TaskSystem` | 任务依赖图管理 |
| `Compaction` | 会话压缩（减少 token 消耗） |
| `TokenBudget` | Token 预算管理 |
| `DreamExecutor` | 自主执行模式 |
| `IntentGate` | 意图分类与路由 |
| `SkillSystem` | 技能注册表 |

#### 2.6.3 代理角色

系统支持多种代理角色（`AgentRole`），通过 `RoleSystem` 定义各角色的系统提示、能力和权限：

| 角色 | 职责 |
|------|------|
| Planner | 任务规划与分解 |
| Coder | 代码实现 |
| Reviewer | 代码审查 |
| Verifier | 验证与测试 |
| Architect | 架构设计 |

---

### 2.7 Evolution Engine（进化引擎）

> 路径：`crates/orangecoding-invariant/src/healing.rs`, `evolution.rs`

#### 2.7.1 自我修复 — `healing.rs`

`SelfHealer` 实现 **detect → suggest → fix → verify** 生命周期：

```text
ViolationReport
    ↓
detect_from_report()     → HealingTask [Detected]
    ↓
generate_suggestion()    → FixSuggestion [Suggested]
    ↓
start_healing()          → [InProgress]
    ↓
mark_pending_verification() → [PendingVerification]
    ↓
mark_healed() / mark_failed() → [Healed] / [Failed]
```

**修复类型与优先级映射**：

| 违规类别 | 修复类型 | 优先级 |
|---------|---------|--------|
| Auth (INV-AUTH-*) | ConfigChange | Immediate |
| ToolPermission (INV-TOOL-*) | PolicyAdjust | Immediate |
| Cancellation (INV-CANCEL-*) | CodeFix | Soon |
| Audit (INV-AUDIT-02) | CodeFix | Soon |
| 其他 | ManualIntervention | Later |

每个 `FixSuggestion` 包含：
- 修复类型和优先级
- 修复描述
- 具体步骤列表
- 预期结果

#### 2.7.2 自我进化 — `evolution.rs`

`EvolutionEngine` 从历史数据中学习模式，生成优化策略：

```text
ViolationReport ──→ learn_from_violations() ──→ FailurePattern
RollbackLog    ──→ learn_from_rollbacks()  ──↗
                                                    ↓
                              generate_strategy() ──→ EvolutionStrategy
                                                    ↓
                              take_snapshot()     ──→ EvolutionSnapshot
                                                    ↓
                              compare_snapshots() ──→ EvolutionDelta
                                                         ↓
                                                    improved: true/false
```

**策略类型与类别映射**：

| 失败类别 | 策略类型 | 说明 |
|---------|---------|------|
| Auth | PromptImprovement | 改进认证相关的提示词 |
| ToolPermission | NewInvariant | 添加新的不变量规则 |
| Cancellation | ToolchainOptimize | 优化取消传播的工具链 |
| Session | ArchitectureChange | 会话管理架构调整 |
| 其他 | RoutingAdjust | 调整路由策略 |

**快照比较**：

`EvolutionDelta` 对比两个时间点的系统状态：
- `violation_delta` — 违规数变化
- `rollback_delta` — 回滚数变化
- `healed_delta` — 修复数变化
- `improved` — 整体是否改善

---

### 2.8 Control Plane（控制面）

> 路径：`crates/orangecoding-control-protocol/src/`, `crates/orangecoding-control-server/src/`, `crates/orangecoding-worker/src/`

#### 2.8.1 控制协议 — `orangecoding-control-protocol`

定义浏览器/Worker 与服务器之间的通信协议：

**客户端命令**（`ClientCommand`，6 种）：

| 命令 | 说明 |
|------|------|
| `UserMessage` | 用户输入消息 |
| `CreateSession` | 创建新会话 |
| `ListSessions` | 列出所有会话 |
| `CancelTask` | 取消当前任务 |
| `RespondToApproval` | 响应审批请求 |
| `Ping` | 保活心跳 |

**服务器事件**（`ServerEvent`，13 种）：

| 事件 | 说明 |
|------|------|
| `SessionCreated` | 会话已创建 |
| `SessionSnapshot` | 完整会话状态快照 |
| `AssistantDelta` | 流式文本片段 |
| `AssistantDone` | 文本输出完成 |
| `ToolCallStarted` | 工具执行开始 |
| `ToolCallCompleted` | 工具执行完成 |
| `ToolCallFailed` | 工具执行失败 |
| `ApprovalRequired` | 需要人工审批 |
| `ApprovalResolved` | 审批已决策 |
| `AgentStatus` | 代理状态更新 |
| `UsageDelta` | Token 消耗更新 |
| `Error` | 错误事件 |
| `Pong` | 心跳响应 |

#### 2.8.2 控制服务器 — `orangecoding-control-server`

HTTP + WebSocket 服务器，提供 Web 控制界面的后端：

| 路由 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查（无需认证） |
| `/sessions` | GET | 列出会话 |
| `/sessions` | POST | 创建会话 |
| `/ws` | WS | WebSocket 网关（JSON 消息协议） |

WebSocket 认证流程：

```text
客户端 → /ws?token=xxx → LocalAuth 验证 → WsState 建立
                                              ↓
                              双向 JSON 消息流：
                              ← ServerEvent (推送)
                              → ClientCommand (接收)
```

---

## 3. 执行循环

Agent OS 的核心执行循环遵循 **TDD + 不变量验证** 的范式：

```text
┌──────────────────────────────────────────────────────────────┐
│                        执行循环                              │
│                                                              │
│  ① 选择 TODO                                                │
│     ↓                                                        │
│  ② 编写测试（预期 FAIL）                                      │
│     ↓                                                        │
│  ③ 最小实现                                                   │
│     ↓                                                        │
│  ④ 运行测试                                                   │
│     ├── PASS → ⑤                                             │
│     └── FAIL → 自我修复 → ③                                   │
│  ⑤ 不变量检查                                                 │
│     ├── 通过 → ⑥                                             │
│     └── 违规 → 自动回滚 → ③ 或 自我修复                        │
│  ⑥ 验证代理 5 项检查                                          │
│     ├── Approved → ⑦                                         │
│     ├── NeedsWork → 自我修复 → ③                              │
│     └── Rejected → 自动回滚 → ①                               │
│  ⑦ 提交（git commit）                                        │
│     ↓                                                        │
│  ⑧ PreCheckGate 分析                                         │
│     ├── Allow → 提交成功                                      │
│     ├── Warn → 记录警告 → 提交                                │
│     └── Block → 自动回滚                                      │
│                                                              │
│  ── 循环至所有 TODO 完成 ──                                    │
│                                                              │
│  进化引擎：                                                   │
│  每轮结束 → 学习模式 → 生成策略 → 快照比较                      │
└──────────────────────────────────────────────────────────────┘
```

**失败恢复策略**：

| 失败类型 | 恢复路径 |
|---------|---------|
| 测试失败 | SelfHealer → 生成修复建议 → 重新实现 |
| 不变量违规 | AutoRollback → git revert → SelfHealer → 修复 |
| 验证拒绝 | AutoRollback → 重新规划 |
| 安全告警 | RuntimeGuard → Deny → 记录审计 |

---

## 4. 数据流

### 4.1 正常执行流

```text
用户输入
  ↓
AgentLoop (orangecoding-agent)
  ↓
AI 模型调用 (orangecoding-ai)
  ↓
工具调用请求
  ↓
RuntimeGuard.check_tool_call()  ← 运行时拦截
  ├── Deny → 拒绝 + 审计
  ├── RequireApproval → ApprovalBridge → 等待审批
  └── Allow ↓
ToolExecutor
  ↓
validate_input() → check_permissions() → execute()
  ↓
结果 → AuditLogger.log()  ← 审计记录
  ↓
结果反馈 AI → 流式输出 → 用户
```

### 4.2 违规检测与修复流

```text
操作结果
  ↓
InvariantChecker.check(ctx)
  ↓
ViolationReport
  ├── is_clean() → 继续
  └── has violations ↓
      SelfHealer.detect_from_report()
        ↓
      generate_suggestion()
        ↓
      FixSuggestion { steps, expected_outcome }
        ↓
      执行修复 → mark_pending_verification()
        ↓
      重新检查 → mark_healed() / mark_failed()
```

### 4.3 进化学习流

```text
历史 ViolationReport 集合
  ↓
EvolutionEngine.learn_from_violations()
  ↓
FailurePattern { category, frequency, related_rules }
  ↓
generate_strategy()
  ↓
EvolutionStrategy { type, description, expected_improvement }
  ↓
take_snapshot() → EvolutionSnapshot
  ↓
compare_snapshots(before, after) → EvolutionDelta { improved: bool }
```

### 4.4 控制面数据流

```text
浏览器 UI
  ↓ WebSocket
ControlServer (orangecoding-control-server)
  ↓ ClientCommand
WorkerRuntime (orangecoding-worker)
  ├── SessionSupervisor → 会话管理
  ├── ApprovalBridge → 审批决策
  └── AgentExecutor → 代理执行
       ↓ AgentEvent
  EventBridge → ServerEvent 转换
       ↓ broadcast
  WebSocket → 浏览器 UI
```

---

## 5. 安全模型

### 5.1 认证层

| 不变量 | 防护点 | 实现 |
|--------|--------|------|
| INV-AUTH-01 | WebSocket 握手 | `RuntimeGuard.check_ws_connection()` |
| INV-AUTH-02 | HTTP 端点 | `LocalAuth` 中间件 |
| INV-AUTH-03 | 日志输出 | `Sanitizer` + `SecretObfuscator` |

### 5.2 工具权限模型

```text
工具调用请求
  ↓
PermissionChecker
  ├── PermissionKind: Read / Write / Execute / Delete / Network
  ├── PermissionLevel: None / User / Admin / System
  └── PermissionPolicy → PermissionDecision
       ├── Approved → 执行
       └── Denied → 拒绝 + 审计
```

**高危工具**（`is_destructive = true`）需要额外的 `check_permissions` 调用（INV-TOOL-01）。

### 5.3 审计链完整性

```text
Entry₁ ──hash₁──→ Entry₂ ──hash₂──→ Entry₃ ──hash₃──→ ...
  ↑                  ↑                  ↑
genesis(000...0)  SHA-256(hash₁+data₂)  SHA-256(hash₂+data₃)
```

任何篡改都会导致后续哈希链断裂（INV-AUDIT-02）。

### 5.4 防护拦截矩阵

| 拦截点 | 检查内容 | 违规动作 |
|--------|---------|---------|
| WebSocket 连接 | Token 有效性 | Deny（断开连接） |
| 工具调用 | 权限 + 风险级别 | Deny / RequireApproval |
| 会话操作 | 会话状态（是否关闭） | Deny |
| 取消传播 | 子 token 传播完整性 | Deny |
| Git 提交 | Diff 影响分析 | Block / Warn |

### 5.5 路径安全

- `PathValidator` — 防止目录穿越（`../`）
- `FileOperationGuard` — 限制文件操作范围
- `SecurityPolicy` — 可配置的安全约束

---

## 6. 系统能力矩阵

| 能力 | 模块 | 关键类型 | 状态 |
|------|------|---------|------|
| 不变量定义 | `orangecoding-invariant/rules.rs` | `InvariantRule` (18 条) | ✅ 已完成 |
| 运行时检查 | `orangecoding-invariant/checker.rs` | `InvariantChecker` | ✅ 已完成 |
| 违规报告 | `orangecoding-invariant/report.rs` | `ViolationReport` | ✅ 已完成 |
| 预检门控 | `orangecoding-invariant/gate.rs` | `PreCheckGate` | ✅ 已集成到 executor |
| 运行时守卫 | `orangecoding-invariant/runtime_guard.rs` | `RuntimeGuard` | ✅ 已集成到 executor |
| 自动回滚 | `orangecoding-invariant/rollback.rs` | `AutoRollback` | ✅ 已完成 |
| 验证代理 | `orangecoding-invariant/verification.rs` | `VerificationAgent` | ✅ 已完成 |
| 自我修复 | `orangecoding-invariant/healing.rs` | `SelfHealer` | ✅ 已完成 |
| 自我进化 | `orangecoding-invariant/evolution.rs` | `EvolutionEngine` | ✅ 已完成 |
| 工具系统 | `orangecoding-tools` | `Tool` trait (24+ 工具) | ✅ 已完成 |
| 会话管理 | `orangecoding-session` | `SessionManager` | ✅ 已完成 |
| 审计追踪 | `orangecoding-audit` | `AuditLogger` + `HashChain` | ✅ 已完成 |
| MCP 协议 | `orangecoding-mcp` | `McpServer` / `McpClient` | ✅ 已完成 |
| 多代理编排 | `orangecoding-mesh` | `TaskOrchestrator` | ✅ 已完成 |
| 代理执行 | `orangecoding-agent` | `AgentLoop` | ✅ 已完成 |
| 本地 Web 控制面 | `orangecoding-control-server` | `ControlServer` | ✅ Phase A |
| 远程 Worker 连接 | `orangecoding-worker` | `WorkerRuntime` | 🔄 Phase B |
| 公网控制面 | — | Gateway + 认证 | 📋 Phase C |

---

## 7. 模块依赖图

```text
                         ┌──────────────┐
                         │  orangecoding-cli   │
                         └──────┬───────┘
                                │
                    ┌───────────┼───────────┐
                    ↓           ↓           ↓
             ┌──────────┐ ┌─────────┐ ┌─────────┐
             │ orangecoding-tui │ │orangecoding-mcp│ │orangecoding-cfg│
             └────┬─────┘ └────┬────┘ └─────────┘
                  │            │
                  ↓            ↓
           ┌────────────────────────────────────┐
           │          orangecoding-agent                │
           │  AgentLoop · CancellationToken      │
           │  Memory · TaskSystem · Compaction   │
           └──────────┬─────────────────────────┘
                      │
          ┌───────────┼────────────┐
          ↓           ↓            ↓
    ┌──────────┐ ┌──────────┐ ┌──────────┐
    │orangecoding-tool│ │orangecoding-mesh│ │ orangecoding-ai │
    │ 24+ tools│ │Orchestr. │ │ModelProxy│
    └────┬─────┘ └────┬─────┘ └──────────┘
         │            │
         ↓            ↓
    ┌──────────┐ ┌──────────────────┐
    │orangecoding-sess│ │  orangecoding-invariant  │
    │MessagePak│ │ Rules · Checker   │
    └────┬─────┘ │ Gate · Guard      │
         │       │ Rollback · Verify │
         │       │ Healing · Evolve  │
         │       └────────┬─────────┘
         │                │
         ↓                ↓
    ┌──────────┐    ┌──────────┐
    │orangecoding-audi│    │orangecoding-core│
    │ HashChain│    │ Event    │
    │ Sanitizer│    │ Types    │
    └────┬─────┘    └──────────┘
         │                ↑
         └────────────────┘

    ┌──────────────────────────────────┐
    │         Control Plane            │
    │                                  │
    │  ┌──────────────────────────┐    │
    │  │  orangecoding-control-server    │    │
    │  │  HTTP + WebSocket        │    │
    │  └────────────┬─────────────┘    │
    │               ↓                  │
    │  ┌──────────────────────────┐    │
    │  │    orangecoding-worker          │    │
    │  │  SessionSupervisor       │    │
    │  │  ApprovalBridge          │    │
    │  │  EventBridge             │    │
    │  └────────────┬─────────────┘    │
    │               ↓                  │
    │  ┌──────────────────────────┐    │
    │  │ orangecoding-control-protocol   │    │
    │  │ ClientCommand (6)        │    │
    │  │ ServerEvent (13)         │    │
    │  └──────────────────────────┘    │
    └──────────────────────────────────┘
```

---

## 8. 未来演进

### Phase B: 远程 Worker 连接

**目标**：支持远程 Worker 节点连接到控制面，实现分布式代理执行。

- Worker 注册与心跳
- 远程会话同步
- 负载均衡与任务分发
- Worker 健康监控

### Phase C: 公网控制面

**目标**：将控制面暴露到公网，支持多用户远程访问。

- Gateway 网关（反向代理 + 负载均衡）
- OAuth2 / JWT 认证
- RBAC 权限管理
- 速率限制与 DDoS 防护
- TLS 终止

### 增强型迭代代码审查

**目标**：提升验证代理的代码审查能力。

- AST 级别的代码分析
- 跨文件依赖追踪
- 代码风格一致性检查
- 性能模式检测

### 进化引擎增强

**目标**：从被动学习升级为主动优化。

- A/B 测试策略效果
- 自动调整不变量阈值
- 跨项目模式迁移
- 策略效果的统计显著性验证
