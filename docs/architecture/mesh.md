# Mesh 架构

> CEAIR Mesh 是多 Agent 协调的基础设施层，提供消息路由、状态共享、任务编排和角色管理。

## 目录

- [概述](#概述)
- [消息总线 (MessageBus)](#消息总线-messagebus)
- [模型路由 (ModelRouter)](#模型路由-modelrouter)
- [共享状态 (SharedState)](#共享状态-sharedstate)
- [任务编排 (TaskOrchestrator)](#任务编排-taskorchestrator)
- [角色系统 (RoleSystem)](#角色系统-rolesystem)
- [代理注册表 (AgentRegistry)](#代理注册表-agentregistry)
- [Agent 通信 (AgentComm)](#agent-通信-agentcomm)
- [协商协议 (Negotiation)](#协商协议-negotiation)
- [任务移交 (TaskHandoff)](#任务移交-taskhandoff)
- [Mesh 初始化](#mesh-初始化)

---

## 概述

Mesh 层（`ceair-mesh` crate）是多 Agent 系统的"神经网络"，负责 Agent 之间的一切协调工作：

```
┌──────────────────────────────────────────────────────────────┐
│                       ceair-mesh                              │
│                    多 Agent 协调基础设施                        │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐    │
│  │                   MessageBus                          │    │
│  │              发布/订阅 消息路由                         │    │
│  │                                                      │    │
│  │  ┌─────┐   ┌─────┐   ┌─────┐   ┌─────┐   ┌─────┐  │    │
│  │  │Agent│◄─►│Agent│◄─►│Agent│◄─►│Agent│◄─►│Agent│  │    │
│  │  │  A  │   │  B  │   │  C  │   │  D  │   │  E  │  │    │
│  │  └─────┘   └─────┘   └─────┘   └─────┘   └─────┘  │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌────────────┐  ┌──────────────┐  ┌───────────────────┐    │
│  │SharedState │  │ ModelRouter  │  │ TaskOrchestrator  │    │
│  │共享状态管理 │  │ 模型路由选择  │  │ DAG 任务编排      │    │
│  └────────────┘  └──────────────┘  └───────────────────┘    │
│                                                              │
│  ┌────────────┐  ┌──────────────┐  ┌───────────────────┐    │
│  │RoleSystem  │  │AgentRegistry │  │ Negotiation       │    │
│  │角色权限定义 │  │Agent注册发现  │  │ 任务协商协议       │    │
│  └────────────┘  └──────────────┘  └───────────────────┘    │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## 消息总线 (MessageBus)

### 概述

MessageBus 是基于 `tokio::sync::broadcast` 的发布/订阅消息系统，支持 Agent 间的异步通信。

### 核心类型

```rust
/// 消息总线
pub struct MessageBus {
    /// 广播发送端
    sender: broadcast::Sender<BusMessage>,

    /// 通道容量
    capacity: usize,
}

/// 总线消息
pub struct BusMessage {
    /// 发送者 Agent ID
    pub from: AgentId,

    /// 接收者（None 表示广播）
    pub to: Option<AgentId>,

    /// 消息主题
    pub topic: String,

    /// 消息载荷（JSON）
    pub payload: Value,

    /// 发送时间
    pub timestamp: DateTime<Utc>,
}
```

### 工厂方法

```rust
impl BusMessage {
    /// 创建广播消息
    pub fn broadcast(
        from: AgentId,
        topic: impl Into<String>,
        payload: Value,
    ) -> Self;

    /// 创建定向消息
    pub fn directed(
        from: AgentId,
        to: AgentId,
        topic: impl Into<String>,
        payload: Value,
    ) -> Self;
}
```

### MessageBus API

```rust
impl MessageBus {
    /// 创建新的消息总线
    pub fn new(capacity: usize) -> Self;

    /// 发布消息到总线
    pub fn publish(&self, message: BusMessage) -> Result<usize>;

    /// 订阅所有消息
    pub fn subscribe(&self) -> broadcast::Receiver<BusMessage>;

    /// 订阅特定主题的消息
    pub fn subscribe_topic(
        &self,
        topic: &str,
    ) -> FilteredReceiver;
}
```

### 消息主题规范

| 主题 | 描述 | 载荷格式 |
|------|------|----------|
| `task.created` | 新任务创建 | `{ task_id, title, description }` |
| `task.assigned` | 任务被分配 | `{ task_id, agent_id }` |
| `task.completed` | 任务完成 | `{ task_id, result }` |
| `task.failed` | 任务失败 | `{ task_id, error }` |
| `agent.status` | Agent 状态变更 | `{ agent_id, status }` |
| `agent.heartbeat` | Agent 心跳 | `{ agent_id, timestamp }` |
| `negotiation.request` | 协商请求 | `{ task_id, requirements }` |
| `negotiation.propose` | 协商提议 | `{ task_id, agent_id, confidence }` |
| `negotiation.accept` | 接受提议 | `{ task_id, agent_id }` |
| `model.switch` | 模型切换 | `{ agent_id, from, to }` |
| `system.shutdown` | 系统关闭 | `{}` |

### 消息流示意

```
                    MessageBus (broadcast channel)
                    ┌───────────────────────┐
                    │   topic: task.*       │
    Sisyphus ──────►│   topic: agent.*     │──────► All Agents
    (publish)       │   topic: negotiation.*│       (subscribe)
                    │   topic: model.*     │
                    │   topic: system.*    │
                    └───────────────────────┘
                          │           │
                          ▼           ▼
                    ┌──────────┐ ┌──────────┐
                    │ 主题过滤  │ │ 定向过滤  │
                    │ topic =  │ │ to =     │
                    │ "task.*" │ │ agent_id │
                    └──────────┘ └──────────┘
```

---

## 模型路由 (ModelRouter)

### 概述

ModelRouter 根据任务特征动态选择最合适的 AI 模型。

### 核心类型

```rust
/// 模型路由器
pub struct ModelRouter {
    /// 路由规则列表（按优先级排序）
    rules: Vec<RoutingRule>,
}

/// 任务类型
pub enum TaskType {
    Coding,         // 编码任务
    Review,         // 代码审查
    Planning,       // 规划设计
    Documentation,  // 文档编写
    Testing,        // 测试
    General,        // 通用任务
}

/// 路由条件
pub enum RoutingCondition {
    /// 按任务类型匹配
    TaskTypeMatch(TaskType),

    /// 按复杂度阈值（>= threshold）
    ComplexityThreshold(u32),

    /// 按标签匹配
    Tag(String),

    /// 匹配所有
    Any,
}

/// 路由规则
pub struct RoutingRule {
    /// 规则名称
    pub name: String,

    /// 路由条件
    pub condition: RoutingCondition,

    /// 目标提供商
    pub provider_name: String,

    /// 目标模型
    pub model_name: String,

    /// 优先级（越大越优先）
    pub priority: u32,
}

/// 路由上下文
pub struct RoutingContext {
    /// 任务类型
    pub task_type: TaskType,

    /// 复杂度 (0-100)
    pub complexity: u32,

    /// 标签列表
    pub tags: Vec<String>,
}

/// 路由决策
pub struct RoutingDecision {
    /// 选中的提供商
    pub provider: String,

    /// 选中的模型
    pub model: String,

    /// 匹配的规则名
    pub rule_matched: String,
}
```

### ModelRouter API

```rust
impl ModelRouter {
    /// 创建默认路由器（包含内置规则）
    pub fn new() -> Self;

    /// 添加路由规则
    pub fn add_rule(&mut self, rule: RoutingRule);

    /// 根据上下文选择模型
    pub fn route(&self, context: &RoutingContext) -> RoutingDecision;
}
```

### 默认路由规则

```
┌─────────────────────────────────────────────────────────────┐
│                    默认路由规则表                              │
├─────┬─────────────────────────┬────────────┬────────────────┤
│优先级│ 条件                    │ 提供商      │ 模型           │
├─────┼─────────────────────────┼────────────┼────────────────┤
│ 100 │ 复杂度 >= 80            │ anthropic  │ claude-opus-4-6│
│  90 │ TaskType = Planning     │ anthropic  │ claude-opus-4-6│
│  80 │ TaskType = Review       │ openai     │ gpt-5.4        │
│  70 │ TaskType = Coding       │ anthropic  │claude-sonnet-4-6│
│  60 │ Tag("fast")             │ deepseek   │ deepseek-v3    │
│  50 │ TaskType = Documentation│ anthropic  │claude-sonnet-4-6│
│  40 │ TaskType = Testing      │ anthropic  │claude-sonnet-4-6│
│  10 │ Any                     │ anthropic  │claude-sonnet-4-6│
└─────┴─────────────────────────┴────────────┴────────────────┘
```

### 路由决策流程

```
输入: RoutingContext {
    task_type: Coding,
    complexity: 85,
    tags: ["rust", "async"]
}

规则匹配:
  1. 复杂度 >= 80 (priority 100) ✅ → 匹配!
     → provider: anthropic, model: claude-opus-4-6

  (如果规则 1 不存在，继续匹配:)
  2. TaskType = Coding (priority 70) ✅
     → provider: anthropic, model: claude-sonnet-4-6

输出: RoutingDecision {
    provider: "anthropic",
    model: "claude-opus-4-6",
    rule_matched: "high_complexity"
}
```

---

## 共享状态 (SharedState)

### 概述

SharedState 提供线程安全的键值存储，支持 TTL（生存时间）自动过期。

### 核心类型

```rust
/// 线程安全的共享状态
pub struct SharedState {
    /// 键值存储（值 + 可选过期时间）
    state: DashMap<String, (Value, Option<DateTime<Utc>>)>,
}
```

### SharedState API

```rust
impl SharedState {
    /// 创建空的共享状态
    pub fn new() -> Self;

    /// 存储值
    pub fn set(&self, key: impl Into<String>, value: Value);

    /// 读取值
    pub fn get(&self, key: &str) -> Option<Value>;

    /// 存储值并设置 TTL
    pub fn set_with_ttl(
        &self,
        key: impl Into<String>,
        value: Value,
        ttl: Duration,
    );

    /// 删除键
    pub fn delete(&self, key: &str) -> bool;

    /// 检查键是否存在
    pub fn contains(&self, key: &str) -> bool;

    /// 清理过期条目
    pub fn cleanup_expired(&self) -> usize;

    /// 获取所有键
    pub fn keys(&self) -> Vec<String>;
}
```

### 使用场景

| 键模式 | 用途 | TTL |
|--------|------|-----|
| `agent:{id}:status` | Agent 运行状态 | 60秒 |
| `task:{id}:progress` | 任务进度 | 无 |
| `session:{id}:context` | 会话上下文缓存 | 30分钟 |
| `model:usage:{provider}` | 模型使用统计 | 1小时 |
| `cache:search:{hash}` | 搜索结果缓存 | 5分钟 |

### 线程安全保证

```
┌─────────────────────────────────────────────────────────┐
│                  SharedState 并发模型                     │
│                                                         │
│  DashMap (分片锁)                                        │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐     │
│  │Shard│ │Shard│ │Shard│ │Shard│ │Shard│ │Shard│     │
│  │  0  │ │  1  │ │  2  │ │  3  │ │  4  │ │  N  │     │
│  └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘     │
│     │       │       │       │       │       │          │
│  ┌──┴──┐ ┌──┴──┐ ┌──┴──┐ ┌──┴──┐ ┌──┴──┐ ┌──┴──┐    │
│  │Lock │ │Lock │ │Lock │ │Lock │ │Lock │ │Lock │    │
│  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘    │
│                                                         │
│  不同分片的读写可以并行进行                                 │
│  同一分片内的写操作互斥                                     │
│  读操作不阻塞（无锁读）                                    │
└─────────────────────────────────────────────────────────┘
```

---

## 任务编排 (TaskOrchestrator)

### 概述

TaskOrchestrator 实现了基于 DAG（有向无环图）的任务调度系统。

### 核心类型

```rust
/// 任务 ID
pub struct TaskId(pub String);

/// 任务状态
pub enum TaskStatus {
    /// 等待中（有未完成的依赖）
    Pending,

    /// 就绪（所有依赖已完成）
    Ready,

    /// 执行中
    Running,

    /// 已完成
    Completed,

    /// 已失败
    Failed,

    /// 已取消
    Cancelled,
}

/// 任务定义
pub struct Task {
    /// 唯一标识
    pub id: TaskId,

    /// 任务标题
    pub title: String,

    /// 详细描述
    pub description: Option<String>,

    /// 依赖列表（DAG 边）
    pub dependencies: Vec<TaskId>,

    /// 当前状态
    pub status: TaskStatus,

    /// 分配的 Agent
    pub assigned_to: Option<AgentId>,

    /// 执行结果
    pub result: Option<Value>,
}
```

### TaskOrchestrator API

```rust
impl TaskOrchestrator {
    /// 创建空编排器
    pub fn new() -> Self;

    /// 添加任务
    pub fn add_task(&mut self, task: Task);

    /// 添加依赖关系
    pub fn add_dependency(
        &mut self,
        task_id: &TaskId,
        depends_on: &TaskId,
    ) -> Result<()>;

    /// 获取所有就绪任务（依赖已满足）
    pub fn get_ready_tasks(&self) -> Vec<TaskId>;

    /// 更新任务状态
    pub fn update_status(
        &mut self,
        task_id: &TaskId,
        status: TaskStatus,
    );

    /// 拓扑排序（返回层级分组）
    pub fn topological_sort(&self) -> Vec<Vec<TaskId>>;

    /// 获取任务详情
    pub fn get_task(&self, task_id: &TaskId) -> Option<&Task>;

    /// 获取所有任务
    pub fn all_tasks(&self) -> Vec<&Task>;
}
```

### DAG 任务调度示例

```
任务: 实现用户认证系统

DAG 结构:
                    ┌──────────────────┐
                    │ task-1: 设计数据库│
                    │ status: Ready    │
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │ task-2: 实现模型  │
                    │ status: Pending  │
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │ task-3: 实现逻辑  │
                    │ status: Pending  │
                    └───┬────────┬─────┘
                        │        │
              ┌─────────▼─┐  ┌──▼──────────┐
              │task-4: 测试│  │task-5: 安全  │
              │ Pending    │  │审查 Pending  │
              └─────────┬──┘  └──┬──────────┘
                        │        │
                    ┌───▼────────▼────┐
                    │ task-6: 文档     │
                    │ status: Pending  │
                    └─────────────────┘

拓扑排序结果（层级分组）:
  Level 0: [task-1]                    ← 可立即并行执行
  Level 1: [task-2]                    ← task-1 完成后执行
  Level 2: [task-3]                    ← task-2 完成后执行
  Level 3: [task-4, task-5]            ← task-3 完成后可并行
  Level 4: [task-6]                    ← task-4 和 task-5 都完成后

就绪任务查询 (get_ready_tasks):
  当前: [task-1]（无依赖，直接就绪）
  task-1 完成后: [task-2]
  task-2 完成后: [task-3]
  task-3 完成后: [task-4, task-5]（两个同时就绪！）
  task-4,5 完成后: [task-6]
```

### 循环依赖检测

```rust
// add_dependency 内部会检查是否形成循环
let result = orchestrator.add_dependency(
    &TaskId("task-1".into()),
    &TaskId("task-3".into()),  // task-3 → task-2 → task-1
);
// result = Err("循环依赖: task-1 → task-3 → task-2 → task-1")
```

---

## 角色系统 (RoleSystem)

### 概述

RoleSystem 管理 Agent 的角色定义，包括系统提示和工具权限。

### 核心类型

```rust
/// 角色定义
pub struct RoleDefinition {
    /// 角色类型
    pub role: AgentRole,

    /// 系统提示词
    pub system_prompt: String,

    /// 允许使用的工具列表
    pub allowed_tools: Vec<String>,

    /// 角色描述
    pub description: String,
}

/// 角色注册表
pub struct RoleRegistry {
    roles: HashMap<AgentRole, RoleDefinition>,
}
```

### RoleRegistry API

```rust
impl RoleRegistry {
    /// 创建包含默认角色的注册表
    pub fn new() -> Self;

    /// 注册自定义角色
    pub fn register_role(&mut self, def: RoleDefinition);

    /// 获取角色的系统提示
    pub fn get_system_prompt(&self, role: &AgentRole) -> Option<&str>;

    /// 检查角色是否可以使用指定工具
    pub fn is_tool_allowed(
        &self,
        role: &AgentRole,
        tool_name: &str,
    ) -> bool;

    /// 获取角色定义
    pub fn get_role(&self, role: &AgentRole) -> Option<&RoleDefinition>;

    /// 列出所有角色
    pub fn list_roles(&self) -> Vec<&AgentRole>;
}
```

### 默认角色定义

#### Coder（编码者）

```rust
RoleDefinition {
    role: AgentRole::Coder,
    system_prompt: "你是一个专业的软件工程师...",
    allowed_tools: vec![
        "read_file", "write_file", "edit",
        "bash", "grep", "find",
        "browser", "web_search", "fetch",
        "lsp", "ast_grep", "python",
        "todo", "task", "ask", "calc",
    ],
    description: "负责编写、修改和调试代码",
}
```

#### Reviewer（审查者）

```rust
RoleDefinition {
    role: AgentRole::Reviewer,
    system_prompt: "你是一个经验丰富的代码审查专家...",
    allowed_tools: vec![
        "read_file", "grep", "find",
        "browser", "web_search", "fetch",
        "lsp", "ast_grep",
        "todo", "ask",
    ],
    description: "负责代码审查，只读不写",
}
```

#### Planner（规划者）

```rust
RoleDefinition {
    role: AgentRole::Planner,
    system_prompt: "你是一个资深的技术架构师和项目规划专家...",
    allowed_tools: vec![
        "read_file", "grep", "find",
        "browser", "web_search",
        "todo", "task", "ask",
    ],
    description: "负责技术方案设计和项目规划",
}
```

#### Executor（执行者）

```rust
RoleDefinition {
    role: AgentRole::Executor,
    system_prompt: "你是一个高效的任务执行者...",
    allowed_tools: vec![
        "read_file", "write_file", "edit",
        "bash", "grep", "find",
        "fetch", "lsp", "ast_grep",
        "python", "ssh",
        "todo", "task", "ask", "calc",
    ],
    description: "负责执行具体操作任务，权限最广",
}
```

#### Observer（观察者）

```rust
RoleDefinition {
    role: AgentRole::Observer,
    system_prompt: "你是一个系统监控和状态观察者...",
    allowed_tools: vec![
        "read_file", "grep", "find",
        "browser", "web_search",
        "todo", "ask",
    ],
    description: "只读角色，负责监控和报告",
}
```

---

## 代理注册表 (AgentRegistry)

### 概述

AgentRegistry 管理系统中所有 Agent 的注册、发现和生命周期。

### 核心类型

```rust
/// Agent 信息
pub struct AgentInfo {
    /// 唯一 ID
    pub id: AgentId,

    /// 名称
    pub name: String,

    /// 角色
    pub role: AgentRole,

    /// 当前状态
    pub status: AgentStatus,

    /// 能力列表
    pub capabilities: Vec<AgentCapability>,

    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// Agent 注册表
pub struct AgentRegistry {
    /// 线程安全的 Agent 存储
    agents: DashMap<AgentId, AgentInfo>,
}
```

### AgentRegistry API

```rust
impl AgentRegistry {
    /// 创建空注册表
    pub fn new() -> Self;

    /// 注册新 Agent
    pub fn register(&self, info: AgentInfo);

    /// 注销 Agent
    pub fn unregister(&self, id: &AgentId) -> Option<AgentInfo>;

    /// 查询 Agent
    pub fn get(&self, id: &AgentId) -> Option<AgentInfo>;

    /// 列出所有 Agent
    pub fn list(&self) -> Vec<AgentInfo>;

    /// 按能力查找 Agent
    pub fn get_by_capability(
        &self,
        capability: &str,
    ) -> Vec<AgentInfo>;

    /// 更新 Agent 状态
    pub fn update_status(
        &self,
        id: &AgentId,
        status: AgentStatus,
    );

    /// 获取空闲 Agent
    pub fn get_idle_agents(&self) -> Vec<AgentInfo>;

    /// 获取 Agent 数量
    pub fn count(&self) -> usize;
}
```

### Agent 发现机制

```
┌─────────────────────────────────────────────────────────┐
│                  Agent 发现流程                           │
│                                                         │
│  1. 任务需求: "需要能力=代码审查的Agent"                   │
│     ↓                                                   │
│  2. registry.get_by_capability("code_review")            │
│     ↓                                                   │
│  3. 过滤结果:                                            │
│     ├── Momus (Reviewer, Idle)     ✅                    │
│     ├── Oracle (Reviewer, Running) ⚠️ 忙碌              │
│     └── Hephaestus (Coder, Idle)   ❌ 角色不匹配         │
│     ↓                                                   │
│  4. 选择: Momus（空闲且角色匹配）                         │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

## Agent 通信 (AgentComm)

AgentComm 提供更高层次的 Agent 间通信抽象，基于 MessageBus 构建。

```rust
pub struct AgentComm {
    bus: Arc<MessageBus>,
    agent_id: AgentId,
}

impl AgentComm {
    /// 向特定 Agent 发送消息
    pub async fn send_to(
        &self,
        target: &AgentId,
        topic: &str,
        payload: Value,
    ) -> Result<()>;

    /// 广播消息给所有 Agent
    pub async fn broadcast(
        &self,
        topic: &str,
        payload: Value,
    ) -> Result<()>;

    /// 发送请求并等待响应
    pub async fn request(
        &self,
        target: &AgentId,
        topic: &str,
        payload: Value,
        timeout: Duration,
    ) -> Result<Value>;

    /// 注册消息处理器
    pub async fn on_message<F>(
        &self,
        topic: &str,
        handler: F,
    ) where F: Fn(BusMessage) + Send + 'static;
}
```

### 请求-响应模式

```
Agent A                    MessageBus                 Agent B
   │                          │                          │
   │── request(B, "query") ──►│                          │
   │                          │── BusMessage ───────────►│
   │                          │                          │
   │   [等待响应...]           │                          │
   │                          │                    [处理请求]
   │                          │                          │
   │                          │◄── BusMessage(response)─│
   │◄── response ─────────────│                          │
   │                          │                          │
```

---

## 协商协议 (Negotiation)

详见 [Agent 系统架构 - 任务协商](./agent-system.md#任务协商-negotiationprotocol)。

协商协议在 Mesh 层实现，通过 MessageBus 传输协商消息。

```rust
pub struct NegotiationProtocol {
    bus: Arc<MessageBus>,
    registry: Arc<AgentRegistry>,
}

impl NegotiationProtocol {
    /// 发起任务协商
    pub async fn negotiate(
        &self,
        task: &Task,
        timeout: Duration,
    ) -> Result<AgentId>;

    /// 响应协商请求（Agent 调用）
    pub async fn propose(
        &self,
        task_id: &TaskId,
        confidence: f32,
    ) -> Result<()>;
}
```

---

## 任务移交 (TaskHandoff)

详见 [Agent 系统架构 - 任务重分配](./agent-system.md#任务重分配-handoffmanager)。

```rust
pub struct TaskHandoff {
    registry: Arc<AgentRegistry>,
    orchestrator: Arc<Mutex<TaskOrchestrator>>,
    bus: Arc<MessageBus>,
}

impl TaskHandoff {
    /// 执行任务移交
    pub async fn handoff(
        &self,
        task_id: &TaskId,
        from: &AgentId,
        to: &AgentId,
        context: HandoffContext,
    ) -> Result<()>;
}
```

---

## Mesh 初始化

### 初始化流程

```
┌─────────────────────────────────────────────────────────┐
│                   Mesh 初始化流程                         │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. 创建基础设施                                         │
│     let bus = Arc::new(MessageBus::new(1024));           │
│     let state = Arc::new(SharedState::new());            │
│     let registry = Arc::new(AgentRegistry::new());       │
│                                                         │
│  2. 初始化路由器                                         │
│     let router = ModelRouter::new();                     │
│     // 加载配置中的路由规则                                │
│     router.add_rule(/* ... */);                          │
│                                                         │
│  3. 初始化角色系统                                        │
│     let roles = RoleRegistry::new();                     │
│     // 默认 5 种角色已注册                                │
│                                                         │
│  4. 创建任务编排器                                        │
│     let orchestrator = TaskOrchestrator::new();           │
│                                                         │
│  5. 注册 Agent                                           │
│     for agent_kind in AgentKind::all() {                 │
│         let info = AgentInfo::from(agent_kind);          │
│         registry.register(info);                         │
│     }                                                    │
│                                                         │
│  6. 启动消息监听                                         │
│     // 各 Agent 开始订阅相关主题                           │
│     // Sisyphus 订阅 task.*, agent.*                     │
│     // Atlas 订阅 task.*, negotiation.*                   │
│     // Junior 订阅 task.assigned                         │
│                                                         │
│  7. Mesh 就绪                                            │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 组件交互图

```
┌──────────────────────────────────────────────────────────┐
│                    Mesh 组件交互                           │
│                                                          │
│                ┌──────────────┐                           │
│    ┌──────────►│  MessageBus  │◄──────────┐              │
│    │           └──────┬───────┘           │              │
│    │                  │                    │              │
│    │    ┌─────────────┼─────────────┐    │              │
│    │    │             │             │    │              │
│    │    ▼             ▼             ▼    │              │
│ ┌──┴────────┐ ┌────────────┐ ┌─────┴─────┐            │
│ │  Agent     │ │  Task      │ │Negotiation│            │
│ │  Registry  │ │Orchestrator│ │ Protocol  │            │
│ └──────┬─────┘ └──────┬─────┘ └───────────┘            │
│        │              │                                  │
│        │    ┌─────────┘                                  │
│        │    │                                            │
│        ▼    ▼                                            │
│ ┌───────────────┐     ┌──────────────┐                  │
│ │  SharedState   │     │ ModelRouter  │                  │
│ │  (DashMap)     │     │ (规则引擎)    │                  │
│ └───────────────┘     └──────────────┘                  │
│                                                          │
│ ┌───────────────┐                                        │
│ │  RoleSystem    │                                        │
│ │  (权限管理)    │                                        │
│ └───────────────┘                                        │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

---

## 相关文档

- [架构概览](./overview.md) - 系统整体架构
- [Agent 系统架构](./agent-system.md) - Agent 类型和协作详解
- [安全架构](./security.md) - 安全策略设计
- [工具参考](../reference/tools.md) - 工具系统
