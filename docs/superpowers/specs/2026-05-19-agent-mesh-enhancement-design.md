# Agent + Mesh Enhancement Design Spec

**Date:** 2026-05-19
**Status:** Draft
**Scope:** modules/agent + modules/mesh
**Architecture:** Mesh-Native

## Background

The agent and mesh modules are currently decoupled. The mesh module provides `MessageBus`, `TaskOrchestrator`, `AgentRegistry`, and `Negotiator`, but uses only `core` types without referencing the agent module's `AgentLoop`, `BaseAgent`, or `Agent` interface. The `Negotiator` is fire-and-forget -- it publishes a `HandoffMessage` but nothing consumes it to route work to a real agent. Meanwhile, the agent module has its own independent `Orchestrator` in `harness_handoff.go` that does not use `mesh.TaskOrchestrator`.

This spec bridges the two modules and adds four major capability areas: multi-agent collaboration, agent lifecycle management, message bus enhancement, and tool execution safety.

## Architecture: Mesh-Native

The mesh module becomes the collaboration hub. Agent instances register as mesh citizens. All inter-agent communication flows through the mesh message bus.

```
mesh  = collaboration hub (registration, scheduling, communication, security)
agent = execution unit (receive tasks, execute tools, return results)
core  = shared types (messages, events, IDs, errors)
```

## 1. Message Bus Enhancement

### 1.1 ReliableBus

Replace the fire-and-forget `MessageBus` with `ReliableBus` that provides:

- **Message acknowledgment:** Each message gets a unique `MessageId`. Subscribers must explicitly `Ack()` delivery. Unacknowledged messages are redelivered after a timeout (default 30s), up to 3 retries.
- **Dead letter queue:** Messages exceeding the retry limit enter a dead letter queue for inspection.
- **Topic-ordered delivery:** Messages within the same topic are delivered in FIFO order.

```go
type ReliableBus struct {
    store   MessageStore
    guard   SecurityGuard
    streams map[string]Stream
}

type Delivery struct {
    Message
    Ack()  error
    Nack() error
}
```

### 1.2 Message Persistence

- Default storage backend: bbolt (already a project dependency via the audit module).
- `MessageStore` interface: `Store(msg)`, `Pending(topic)`, `MarkDelivered(id)`, `DeadLetters()`.
- On startup, recover undelivered messages from the store to prevent data loss.
- Configurable TTL for automatic cleanup of expired messages.

```go
type MessageStore interface {
    Store(ctx context.Context, msg Message) error
    Pending(ctx context.Context, topic string) ([]Message, error)
    MarkDelivered(ctx context.Context, id MessageId) error
    DeadLetters(ctx context.Context) ([]Message, error)
    Close() error
}
```

### 1.3 Streaming Progress

Agents push progress updates during task execution via `Stream`:

```go
type StreamEvent struct {
    TaskID  string
    Type    StreamEventType  // Progress | Artifact | Log
    Percent int              // Progress only
    Message string
    Content []byte           // Artifact only
    Level   string           // Log only
}

type Stream interface {
    Publish(ctx context.Context, event StreamEvent) error
    Subscribe(ctx context.Context, taskID string) (<-chan StreamEvent, error)
}
```

The TUI and control-server consume streams to render real-time agent progress.

### 1.4 Security

- Each agent receives an `AgentToken` upon registration.
- `SecurityGuard.ValidateMessage()` checks token and topic-level permissions before message delivery.
- Agents can only publish to and subscribe from topics they are authorized for.

### 1.5 Shared Types in Core

The following types live in `core` to avoid circular dependencies between mesh and agent:

```go
// core/task.go
type Task struct {
    ID          TaskId
    Type        TaskType      // Coding, Review, Exploration, General
    Description string
    Priority    int
    ParentID    TaskId        // for sub-tasks
    Dependencies []TaskId
}

type TaskResult struct {
    TaskID  TaskId
    Status  TaskStatus    // Completed, Failed, Skipped
    Output  string
    Error   error
}
```

### 1.5 File Changes

| File | Action |
|---|---|
| `modules/mesh/bus.go` | Keep, mark deprecated |
| `modules/mesh/reliable_bus.go` | New: ReliableBus, Delivery |
| `modules/mesh/message_store.go` | New: MessageStore interface + BoltMessageStore |
| `modules/mesh/stream.go` | New: Stream, StreamEvent, stream manager |

## 2. Agent Lifecycle Management

### 2.1 State Machine

```
              Register()
  [Unregistered] -------> [Idle]
                             | ^
                  AssignTask()  TaskComplete()
                             | |    HealthCheck fail
                             v |    \
                          [Busy]   [Unhealthy]
                             |         |
                      Stop() |    AutoRestart()
                             v         v
                          [Stopped] -> [Idle] (re-registered)
                             |
                       resources released
```

