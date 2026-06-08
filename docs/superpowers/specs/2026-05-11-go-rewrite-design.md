# OrangeCoding Go Rewrite Design

## Overview

Complete rewrite of OrangeCoding (a terminal-based AI coding agent) from Rust to Go, preserving all 15 subsystem modules and full feature parity.

**Current stats:** 208 Rust files, ~83,604 lines, 15 workspace crates, 2061+ tests.

**Target:** Go workspace with 15 modules, idiomatic Go code, English comments.

## Module Mapping: Rust Crate → Go Module

| Rust Crate | Go Module | Layer |
|------------|-----------|-------|
| orangecoding-core | modules/core | 0 (Base) |
| orangecoding-config | modules/config | 0 |
| orangecoding-audit | modules/audit | 0 |
| orangecoding-invariant | modules/invariant | 0 |
| orangecoding-ai | modules/ai | 1 (Service) |
| orangecoding-session | modules/session | 1 |
| orangecoding-mcp | modules/mcp | 1 |
| orangecoding-tools | modules/tools | 1 |
| orangecoding-agent | modules/agent | 2 (Engine) |
| orangecoding-mesh | modules/mesh | 2 |
| orangecoding-tui | modules/tui | 3 (Interface) |
| orangecoding-control-protocol | modules/control-protocol | 3 |
| orangecoding-control-server | modules/control-server | 3 |
| orangecoding-worker | modules/worker | 3 |
| orangecoding-cli | modules/cli | 3 |

## Dependency Graph

```
Layer 3: cli → tui, control-server, worker, mesh
Layer 2: agent → core, tools, ai, invariant
         mesh → core
Layer 1: ai → core
         session → core
         mcp → core
         tools → core
Layer 0: core (no internal deps)
         config → core
         audit → core
         invariant → core
```

Additional cross-layer deps:
- `cli` → `agent`, `config`, `tui`, `mesh`, `mcp`, `control-server`, `worker`
- `control-server` → `control-protocol`, `worker`, `core`
- `worker` → `control-protocol`, `agent`, `core`
- `tui` → `core`

## Technology Stack

| Component | Rust Library | Go Library |
|-----------|-------------|------------|
| Async runtime | tokio | goroutines + channels |
| HTTP client | reqwest | net/http |
| HTTP server | axum | gin (github.com/gin-gonic/gin) |
| WebSocket | axum/ws | gorilla/websocket |
| CLI | clap | cobra (github.com/spf13/cobra) |
| TUI | ratatui + crossterm | bubbletea + lipgloss + glamour |
| Serialization | serde + serde_json | encoding/json |
| Error handling | thiserror + anyhow | Custom error types |
| Logging | tracing | log/slog |
| Crypto | ring + sha2 | crypto/* |
| Embedded DB | sled | bbolt (go.etcd.io/bbolt) |
| Concurrency | dashmap + parking_lot | sync.* + sync.Map |
| Regex | regex | regexp |
| Glob | glob | path/filepath |
| Time | chrono | time |
| UUID | uuid | github.com/google/uuid |
| YAML | serde_yaml | gopkg.in/yaml.v3 |
| TOML | toml | github.com/BurntSushi/toml |

## Detailed Module Designs

### Layer 0: Base

#### core

Core types, errors, events, and messages shared by all other modules.

**Types:**
- `AgentId`, `SessionId`, `ToolName` — named types (not bare strings)
- `Role` — iota constants: `RoleUser`, `RoleAssistant`, `RoleSystem`, `RoleTool`
- `AgentStatus` — iota constants: `StatusIdle`, `StatusRunning`, `StatusWaiting`, `StatusError`, `StatusStopped`
- `TokenUsage` — struct with `PromptTokens`, `CompletionTokens`, `TotalTokens`
- `Conversation` — struct wrapping a slice of `Message`

**Errors:**
- `OrangeError` struct implementing `error` interface
- `ErrorKind` iota constants: `ErrConfig`, `ErrProvider`, `ErrTool`, `ErrAgent`, `ErrSession`, `ErrPermission`, `ErrIO`, etc.
- `WrapError(err, kind, message)` helper for error wrapping

**Events:**
- `AgentEvent` interface with `EventType() string` method
- Concrete event types: `TaskStartedEvent`, `TaskCompletedEvent`, `ToolCallEvent`, `ToolResultEvent`, `TokenUsageEvent`, `ErrorEvent`, etc.
- `EventBus` struct: `Subscribe(handler EventHandler)`, `Publish(event AgentEvent)`, backed by Go channels
- `EventHandler` func type: `func(event AgentEvent) error`

**Messages:**
- `Message` struct: `Role`, `Content`, `ToolCalls []ToolCall`, `ToolCallID`, `Name`, `Images []ImageContent`
- `ToolCall` struct: `ID`, `Name`, `Arguments json.RawMessage`

#### config

Configuration management with JSONC parsing and encrypted key storage.

- `OrangeConfig` struct: all configuration fields
- `ConfigManager`: `Load(path)`, `Save(path)`, `Get(key)`, `Set(key, value)`
- JSONC parsing: strip comments before `json.Unmarshal`
- Encrypted storage: AES-GCM for API keys, stored at `~/.orangecoding/keys.enc`
- Config file: `~/.orangecoding/config.json`

#### audit

Tamper-proof audit logging with SHA-256 hash chain.

- `AuditEntry` struct: `Timestamp`, `Action`, `AgentID`, `Details`, `PrevHash`, `Hash`
- `AuditLog`: `Append(entry)`, `Verify() error`, `GetEntries(from, to) []AuditEntry`
- Hash chain: `entry.Hash = SHA256(entry.PrevHash + entry.Action + entry.Timestamp + entry.Details)`
- Storage: bbolt bucket keyed by timestamp

#### invariant

Runtime invariant framework for guards, checkpoints, and rollback.

- `Invariant` interface: `Name() string`, `Check(ctx context.Context) error`
- `Guard`: runs invariants before operations, blocks on violation
- `Checkpoint`: saves serializable state snapshots
- `Rollback`: restores from checkpoint
- `SelfHealingPolicy`: automated fix attempts with configurable retry
- `InvariantEngine`: orchestrates check/checkpoint/rollback cycle

### Layer 1: Service

#### ai

AI provider adapters with streaming, fallback, and model routing.

**Interfaces:**
```go
type AiProvider interface {
    Complete(ctx context.Context, req *CompletionRequest) (*CompletionResponse, error)
    Stream(ctx context.Context, req *CompletionRequest) (<-chan StreamChunk, error)
    Name() string
}
```

**Components:**
- `ProviderFactory`: creates provider instances from config
- `FallbackChain`: provider failover with per-provider cooldown (exponential backoff)
- `ModelRouter`: category-based routing (8 categories: coding, planning, review, etc.)
- `RoutingRule`: maps intent category → provider + model
- SSE stream parser: `bufio.Scanner` based, parses `data:` lines

**Providers:**
- OpenAI-compatible (GPT-4, GPT-4o, etc.)
- Anthropic (Claude series)
- DeepSeek
- Qianwen/Tongyi (通义千问)
- Wenxin/Baidu ERNIE (文心一言)

Each provider implements `AiProvider`, handles API-specific request/response formats, and supports tool_use/function_calling.

#### session

Session management with JSONL storage and branching.

- `Session` struct: `ID`, `Messages`, `Metadata`, `TokenUsage`, `CreatedAt`, `UpdatedAt`, `ParentID`
- `SessionManager`: `Create()`, `Get(id)`, `Update(id, session)`, `Delete(id)`, `List()`
- JSONL storage: one message per line, append-only for write, full read for load
- `SessionTree`: parent-child branching via `ParentID`, supports fork/merge
- `BlobStore`: content-addressable storage (SHA-256 → content), deduplicates large payloads

#### mcp

Model Context Protocol (JSON-RPC 2.0) client and server.

- `McpClient`: connects to external MCP servers, discovers tools, calls them
- `McpServer`: exposes local tools to external MCP clients
- `Transport` interface: `StdioTransport`, `HTTPTransport`
- JSON-RPC 2.0: `jsonrpc.Request`, `jsonrpc.Response`, `jsonrpc.Notification`
- Bidirectional communication for tool discovery and execution

#### tools

22+ tool implementations with permission system and security policies.

**Core interface:**
```go
type Tool interface {
    Name() string
    Description() string
    Parameters() json.RawMessage
    Execute(ctx context.Context, input json.RawMessage) (*ToolResult, error)
}
```

**Components:**
- `ToolRegistry`: `Register(tool)`, `Get(name)`, `List()`, `CreateDefaultRegistry()`
- Permission system: 5 permission types (allow, deny, ask, auto-approve, conditional), 3 control levels
- `PathValidator`: prevents path traversal, restricts to allowed directories
- `SecurityPolicy`: command allowlist/denylist for bash tool
- `BatchPartition`: concurrent-safe batch tool execution with result aggregation

**Tool list:**
bash, read_file, write_file, edit_file, grep, find, glob, python, browser, ssh, lsp, web_search, task_create, task_update, task_list, agent_dispatch, memory_write, memory_read, and others.

### Layer 2: Engine

#### agent

Core agent engine — the largest and most complex module.

**Agent interface:**
```go
type Agent interface {
    ID() AgentId
    Role() AgentRole
    Run(ctx context.Context, task string) error
    Stop() error
    Status() AgentStatus
}
```

**Core agent loop:**
1. Build `CompletionRequest` from conversation context + system prompt
2. Call `provider.Stream()` to get streaming response
3. Parse response chunks, extract tool calls
4. Execute tool calls via `executor.ExecuteBatch()`
5. Append tool results to conversation history
6. Check termination condition (no more tool calls, max turns, user stop)
7. Repeat from step 1 if not terminated

**Sub-agents (11 types):**
- Sisyphus: main general-purpose agent
- Hephaestus: tool error repair
- Prometheus: planning and decomposition
- Atlas: plan execution
- Oracle: question answering
- Librarian: knowledge management
- Explore: codebase exploration
- Metis: wisdom and judgment
- Momus: code review and criticism
- Junior: simple tasks
- Multimodal: image/multi-modal tasks

**Key subsystems:**
- `IntentGate`: classifies user intent into categories for model routing
- `ModelRouter`: routes to optimal provider+model based on intent category
- `ForkAgent`: clones parent context, spawns sub-agent with restricted tools
- `TTSR` (Regex-Triggered Streaming Rule Injection): injects rules into streaming based on regex patterns
- `Compaction`: compresses conversation history when approaching context limits
- `Memory`: persistent memory storage and recall across sessions
- `AutoDream`: consolidation of memories during idle periods
- `Hooks`: 40+ hook points for extensibility (pre/post tool, pre/post sampling, etc.)
- `Skills`: 6 built-in skills (composable tool+prompt bundles)

**Workflows:**
- UltraWork: fully autonomous mode with budget tracking
- Planning: Prometheus decomposes complex tasks into plans
- Execution: Atlas executes plans step by step
- Boulder: recovery workflow when agent gets stuck

**Concurrency model:**
- Go: one goroutine per agent, communicate via channels
- `context.Context` for cancellation propagation
- `sync.Mutex` for shared state within agent
- Goroutine-per-agent is lighter than tokio task, natural fit for mesh coordination

#### mesh

Multi-agent coordination layer.

- `MessageBus`: pub/sub for inter-agent messages, topic-based routing
- `TaskOrchestrator`: DAG-based task scheduling, respects dependency ordering
- `AgentRegistry`: agent discovery and capability lookup
- `Negotiator`: inter-agent task handoff with context transfer
- `BuddyObserver`: async reaction pattern, monitors agent events and triggers actions

### Layer 3: Interface

#### tui

Terminal UI using Bubble Tea (Elm Architecture).

**Architecture (Bubble Tea Model-View-Update):**
- `Model` struct: holds all TUI state (conversation, input, sidebar, status)
- `Init()` tea.Cmd: initial commands (load sessions, start agent)
- `Update(msg tea.Msg)` (tea.Model, tea.Cmd): handle keyboard, mouse, agent events
- `View()` string: render terminal output

**Components:**
- Main chat view: markdown-rendered messages
- Input area: multi-line editor
- Sidebar: session list, agent status
- Status bar: mode indicator, token usage, connection status
- Theme system: light/dark/custom color modes

**Libraries:**
- `github.com/charmbracelet/bubbletea` — TUI framework
- `github.com/charmbracelet/lipgloss` — styling
- `github.com/charmbracelet/glamour` — markdown rendering
- `github.com/charmbracelet/bubbles` — pre-built components (viewport, textinput, spinner)

#### control-protocol

Shared message types for control plane communication.

- `ClientCommand` interface: commands from web UI to server
- `ServerEvent` interface: events from server to web UI
- Concrete types: `SendTaskCommand`, `ApproveCommand`, `CancelCommand`, `TaskUpdateEvent`, `ToolCallEvent`, `ApprovalRequestEvent`
- JSON serialization for wire format

#### control-server

HTTP + WebSocket server for web-based control.

- Gin router with REST endpoints + WebSocket upgrade
- REST API: session CRUD, agent start/stop/status, config management
- WebSocket: real-time event streaming, approval flow
- Middleware: authentication, CORS, request logging
- Default bind: `127.0.0.1:3200`

#### worker

Agent worker runtime for managing agent lifecycles.

- `WorkerRuntime`: spawns and manages agent goroutines
- `AgentExecutor`: wraps agent execution with progress reporting
- Communicates with control-server via Go channels
- Supports concurrent agent execution with resource limits

#### cli

Command-line entry point using Cobra.

**Commands:**
- `orangecoding` (root) → defaults to `launch`
- `orangecoding launch [-p prompt]` — start agent (TUI, text, or single-shot)
- `orangecoding init` — initialize project config
- `orangecoding config [get|set key value]` — manage config
- `orangecoding status` — show system status
- `orangecoding serve` — start control server
- `orangecoding version` — show version

**Slash commands (in TUI/text mode):**
- `/model`, `/mode`, `/think`, `/plan`, `/clear`, `/help`, `/quit`

**Output:** single `orangecoding` binary

## Go-Specific Design Decisions

### Error Handling
- Custom `OrangeError` type with `Kind` and `Cause` fields
- `errors.Is()` and `errors.As()` for error matching
- `fmt.Errorf("...: %w", err)` for wrapping
- No panic/recover for flow control

### Concurrency
- Goroutines instead of async/await
- Channels for communication (agent events, tool results, stream chunks)
- `context.Context` for cancellation and timeout
- `sync.Mutex` / `sync.RWMutex` for shared mutable state
- `sync.WaitGroup` for goroutine coordination
- `sync.Map` for concurrent-safe maps

### Interface Design
- Small, focused interfaces (1-3 methods)
- Accept interfaces, return structs
- Mock-friendly for testing

### Testing
- `testing` package + standard `go test`
- Table-driven tests for comprehensive coverage
- Interface mocking via generated mocks or hand-written test doubles
- Integration tests in `tests/` directory

### Code Organization
- Each module is a Go module with its own `go.mod`
- `internal/` packages within modules for unexported implementation details
- Package names follow Go conventions (lowercase, no underscores)
- English comments and doc comments

## Build & Release

### Build
```bash
go work sync                    # Sync workspace
go build ./modules/cli          # Build CLI binary
go test ./...                   # Run all tests
```

### Cross-Compilation
```bash
GOOS=linux GOARCH=amd64 go build -o orangecoding-linux-amd64 ./modules/cli
GOOS=linux GOARCH=arm64 go build -o orangecoding-linux-arm64 ./modules/cli
GOOS=darwin GOARCH=amd64 go build -o orangecoding-darwin-amd64 ./modules/cli
GOOS=darwin GOARCH=arm64 go build -o orangecoding-darwin-arm64 ./modules/cli
```

### CI/CD
- GitHub Actions workflow: test → build (4 targets) → release on `v*` tags
- Reuse the same release pattern as the Rust version

## Scope

All 15 Rust crates will be fully rewritten with feature parity. No subsystems are excluded.