### 2.2 AgentPool

`AgentPool` manages a set of `ManagedAgent` instances:

```go
type AgentPool struct {
    registry    *AgentRegistry
    bus         *ReliableBus
    monitor     *HealthMonitor
    budget      ResourceBudget
    maxAgents   int
    idleTimeout time.Duration
}

func (p *AgentPool) Acquire(ctx context.Context, role core.AgentRole, caps []string) (ManagedAgent, error)
func (p *AgentPool) Release(agentID core.AgentId) error
func (p *AgentPool) Status() PoolStatus
```

- `Acquire` matches idle agents by role/capability. Creates a new agent if none available and under budget.
- `Release` returns the agent to the pool. Agents idle for longer than `idleTimeout` (default 5 minutes) are stopped.
- Maximum concurrent agents configurable (default: number of CPU cores).

### 2.3 Health Monitoring and Self-Healing

- Each agent runs a heartbeat goroutine (default 10s interval).
- `HealthMonitor` aggregates heartbeats. Three consecutive missed heartbeats = unhealthy.
- Auto-restart strategy: unhealthy -> wait 5s -> restart -> up to 3 attempts -> mark failed and notify user.
- On restart, restore last checkpoint using the agent module's existing `CheckpointStore`.

### 2.4 Resource Budgets

```go
type ResourceBudget struct {
    MaxConcurrentAgents int
    MaxMemoryMB         int64
    MaxCPUTime          time.Duration
    TaskTimeout         time.Duration
}
```

- Resource check before agent creation. Insufficient resources -> queue the request.
- `TaskTimeout` via context timeout. Graceful termination on expiry.
- Real-time metrics: active count, memory usage, queue length.

### 2.5 State Visualization

- `AgentPool.Status()` returns a snapshot of all agent states.
- TUI: new Agent panel showing active/idle/failed agents with status indicators.
- Control-server: `/api/v1/agents` endpoint returning JSON status.
- State changes published via `Stream` for real-time UI updates.

### 2.6 File Changes

| File | Action |
|---|---|
| `modules/mesh/agent_pool.go` | New: AgentPool, Acquire/Release |
| `modules/mesh/health.go` | New: HealthMonitor, Heartbeat |
| `modules/mesh/registry.go` | Modify: expand AgentInfo with status, health, budget fields |
| `modules/agent/agents/base.go` | Modify: BaseAgent implements ManagedAgent |

## 3. Multi-Agent Collaboration

### 3.1 ManagedAgent Interface

The unified interface for agent instances within the mesh:

```go
type ManagedAgent interface {
    ID() core.AgentId
    Role() core.AgentRole
    Capabilities() []string
    Status() AgentStatus
    AssignTask(ctx context.Context, task Task) (TaskResult, error)
    HealthCheck(ctx context.Context) HealthReport
    Stop(ctx context.Context, reason string) error
}
```

### 3.2 CollaborationProtocol

All collaboration modes implement a common interface:

```go
type CollaborationProtocol interface {
    Execute(ctx context.Context, plan AssignmentPlan) ([]TaskResult, error)
}
```

### 3.3 Master-Worker Orchestration

```
                    +--- Worker A (Explore) ---+
  User Task --> Master --> Worker B (Atlas)  --> Master --> Result
                    +--- Worker C (Hephaestus) -+
                        | parallel              ^ aggregation
```

- Master decomposes the task using `Orchestrator.Decompose()`.
- Sub-tasks assigned to workers matching role/capability.
- Workers execute in parallel. Master aggregates results.
- Error handling: single worker failure -> Master decides retry or skip.

### 3.4 Pipeline (Chain)

```
  Task --> Prometheus (Plan) --> Atlas (Execute) --> Momus (Review) --> Result
                                      | fail
                                  Hephaestus (Fix) --> retry Atlas
```

- Predefined or dynamically constructed stage sequence.
- Each stage's output is the next stage's input.
- Failure triggers recovery chain: current stage -> repair agent -> retry.
- Data and progress between stages via `ReliableBus`.

### 3.5 Peer Negotiation

```
  Agent A --> "I can do X" --> Bus --> Agent B "I'll do Y"
                                --> Agent C "I need Z first"
```

- No central coordinator. Agents claim tasks through the `Negotiator`.
- `Negotiator.Announce(task)` broadcasts a task on the bus.
- Agents submit bids via `Negotiator.Bid(task, proposal)`.
- Bid selection: capability match + current load.
- Timeout with no bids -> fall back to Master-Worker.

### 3.6 Dynamic Collaboration

- `CollaborationRouter` uses a `TaskClassifier` interface to classify task type (avoiding direct mesh-to-agent dependency).
- Coding tasks -> Master-Worker (multiple Sisyphus in parallel).
- Review tasks -> Pipeline (Plan -> Execute -> Review).
- Exploration tasks -> Peer Negotiation (multiple Explore agents).
- Supports runtime mode switching.

```go
// TaskClassifier abstracts intent classification to avoid mesh -> agent dependency.
// The agent module provides the concrete implementation.
type TaskClassifier interface {
    Classify(ctx context.Context, task Task) TaskType
}
```

### 3.7 CollaborationRouter

```go
type CollaborationRouter struct {
    classifier TaskClassifier           // injected, avoids mesh -> agent dependency
    protocols  map[TaskType]CollaborationProtocol
    fallback   CollaborationProtocol
}

func (r *CollaborationRouter) Route(ctx context.Context, task Task) ([]TaskResult, error)
```

### 3.8 File Changes

| File | Action |
|---|---|
| `modules/mesh/collaboration.go` | New: CollaborationProtocol interface, CollaborationRouter, AssignmentPlan |
| `modules/mesh/master_worker.go` | New: MasterWorker protocol |
| `modules/mesh/pipeline.go` | New: Pipeline protocol |
| `modules/mesh/peer.go` | New: PeerNegotiation protocol |
| `modules/mesh/dynamic.go` | New: DynamicCollaboration protocol |
| `modules/mesh/orchestrator.go` | Modify: integrate with AgentPool |
| `modules/mesh/negotiator.go` | Modify: upgrade to real task negotiation with bidding |

## 4. Tool Execution Safety

### 4.1 Security Guard Pipeline

```
  AgentLoop.Run()
       |
       v tool call request
  +-- SecurityGuard ----------------------------------+
  |  1. PermissionGuard (tool access control)          |
  |  2. CommandApprovalGuard (command approval flow)   |
  |  3. BudgetGuard (execution budget control)         |
  +---------------- all pass -------------------------+
       |
       v execute tool
  +-- OutputValidator --------------------------------+
  |  4. ResultValidator (output validation)            |
  +---------------------------------------------------+
       |
       v return result to AgentLoop
```

### 4.2 PermissionGuard

- Each agent role has a predefined tool whitelist (extend existing `newFilteredAgent` to RBAC).
- `PermissionGuard.Check(agentRole, toolName) (bool, string)`.
- Runtime dynamic authorization: Master can temporarily grant Worker additional tool permissions.
- Configuration file defines default permission policies, overridable per task.

```go
type PermissionGuard struct {
    policies map[core.AgentRole][]string  // role -> allowed tools
    grants   map[core.AgentId][]string    // dynamic grants
}

func (g *PermissionGuard) Check(role core.AgentRole, agentID, tool string) (bool, string)
func (g *PermissionGuard) Grant(agentID core.AgentId, tools []string, ttl time.Duration)
```

### 4.3 CommandApprovalGuard

Bash tool commands go through two-layer filtering before execution:

- **Blacklist:** destructive commands like `rm -rf /`, `mkfs`, `dd if=/dev/zero`.
- **Pattern matching:** high-risk patterns like `sudo *`, `chmod 777 *`, `> /etc/*`.
- Non-blacklisted, non-whitelisted commands enter "pending approval" state.
- Local mode: auto-approve whitelisted commands; others request user confirmation via TUI or control-server.
- Approval results cached; identical commands don't re-prompt.

```go
type CommandApprovalGuard struct {
    blacklist  []*regexp.Regexp
    patterns   []*regexp.Regexp
    whitelist  []*regexp.Regexp
    cache      ApprovalCache
    approver   Approver  // UI integration point
}

// Approver decides whether to approve a pending command.
// Implemented by TUI or control-server.
type Approver interface {
    Approve(ctx context.Context, cmd string) (bool, error)
}
```

### 4.4 BudgetGuard

Each agent task receives a `ToolBudget`:

```go
type ToolBudget struct {
    MaxCalls    int
    MaxTokens   int64
    MaxWallTime time.Duration
}

type BudgetGuard struct {
    budgets map[core.AgentId]*ToolBudget
    usage   map[core.AgentId]*BudgetUsage
}

func (g *BudgetGuard) Check(agentID core.AgentId) (bool, string)
```

- Three budget dimensions: call count, token consumption, wall clock time.
- Over-budget returns an error instead of executing. Agent can request more budget or change strategy.
- Master can assign different budgets to different workers.

**Relationship to existing agent guardrails:** The agent module already has `TokenBudgetGuardrail` (in `harness_guardrail.go`) and `ToolUseBudget` (in `harness_handoff.go`) for per-agent-run budgets. The mesh `BudgetGuard` operates at a higher level -- it controls aggregate budgets across the agent pool (how many total tool calls an agent can make across all its tasks). The per-run guardrails remain for individual task execution limits.

### 4.5 Output Validation

Validator chain: `SchemaValidator -> SizeValidator -> AnomalyValidator`

- **SchemaValidator:** checks output format (non-empty file content, valid JSON, etc.).
- **SizeValidator:** output size under configured limit (default 1MB).
- **AnomalyValidator:** detects anomalies (non-zero exit code, "permission denied", timeout).
- Failed validation marks the result as a warning attached to the agent response.

```go
type OutputValidator struct {
    maxSize  int64
    patterns []*regexp.Regexp  // anomaly patterns
}

func (v *OutputValidator) Validate(result ToolResult) (valid bool, warnings []string)
```

### 4.6 File Changes

| File | Action |
|---|---|
| `modules/mesh/security.go` | New: SecurityGuard, PermissionGuard, CommandApprovalGuard |
| `modules/mesh/validation.go` | New: OutputValidator, SchemaValidator, SizeValidator, AnomalyValidator |
| `modules/mesh/budget.go` | New: ToolBudget, BudgetGuard |
| `modules/agent/executor.go` | Modify: integrate SecurityGuard into tool execution path |
| `modules/agent/loop.go` | Modify: integrate Stream for progress reporting |
| `modules/agent/security_bridge.go` | New: bridge between mesh SecurityGuard and agent execution |

## 5. End-to-End Task Flow

```
1. User submits task (TUI / CLI / API)
2. mesh.CollaborationRouter.Analyze(task)
   -> IntentGate classifies task type
   -> Selects collaboration protocol (MasterWorker/Pipeline/Peer/Dynamic)
3. Protocol creates AssignmentPlan
   -> TaskScheduler.Schedule() assigns agents
   -> AgentPool.Acquire() gets/creates agent instances
4. Agent.AssignTask(task)
   -> AgentLoop.Run() starts
   -> Each tool call passes through SecurityGuard
   -> Progress pushed via Stream in real-time
5. Task completes
   -> Results delivered via ReliableBus
   -> AgentPool.Release() returns agent
   -> Protocol aggregates results, returns to user
```

## 6. Complete File Change Summary

### mesh module (new files)

| File | Contents |
|---|---|
| `reliable_bus.go` | ReliableBus, Delivery, Ack/Nack, topic-ordered delivery |
| `message_store.go` | MessageStore interface, BoltMessageStore implementation |
| `stream.go` | Stream, StreamEvent, stream manager |
| `security.go` | SecurityGuard, PermissionGuard, CommandApprovalGuard |
| `validation.go` | OutputValidator, SchemaValidator, SizeValidator, AnomalyValidator |
| `budget.go` | ToolBudget, BudgetGuard, BudgetUsage |
| `agent_pool.go` | AgentPool, Acquire/Release/Status |
| `health.go` | HealthMonitor, Heartbeat, HealthReport |
| `collaboration.go` | CollaborationProtocol, CollaborationRouter, AssignmentPlan |
| `master_worker.go` | MasterWorker protocol implementation |
| `pipeline.go` | Pipeline protocol implementation |
| `peer.go` | PeerNegotiation protocol implementation |
| `dynamic.go` | DynamicCollaboration protocol implementation |

### mesh module (modified files)

| File | Changes |
|---|---|
| `orchestrator.go` | Integrate with AgentPool for agent-based task execution |
| `registry.go` | Expand AgentInfo with status, health, budget fields |
| `negotiator.go` | Upgrade to real task negotiation with bidding and acknowledgment |
| `mesh_test.go` | Expand tests for all new components |

### agent module (modified files)

| File | Changes |
|---|---|
| `agents/base.go` | BaseAgent implements ManagedAgent interface |
| `executor.go` | Integrate SecurityGuard into tool execution pipeline |
| `loop.go` | Integrate Stream for progress reporting |

### agent module (new files)

| File | Contents |
|---|---|
| `security_bridge.go` | Bridge between mesh SecurityGuard and agent execution layer |

## 7. Testing Strategy

- **Unit tests:** Each new component (ReliableBus, AgentPool, each protocol, each guard) gets isolated unit tests with mocked dependencies.
- **Integration tests:** End-to-end flow from task submission through collaboration to result aggregation, using real agent instances with stubbed AI providers.
- **Concurrency tests:** Verify reliable delivery under concurrent publishers/subscribers, agent pool thread safety, and health monitor accuracy under load.
- **Security tests:** Validate permission enforcement, command filtering, and budget limits with adversarial inputs.

## 8. Compatibility Notes

- The existing `MessageBus` remains available for backward compatibility, marked deprecated.
- The existing `Negotiator` API is preserved; new negotiation features added as extensions.
- The agent module's existing `Orchestrator` in `harness_handoff.go` continues to work for single-agent tasks. The mesh collaboration layer is for multi-agent scenarios only.
- `BaseAgent` gains new methods for ManagedAgent compliance but existing callers are unaffected.
