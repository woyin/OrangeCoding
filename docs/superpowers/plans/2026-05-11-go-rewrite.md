# OrangeCoding Go Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite OrangeCoding from Rust to Go with full feature parity across 15 modules, 22+ tools, 5 AI providers, and 11 sub-agents.

**Architecture:** Go workspace (`go.work`) with 15 modules organized in 4 layers: Base (core/config/audit/invariant), Service (ai/session/mcp/tools), Engine (agent/mesh), Interface (tui/control-protocol/control-server/worker/cli). Each module is a Go module with its own `go.mod`. Modules communicate through small interfaces defined in the `core` module.

**Tech Stack:** Go 1.22+, goroutines+channels, cobra (CLI), bubbletea (TUI), gin (HTTP), gorilla/websocket, bbolt (embedded DB), slog (logging), `github.com/google/uuid`.

---

## Phase 0: Workspace Setup

### Task 1: Branch and Go Workspace

**Files:**
- Create: `go.work`
- Create: `modules/core/go.mod`
- Create: `modules/config/go.mod`
- Create: `modules/audit/go.mod`
- Create: `modules/invariant/go.mod`
- Create: `modules/ai/go.mod`
- Create: `modules/session/go.mod`
- Create: `modules/mcp/go.mod`
- Create: `modules/tools/go.mod`
- Create: `modules/agent/go.mod`
- Create: `modules/mesh/go.mod`
- Create: `modules/tui/go.mod`
- Create: `modules/control-protocol/go.mod`
- Create: `modules/control-server/go.mod`
- Create: `modules/worker/go.mod`
- Create: `modules/cli/go.mod`

- [ ] **Step 1: Create the Go rewrite branch**

```bash
git checkout -b go-rewrite
```

- [ ] **Step 2: Create directory structure for all 15 modules**

```bash
mkdir -p modules/{core,config,audit,invariant,ai,session,mcp,tools,agent,mesh,tui,control-protocol,control-server,worker,cli}
```

- [ ] **Step 3: Create `go.work` file**

```go
// go.work
go 1.22

use (
    ./modules/core
    ./modules/config
    ./modules/audit
    ./modules/invariant
    ./modules/ai
    ./modules/session
    ./modules/mcp
    ./modules/tools
    ./modules/agent
    ./modules/mesh
    ./modules/tui
    ./modules/control-protocol
    ./modules/control-server
    ./modules/worker
    ./modules/cli
)
```

- [ ] **Step 4: Create `go.mod` for each module**

Each module needs its own `go.mod`. The module path pattern is `github.com/woyin/OrangeCoding/modules/<name>`.

For **core** (no internal deps):
```
module github.com/woyin/OrangeCoding/modules/core
go 1.22
require github.com/google/uuid v1.6.0
```

For modules that depend on **core** (config, audit, invariant, ai, session, mcp, tools, agent, mesh, tui, control-protocol, control-server, worker, cli), add:
```
require github.com/woyin/OrangeCoding/modules/core v0.0.0
replace github.com/woyin/OrangeCoding/modules/core => ../core
```

For **cli** (depends on many modules):
```
module github.com/woyin/OrangeCoding/modules/cli
go 1.22
require (
    github.com/woyin/OrangeCoding/modules/core v0.0.0
    github.com/woyin/OrangeCoding/modules/config v0.0.0
    github.com/woyin/OrangeCoding/modules/agent v0.0.0
    github.com/woyin/OrangeCoding/modules/tui v0.0.0
    github.com/woyin/OrangeCoding/modules/mesh v0.0.0
    github.com/woyin/OrangeCoding/modules/control-server v0.0.0
    github.com/woyin/OrangeCoding/modules/worker v0.0.0
    github.com/spf13/cobra v1.8.0
)
replace (
    github.com/woyin/OrangeCoding/modules/core => ../core
    github.com/woyin/OrangeCoding/modules/config => ../config
    github.com/woyin/OrangeCoding/modules/agent => ../agent
    github.com/woyin/OrangeCoding/modules/tui => ../tui
    github.com/woyin/OrangeCoding/modules/mesh => ../mesh
    github.com/woyin/OrangeCoding/modules/control-server => ../control-server
    github.com/woyin/OrangeCoding/modules/worker => ../worker
)
```

Create all 15 `go.mod` files following this pattern, adding only the direct dependencies each module needs per the dependency graph in the spec.

- [ ] **Step 5: Create placeholder `doc.go` for each module**

Each module needs at least one Go file to be recognized by `go work sync`. Create a minimal `doc.go`:

```go
// Package <name> provides <description>.
package <name>
```

Package names: `core`, `config`, `audit`, `invariant`, `ai`, `session`, `mcp`, `tools`, `agent`, `mesh`, `tui`, `controlprotocol`, `controlserver`, `worker`, `cli`.

- [ ] **Step 6: Verify workspace compiles**

```bash
cd /Users/breestealth/Documents/DevelopmentRepository/OrangeCoding
go work sync
go build ./modules/...
```

Expected: All modules compile (empty packages).

- [ ] **Step 7: Commit**

```bash
git add go.work modules/
git commit -m "feat: initialize Go workspace with 15 module scaffolds"
```

---

## Phase 1: Layer 0 — Core Module

### Task 2: Core Types

**Files:**
- Create: `modules/core/types.go`
- Create: `modules/core/types_test.go`

- [ ] **Step 1: Write failing tests for core types**

```go
// modules/core/types_test.go
package core

import (
    "testing"
)

func TestAgentId(t *testing.T) {
    id := NewAgentId()
    s := id.String()
    if len(s) == 0 {
        t.Fatal("AgentId.String() should not be empty")
    }
    // Format: "agent-{uuid}"
    if s[:6] != "agent-" {
        t.Fatalf("AgentId format = %q, want prefix 'agent-'", s)
    }
    // Round-trip via ParseAgentId
    parsed, err := ParseAgentId(s)
    if err != nil {
        t.Fatalf("ParseAgentId(%q) error: %v", s, err)
    }
    if parsed != id {
        t.Fatalf("round-trip failed: %v != %v", parsed, id)
    }
}

func TestSessionId(t *testing.T) {
    id := NewSessionId()
    s := id.String()
    if s[:8] != "session-" {
        t.Fatalf("SessionId format = %q, want prefix 'session-'", s)
    }
    parsed, err := ParseSessionId(s)
    if err != nil {
        t.Fatalf("ParseSessionId(%q) error: %v", s, err)
    }
    if parsed != id {
        t.Fatalf("round-trip failed: %v != %v", parsed, id)
    }
}

func TestToolName(t *testing.T) {
    tn := NewToolName("bash")
    if tn.String() != "bash" {
        t.Fatalf("ToolName = %q, want %q", tn.String(), "bash")
    }
}

func TestTokenUsage(t *testing.T) {
    u := NewTokenUsage(100, 50)
    if u.PromptTokens != 100 || u.CompletionTokens != 50 || u.TotalTokens != 150 {
        t.Fatalf("TokenUsage = %+v, want {100, 50, 150}", u)
    }
    u2 := NewTokenUsage(200, 100)
    u.Accumulate(u2)
    if u.TotalTokens != 450 {
        t.Fatalf("after Accumulate, TotalTokens = %d, want 450", u.TotalTokens)
    }
    if u.IsEmpty() {
        t.Fatal("non-zero TokenUsage should not be empty")
    }
    zero := TokenUsage{}
    if !zero.IsEmpty() {
        t.Fatal("zero TokenUsage should be empty")
    }
}

func TestAgentRole(t *testing.T) {
    // Verify all roles are distinct
    roles := []AgentRole{RoleCoder, RoleReviewer, RolePlanner, RoleExecutor, RoleObserver}
    seen := make(map[AgentRole]bool)
    for _, r := range roles {
        if seen[r] {
            t.Fatalf("duplicate role: %d", r)
        }
        seen[r] = true
    }
}

func TestAgentStatus(t *testing.T) {
    if StatusCompleted.IsTerminal() != true {
        t.Fatal("StatusCompleted should be terminal")
    }
    if StatusFailed.IsTerminal() != true {
        t.Fatal("StatusFailed should be terminal")
    }
    if StatusRunning.IsTerminal() != false {
        t.Fatal("StatusRunning should not be terminal")
    }
    if StatusRunning.IsActive() != true {
        t.Fatal("StatusRunning should be active")
    }
    if StatusWaiting.IsActive() != true {
        t.Fatal("StatusWaiting should be active")
    }
    if StatusIdle.IsActive() != false {
        t.Fatal("StatusIdle should not be active")
    }
}

func TestRole(t *testing.T) {
    roles := []Role{RoleSystem, RoleUser, RoleAssistant, RoleTool}
    for _, r := range roles {
        s := r.String()
        if len(s) == 0 {
            t.Fatalf("Role(%d).String() is empty", r)
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/core && go test -run TestAgentId -v
```

Expected: FAIL (no types defined yet).

- [ ] **Step 3: Implement core types**

```go
// modules/core/types.go
package core

import (
    "encoding/json"
    "fmt"
    "github.com/google/uuid"
)

// AgentId uniquely identifies an agent instance.
type AgentId struct{ uuid.UUID }

func NewAgentId() AgentId       { return AgentId{uuid.New()} }
func (a AgentId) String() string { return "agent-" + a.UUID.String() }

func ParseAgentId(s string) (AgentId, error) {
    if len(s) < 6 || s[:6] != "agent-" {
        return AgentId{}, fmt.Errorf("invalid AgentId format: %q", s)
    }
    u, err := uuid.Parse(s[6:])
    if err != nil {
        return AgentId{}, fmt.Errorf("invalid AgentId UUID: %w", err)
    }
    return AgentId{u}, nil
}

// SessionId uniquely identifies a conversation session.
type SessionId struct{ uuid.UUID }

func NewSessionId() SessionId       { return SessionId{uuid.New()} }
func (s SessionId) String() string  { return "session-" + s.UUID.String() }

func ParseSessionId(s string) (SessionId, error) {
    if len(s) < 8 || s[:8] != "session-" {
        return SessionId{}, fmt.Errorf("invalid SessionId format: %q", s)
    }
    u, err := uuid.Parse(s[8:])
    if err != nil {
        return SessionId{}, fmt.Errorf("invalid SessionId UUID: %w", err)
    }
    return SessionId{u}, nil
}

// ToolName identifies a tool by name.
type ToolName struct{ name string }

func NewToolName(name string) ToolName { return ToolName{name: name} }
func (t ToolName) String() string       { return t.name }

// AgentRole represents the functional role of an agent.
type AgentRole int

const (
    RoleCoder    AgentRole = iota
    RoleReviewer
    RolePlanner
    RoleExecutor
    RoleObserver
)

var agentRoleNames = [...]string{"coder", "reviewer", "planner", "executor", "observer"}

func (r AgentRole) String() string {
    if int(r) < len(agentRoleNames) {
        return agentRoleNames[r]
    }
    return fmt.Sprintf("AgentRole(%d)", r)
}

func (r AgentRole) MarshalJSON() ([]byte, error) { return json.Marshal(r.String()) }

// AgentStatus represents the current state of an agent.
type AgentStatus int

const (
    StatusIdle     AgentStatus = iota
    StatusRunning
    StatusWaiting
    StatusCompleted
    StatusFailed
)

func (s AgentStatus) IsTerminal() bool { return s == StatusCompleted || s == StatusFailed }
func (s AgentStatus) IsActive() bool   { return s == StatusRunning || s == StatusWaiting }

var agentStatusNames = [...]string{"idle", "running", "waiting", "completed", "failed"}

func (s AgentStatus) String() string {
    if int(s) < len(agentStatusNames) {
        return agentStatusNames[s]
    }
    return fmt.Sprintf("AgentStatus(%d)", s)
}

// TokenUsage tracks token consumption.
type TokenUsage struct {
    PromptTokens     uint64 `json:"prompt_tokens"`
    CompletionTokens uint64 `json:"completion_tokens"`
    TotalTokens      uint64 `json:"total_tokens"`
}

func NewTokenUsage(prompt, completion uint64) TokenUsage {
    return TokenUsage{
        PromptTokens:     prompt,
        CompletionTokens: completion,
        TotalTokens:      prompt + completion,
    }
}

func (t *TokenUsage) Accumulate(other TokenUsage) {
    t.PromptTokens += other.PromptTokens
    t.CompletionTokens += other.CompletionTokens
    t.TotalTokens += other.TotalTokens
}

func (t TokenUsage) IsEmpty() bool { return t.TotalTokens == 0 }

// Role represents the role of a message sender.
type Role int

const (
    RoleSystem    Role = iota
    RoleUser
    RoleAssistant
    RoleTool
)

var roleNames = [...]string{"system", "user", "assistant", "tool"}

func (r Role) String() string {
    if int(r) < len(roleNames) {
        return roleNames[r]
    }
    return fmt.Sprintf("Role(%d)", r)
}

func (r Role) MarshalJSON() ([]byte, error) { return json.Marshal(r.String()) }

func (r *Role) UnmarshalJSON(data []byte) error {
    var s string
    if err := json.Unmarshal(data, &s); err != nil {
        return err
    }
    switch s {
    case "system":
        *r = RoleSystem
    case "user":
        *r = RoleUser
    case "assistant":
        *r = RoleAssistant
    case "tool":
        *r = RoleTool
    default:
        return fmt.Errorf("unknown role: %q", s)
    }
    return nil
}

// AgentCapability describes what an agent can do.
type AgentCapability struct {
    Name             string     `json:"name"`
    Description      string     `json:"description"`
    SupportedTools   []ToolName `json:"supported_tools"`
}

func (c AgentCapability) SupportsTool(name ToolName) bool {
    for _, t := range c.SupportedTools {
        if t.String() == name.String() {
            return true
        }
    }
    return false
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd modules/core && go test -v ./...
```

Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/core/types.go modules/core/types_test.go
git commit -m "feat(core): add AgentId, SessionId, ToolName, TokenUsage, Role, AgentRole, AgentStatus types"
```

---

### Task 3: Core Errors

**Files:**
- Create: `modules/core/error.go`
- Create: `modules/core/error_test.go`

- [ ] **Step 1: Write failing tests for error types**

```go
// modules/core/error_test.go
package core

import (
    "errors"
    "fmt"
    "testing"
)

func TestOrangeError(t *testing.T) {
    err := NewConfigError("missing api_key")
    if err.Kind() != ErrConfig {
        t.Fatalf("Kind = %v, want ErrConfig", err.Kind())
    }
    if !errors.Is(err, err) {
        t.Fatal("errors.Is should match itself")
    }
    if err.Error() != "config: missing api_key" {
        t.Fatalf("Error() = %q", err.Error())
    }
}

func TestOrangeErrorWrap(t *testing.T) {
    inner := fmt.Errorf("connection refused")
    err := WrapError(inner, ErrNetwork, "failed to reach provider")
    if err.Kind() != ErrNetwork {
        t.Fatalf("Kind = %v, want ErrNetwork", err.Kind())
    }
    if !errors.Is(err, inner) {
        t.Fatal("wrapped error should match inner via errors.Is")
    }
    unwrapped := errors.Unwrap(err)
    if unwrapped != inner {
        t.Fatal("Unwrap should return inner error")
    }
}

func TestOrangeErrorIsRetryable(t *testing.T) {
    tests := []struct {
        kind      ErrorKind
        retryable bool
    }{
        {ErrNetwork, true},
        {ErrProvider, true},
        {ErrConfig, false},
        {ErrTool, false},
        {ErrAgent, false},
        {ErrIO, false},
    }
    for _, tt := range tests {
        err := &OrangeError{kind: tt.kind, message: "test"}
        if got := err.IsRetryable(); got != tt.retryable {
            t.Errorf("IsRetryable(%v) = %v, want %v", tt.kind, got, tt.retryable)
        }
    }
}

func TestOrangeErrorConvenienceConstructors(t *testing.T) {
    err := NewNetworkError("timeout")
    if err.Kind() != ErrNetwork {
        t.Fatal("NewNetworkError should set ErrNetwork")
    }
    err = NewToolError("bash", "permission denied")
    if err.Kind() != ErrTool {
        t.Fatal("NewToolError should set ErrTool")
    }
    err = NewAgentError("agent-123", "loop detected")
    if err.Kind() != ErrAgent {
        t.Fatal("NewAgentError should set ErrAgent")
    }
    err = NewAuthError("invalid token")
    if err.Kind() != ErrAuth {
        t.Fatal("NewAuthError should set ErrAuth")
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/core && go test -run TestOrangeError -v
```

Expected: FAIL.

- [ ] **Step 3: Implement error types**

```go
// modules/core/error.go
package core

import (
    "errors"
    "fmt"
)

// ErrorKind categorizes errors by domain.
type ErrorKind int

const (
    ErrConfig ErrorKind = iota
    ErrIO
    ErrNetwork
    ErrProvider
    ErrAgent
    ErrTool
    ErrProtocol
    ErrSerialization
    ErrAuth
    ErrInternal
)

var errorKindNames = [...]string{
    "config", "io", "network", "provider", "agent",
    "tool", "protocol", "serialization", "auth", "internal",
}

func (k ErrorKind) String() string {
    if int(k) < len(errorKindNames) {
        return errorKindNames[k]
    }
    return fmt.Sprintf("ErrorKind(%d)", k)
}

// OrangeError is the standard error type for the OrangeCoding system.
type OrangeError struct {
    kind    ErrorKind
    message string
    cause   error
}

func (e *OrangeError) Error() string {
    if e.cause != nil {
        return fmt.Sprintf("%s: %s: %v", e.kind, e.message, e.cause)
    }
    return fmt.Sprintf("%s: %s", e.kind, e.message)
}

func (e *OrangeError) Unwrap() error { return e.cause }
func (e *OrangeError) Kind() ErrorKind { return e.kind }

// IsRetryable returns true if the error may succeed on retry.
func (e *OrangeError) IsRetryable() bool {
    return e.kind == ErrNetwork || e.kind == ErrProvider
}

// WrapError creates a new OrangeError wrapping an existing error.
func WrapError(cause error, kind ErrorKind, message string) *OrangeError {
    return &OrangeError{kind: kind, message: message, cause: cause}
}

// Convenience constructors for each error kind.
func NewConfigError(msg string) *OrangeError   { return &OrangeError{kind: ErrConfig, message: msg} }
func NewIOError(msg string) *OrangeError        { return &OrangeError{kind: ErrIO, message: msg} }
func NewNetworkError(msg string) *OrangeError   { return &OrangeError{kind: ErrNetwork, message: msg} }
func NewProviderError(msg string) *OrangeError  { return &OrangeError{kind: ErrProvider, message: msg} }
func NewProtocolError(msg string) *OrangeError  { return &OrangeError{kind: ErrProtocol, message: msg} }
func NewSerializationError(msg string) *OrangeError { return &OrangeError{kind: ErrSerialization, message: msg} }
func NewAuthError(msg string) *OrangeError      { return &OrangeError{kind: ErrAuth, message: msg} }
func NewInternalError(msg string) *OrangeError  { return &OrangeError{kind: ErrInternal, message: msg} }

func NewToolError(toolName, msg string) *OrangeError {
    return &OrangeError{kind: ErrTool, message: fmt.Sprintf("[%s] %s", toolName, msg)}
}

func NewAgentError(agentId, msg string) *OrangeError {
    return &OrangeError{kind: ErrAgent, message: fmt.Sprintf("[%s] %s", agentId, msg)}
}

// Result is a convenience type alias.
type Result[T any] struct {
    Value T
    Err   error
}

// Ensure OrangeError implements the error interface.
var _ error = (*OrangeError)(nil)

// Ensure errors.Is/As work with OrangeError.
func init() {
    // OrangeError already satisfies error via Error() method.
    // errors.Is/As work via Unwrap() method.
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd modules/core && go test -run TestOrangeError -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/core/error.go modules/core/error_test.go
git commit -m "feat(core): add OrangeError, ErrorKind, and error constructors"
```

---

### Task 4: Core Messages

**Files:**
- Create: `modules/core/message.go`
- Create: `modules/core/message_test.go`

- [ ] **Step 1: Write failing tests for messages**

```go
// modules/core/message_test.go
package core

import (
    "encoding/json"
    "testing"
)

func TestMessageConstructors(t *testing.T) {
    sys := NewSystemMessage("You are a helpful assistant.")
    if sys.Role != RoleSystem {
        t.Fatalf("role = %v, want RoleSystem", sys.Role)
    }
    if sys.Content != "You are a helpful assistant." {
        t.Fatalf("content = %q", sys.Content)
    }

    usr := NewUserMessage("Hello")
    if usr.Role != RoleUser {
        t.Fatalf("role = %v, want RoleUser", usr.Role)
    }

    asst := NewAssistantMessage("Hi there")
    if asst.Role != RoleAssistant {
        t.Fatalf("role = %v, want RoleAssistant", asst.Role)
    }
}

func TestMessageWithToolCalls(t *testing.T) {
    tc := ToolCall{ID: "call-1", FunctionName: "bash", Arguments: json.RawMessage(`{"command":"ls"}`)}
    msg := NewAssistantMessageWithToolCalls("", []ToolCall{tc})
    if len(msg.ToolCalls) != 1 {
        t.Fatalf("ToolCalls len = %d, want 1", len(msg.ToolCalls))
    }
    if !msg.HasToolCalls() {
        t.Fatal("HasToolCalls() should return true")
    }
}

func TestToolResultMessage(t *testing.T) {
    msg := NewToolResultMessage("call-1", "output text", false)
    if msg.Role != RoleTool {
        t.Fatalf("role = %v, want RoleTool", msg.Role)
    }
    if msg.ToolCallID != "call-1" {
        t.Fatalf("ToolCallID = %q, want %q", msg.ToolCallID, "call-1")
    }
}

func TestConversation(t *testing.T) {
    conv := NewConversation()
    if conv.Len() != 0 {
        t.Fatal("new conversation should be empty")
    }
    if !conv.IsEmpty() {
        t.Fatal("new conversation should report IsEmpty")
    }

    conv.AddMessage(NewSystemMessage("system prompt"))
    conv.AddMessage(NewUserMessage("hello"))
    conv.AddMessage(NewAssistantMessage("hi"))
    if conv.Len() != 3 {
        t.Fatalf("len = %d, want 3", conv.Len())
    }
    if conv.IsEmpty() {
        t.Fatal("conversation with 3 messages should not be empty")
    }

    // System prompt
    sp := conv.SystemPrompt()
    if sp == nil || *sp != "system prompt" {
        t.Fatalf("SystemPrompt = %v, want 'system prompt'", sp)
    }

    // Last message
    last := conv.LastMessage()
    if last == nil || last.Content != "hi" {
        t.Fatal("LastMessage should be the assistant message")
    }

    // Clear
    conv.Clear()
    if conv.Len() != 0 {
        t.Fatal("after Clear, len should be 0")
    }
}

func TestConversationWithSystemPrompt(t *testing.T) {
    conv := NewConversationWithSystemPrompt("You are helpful.")
    if conv.Len() != 1 {
        t.Fatalf("len = %d, want 1", conv.Len())
    }
    sp := conv.SystemPrompt()
    if sp == nil || *sp != "You are helpful." {
        t.Fatalf("SystemPrompt = %v", sp)
    }
}

func TestConversationPendingToolCalls(t *testing.T) {
    conv := NewConversation()
    tc := ToolCall{ID: "call-1", FunctionName: "bash", Arguments: json.RawMessage(`{}`)}
    conv.AddMessage(NewAssistantMessageWithToolCalls("", []ToolCall{tc}))
    pending := conv.PendingToolCalls()
    if len(pending) != 1 {
        t.Fatalf("pending = %d, want 1", len(pending))
    }
}

func TestConversationTokenEstimate(t *testing.T) {
    conv := NewConversation()
    conv.AddMessage(NewUserMessage("hello world"))
    estimate := conv.TokenEstimate()
    if estimate == 0 {
        t.Fatal("TokenEstimate should be > 0 for non-empty conversation")
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/core && go test -run TestMessage -v
```

Expected: FAIL.

- [ ] **Step 3: Implement message types**

```go
// modules/core/message.go
package core

import (
    "encoding/json"
    "time"
)

// Message represents a single message in a conversation.
type Message struct {
    Role       Role            `json:"role"`
    Content    string          `json:"content,omitempty"`
    Name       string          `json:"name,omitempty"`
    ToolCalls  []ToolCall      `json:"tool_calls,omitempty"`
    ToolCallID string          `json:"tool_call_id,omitempty"`
    CreatedAt  time.Time       `json:"created_at"`
}

func NewSystemMessage(content string) Message {
    return Message{Role: RoleSystem, Content: content, CreatedAt: time.Now().UTC()}
}

func NewUserMessage(content string) Message {
    return Message{Role: RoleUser, Content: content, CreatedAt: time.Now().UTC()}
}

func NewAssistantMessage(content string) Message {
    return Message{Role: RoleAssistant, Content: content, CreatedAt: time.Now().UTC()}
}

func NewAssistantMessageWithToolCalls(content string, toolCalls []ToolCall) Message {
    return Message{Role: RoleAssistant, Content: content, ToolCalls: toolCalls, CreatedAt: time.Now().UTC()}
}

func NewToolResultMessage(toolCallID, content string, isError bool) Message {
    return Message{Role: RoleTool, Content: content, ToolCallID: toolCallID, CreatedAt: time.Now().UTC()}
}

func (m Message) HasToolCalls() bool { return len(m.ToolCalls) > 0 }

// ToolCall represents a request from the AI to invoke a tool.
type ToolCall struct {
    ID           string          `json:"id"`
    FunctionName string          `json:"function_name"`
    Arguments    json.RawMessage `json:"arguments"`
}

// ToolResult represents the result of executing a tool.
type ToolResult struct {
    ToolCallID string `json:"tool_call_id"`
    Content    string `json:"content"`
    IsError    bool   `json:"is_error"`
}

func NewToolResultSuccess(toolCallID, content string) ToolResult {
    return ToolResult{ToolCallID: toolCallID, Content: content}
}

func NewToolResultError(toolCallID, content string) ToolResult {
    return ToolResult{ToolCallID: toolCallID, Content: content, IsError: true}
}

func (r ToolResult) ToMessage() Message {
    return NewToolResultMessage(r.ToolCallID, r.Content, r.IsError)
}

// Conversation manages an ordered sequence of messages.
type Conversation struct {
    messages []Message
}

func NewConversation() *Conversation { return &Conversation{} }

func NewConversationWithSystemPrompt(prompt string) *Conversation {
    c := &Conversation{}
    c.AddMessage(NewSystemMessage(prompt))
    return c
}

func (c *Conversation) AddMessage(msg Message) {
    c.messages = append(c.messages, msg)
}

func (c *Conversation) Messages() []Message { return c.messages }
func (c *Conversation) Len() int            { return len(c.messages) }
func (c *Conversation) IsEmpty() bool       { return len(c.messages) == 0 }

func (c *Conversation) SystemPrompt() *string {
    if len(c.messages) > 0 && c.messages[0].Role == RoleSystem {
        s := c.messages[0].Content
        return &s
    }
    return nil
}

func (c *Conversation) LastMessage() *Message {
    if len(c.messages) == 0 {
        return nil
    }
    return &c.messages[len(c.messages)-1]
}

func (c *Conversation) LastAssistantMessage() *Message {
    for i := len(c.messages) - 1; i >= 0; i-- {
        if c.messages[i].Role == RoleAssistant {
            return &c.messages[i]
        }
    }
    return nil
}

func (c *Conversation) PendingToolCalls() []ToolCall {
    var result []ToolCall
    for i := len(c.messages) - 1; i >= 0; i-- {
        if c.messages[i].Role == RoleAssistant && c.messages[i].HasToolCalls() {
            result = append(result, c.messages[i].ToolCalls...)
            break
        }
    }
    return result
}

func (c *Conversation) Clear() { c.messages = nil }

// TokenEstimate returns a rough token count (1 token ≈ 4 chars).
func (c *Conversation) TokenEstimate() int {
    total := 0
    for _, m := range c.messages {
        total += len(m.Content)
        for _, tc := range m.ToolCalls {
            total += len(tc.Arguments)
        }
    }
    return total / 4
}
```

- [ ] **Step 4: Run tests**

```bash
cd modules/core && go test -run TestMessage -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/core/message.go modules/core/message_test.go
git commit -m "feat(core): add Message, ToolCall, ToolResult, Conversation types"
```

---

### Task 5: Core Events

**Files:**
- Create: `modules/core/event.go`
- Create: `modules/core/event_test.go`

- [ ] **Step 1: Write failing tests for events**

```go
// modules/core/event_test.go
package core

import (
    "context"
    "sync"
    "testing"
    "time"
)

func TestAgentEventTypes(t *testing.T) {
    id := NewAgentId()
    sid := NewSessionId()

    events := []AgentEvent{
        NewStartedEvent(id, sid),
        NewCompletedEvent(id, sid, "done"),
        NewErrorEvent(id, sid, "something failed"),
        NewStreamChunkEvent(id, sid, "hello"),
        NewTokenUsageUpdatedEvent(id, sid, NewTokenUsage(100, 50)),
    }

    expectedTypes := []string{"started", "completed", "error", "stream_chunk", "token_usage_updated"}
    for i, ev := range events {
        if ev.EventType() != expectedTypes[i] {
            t.Errorf("event[%d].EventType() = %q, want %q", i, ev.EventType(), expectedTypes[i])
        }
        if ev.AgentID() != id {
            t.Errorf("event[%d].AgentID() mismatch", i)
        }
        if ev.SessionID() != sid {
            t.Errorf("event[%d].SessionID() mismatch", i)
        }
    }
}

func TestEventBus(t *testing.T) {
    bus := NewEventBus(10)
    ctx := context.Background()

    var received []AgentEvent
    var mu sync.Mutex

    handler := EventHandlerFunc{
        name: "test-handler",
        fn: func(ev AgentEvent) error {
            mu.Lock()
            received = append(received, ev)
            mu.Unlock()
            return nil
        },
    }

    subID, err := bus.Subscribe(handler)
    if err != nil {
        t.Fatalf("Subscribe error: %v", err)
    }

    id := NewAgentId()
    sid := NewSessionId()
    bus.Publish(ctx, NewStartedEvent(id, sid))
    bus.Publish(ctx, NewCompletedEvent(id, sid, "done"))

    // Give handler goroutine time to process
    time.Sleep(50 * time.Millisecond)

    mu.Lock()
    count := len(received)
    mu.Unlock()

    if count != 2 {
        t.Fatalf("received %d events, want 2", count)
    }

    // Unsubscribe
    bus.Unsubscribe(subID)
    bus.Publish(ctx, NewErrorEvent(id, sid, "test"))
    time.Sleep(50 * time.Millisecond)

    mu.Lock()
    count = len(received)
    mu.Unlock()

    if count != 2 {
        t.Fatalf("after unsubscribe, received %d events, want 2", count)
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/core && go test -run TestAgentEvent -v
```

Expected: FAIL.

- [ ] **Step 3: Implement events**

```go
// modules/core/event.go
package core

import (
    "context"
    "sync"
    "time"
)

// AgentEvent is the interface for all agent events.
type AgentEvent interface {
    EventType() string
    AgentID() AgentId
    SessionID() SessionId
    Timestamp() time.Time
}

// BaseEvent provides common fields for all events.
type BaseEvent struct {
    Type      string    `json:"type"`
    Agent     AgentId   `json:"agent_id"`
    Session   SessionId `json:"session_id"`
    Time      time.Time `json:"timestamp"`
}

func (e BaseEvent) EventType() string    { return e.Type }
func (e BaseEvent) AgentID() AgentId     { return e.Agent }
func (e BaseEvent) SessionID() SessionId { return e.Session }
func (e BaseEvent) Timestamp() time.Time { return e.Time }

// Concrete event types.

type StartedEvent struct{ BaseEvent }
type CompletedEvent struct {
    BaseEvent
    Summary string `json:"summary"`
}
type MessageReceivedEvent struct {
    BaseEvent
    ContentPreview string `json:"content_preview"`
}
type ToolCallRequestedEvent struct {
    BaseEvent
    ToolCall ToolCall `json:"tool_call"`
}
type ToolCallCompletedEvent struct {
    BaseEvent
    ToolName   ToolName `json:"tool_name"`
    Success    bool     `json:"success"`
    DurationMs uint64   `json:"duration_ms"`
}
type TokenUsageUpdatedEvent struct {
    BaseEvent
    Usage TokenUsage `json:"usage"`
}
type StreamChunkEvent struct {
    BaseEvent
    Content string `json:"content"`
}
type ErrorEvent struct {
    BaseEvent
    ErrorMessage string `json:"error_message"`
}
type GoalPhaseChangedEvent struct {
    BaseEvent
    Phase string `json:"phase"`
    Cycle uint32 `json:"cycle"`
}
type GoalTaskCompletedEvent struct {
    BaseEvent
    TaskID          string `json:"task_id"`
    TaskDescription string `json:"task_description"`
    Success         bool   `json:"success"`
}
type GoalCycleCompleteEvent struct {
    BaseEvent
    Cycle              uint32 `json:"cycle"`
    TasksCompleted     uint32 `json:"tasks_completed"`
    TasksFailed        uint32 `json:"tasks_failed"`
    VerificationPassed bool   `json:"verification_passed"`
}

// Constructors for each event type.

func newBase(eventType string, agentID AgentId, sessionID SessionId) BaseEvent {
    return BaseEvent{Type: eventType, Agent: agentID, Session: sessionID, Time: time.Now().UTC()}
}

func NewStartedEvent(agentID AgentId, sessionID SessionId) StartedEvent {
    return StartedEvent{BaseEvent: newBase("started", agentID, sessionID)}
}

func NewCompletedEvent(agentID AgentId, sessionID SessionId, summary string) CompletedEvent {
    return CompletedEvent{BaseEvent: newBase("completed", agentID, sessionID), Summary: summary}
}

func NewMessageReceivedEvent(agentID AgentId, sessionID SessionId, preview string) MessageReceivedEvent {
    return MessageReceivedEvent{BaseEvent: newBase("message_received", agentID, sessionID), ContentPreview: preview}
}

func NewToolCallRequestedEvent(agentID AgentId, sessionID SessionId, tc ToolCall) ToolCallRequestedEvent {
    return ToolCallRequestedEvent{BaseEvent: newBase("tool_call_requested", agentID, sessionID), ToolCall: tc}
}

func NewToolCallCompletedEvent(agentID AgentId, sessionID SessionId, name ToolName, success bool, durMs uint64) ToolCallCompletedEvent {
    return ToolCallCompletedEvent{BaseEvent: newBase("tool_call_completed", agentID, sessionID), ToolName: name, Success: success, DurationMs: durMs}
}

func NewTokenUsageUpdatedEvent(agentID AgentId, sessionID SessionId, usage TokenUsage) TokenUsageUpdatedEvent {
    return TokenUsageUpdatedEvent{BaseEvent: newBase("token_usage_updated", agentID, sessionID), Usage: usage}
}

func NewStreamChunkEvent(agentID AgentId, sessionID SessionId, content string) StreamChunkEvent {
    return StreamChunkEvent{BaseEvent: newBase("stream_chunk", agentID, sessionID), Content: content}
}

func NewErrorEvent(agentID AgentId, sessionID SessionId, errMsg string) ErrorEvent {
    return ErrorEvent{BaseEvent: newBase("error", agentID, sessionID), ErrorMessage: errMsg}
}

func NewGoalPhaseChangedEvent(agentID AgentId, sessionID SessionId, phase string, cycle uint32) GoalPhaseChangedEvent {
    return GoalPhaseChangedEvent{BaseEvent: newBase("goal_phase_changed", agentID, sessionID), Phase: phase, Cycle: cycle}
}

func NewGoalTaskCompletedEvent(agentID AgentId, sessionID SessionId, taskID, desc string, success bool) GoalTaskCompletedEvent {
    return GoalTaskCompletedEvent{BaseEvent: newBase("goal_task_completed", agentID, sessionID), TaskID: taskID, TaskDescription: desc, Success: success}
}

func NewGoalCycleCompleteEvent(agentID AgentId, sessionID SessionId, cycle, completed, failed uint32, verified bool) GoalCycleCompleteEvent {
    return GoalCycleCompleteEvent{BaseEvent: newBase("goal_cycle_complete", agentID, sessionID), Cycle: cycle, TasksCompleted: completed, TasksFailed: failed, VerificationPassed: verified}
}

// EventHandler processes agent events.
type EventHandler interface {
    Handle(ev AgentEvent) error
    Name() string
}

// EventHandlerFunc is a function-based event handler.
type EventHandlerFunc struct {
    name string
    fn   func(AgentEvent) error
}

func (h EventHandlerFunc) Handle(ev AgentEvent) error { return h.fn(ev) }
func (h EventHandlerFunc) Name() string               { return h.name }

// EventBus distributes events to subscribers.
type EventBus struct {
    mu          sync.RWMutex
    subscribers map[string]EventHandler
    buffer      int
}

func NewEventBus(buffer int) *EventBus {
    return &EventBus{subscribers: make(map[string]EventHandler), buffer: buffer}
}

func (b *EventBus) Subscribe(handler EventHandler) (string, error) {
    b.mu.Lock()
    defer b.mu.Unlock()
    id := handler.Name()
    b.subscribers[id] = handler
    return id, nil
}

func (b *EventBus) Unsubscribe(id string) {
    b.mu.Lock()
    defer b.mu.Unlock()
    delete(b.subscribers, id)
}

func (b *EventBus) Publish(ctx context.Context, ev AgentEvent) {
    b.mu.RLock()
    defer b.mu.RUnlock()
    for _, handler := range b.subscribers {
        go func(h EventHandler) {
            _ = h.Handle(ev) // fire-and-forget; errors logged by handler
        }(handler)
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd modules/core && go test -run TestEvent -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/core/event.go modules/core/event_test.go
git commit -m "feat(core): add AgentEvent types, EventBus, EventHandler"
```

---

## Phase 2: Layer 0 — Config, Audit, Invariant

### Task 6: Config Module

**Files:**
- Create: `modules/config/config.go`
- Create: `modules/config/jsonc.go`
- Create: `modules/config/crypto.go`
- Create: `modules/config/config_test.go`

- [ ] **Step 1: Write failing tests for config**

```go
// modules/config/config_test.go
package config

import (
    "os"
    "path/filepath"
    "testing"
)

func TestOrangeConfigDefaults(t *testing.T) {
    cfg := DefaultConfig()
    if cfg.LogLevel != "info" {
        t.Fatalf("default LogLevel = %q, want %q", cfg.LogLevel, "info")
    }
    if cfg.ControlPort != 3200 {
        t.Fatalf("default ControlPort = %d, want 3200", cfg.ControlPort)
    }
}

func TestConfigManagerLoadSave(t *testing.T) {
    dir := t.TempDir()
    path := filepath.Join(dir, "config.json")

    cfg := DefaultConfig()
    cfg.LogLevel = "debug"
    cfg.DefaultProvider = "openai"

    mgr := NewConfigManager()
    if err := mgr.Save(path, cfg); err != nil {
        t.Fatalf("Save error: %v", err)
    }

    loaded, err := mgr.Load(path)
    if err != nil {
        t.Fatalf("Load error: %v", err)
    }
    if loaded.LogLevel != "debug" {
        t.Fatalf("loaded LogLevel = %q, want %q", loaded.LogLevel, "debug")
    }
    if loaded.DefaultProvider != "openai" {
        t.Fatalf("loaded DefaultProvider = %q, want %q", loaded.DefaultProvider, "openai")
    }
}

func TestJSONCParsing(t *testing.T) {
    input := `{
        // This is a comment
        "key": "value",
        /* block comment */
        "number": 42
    }`
    result, err := ParseJSONC(input)
    if err != nil {
        t.Fatalf("ParseJSONC error: %v", err)
    }
    if result["key"] != "value" {
        t.Fatalf("key = %v, want %q", result["key"], "value")
    }
    if result["number"] != float64(42) {
        t.Fatalf("number = %v, want 42", result["number"])
    }
}

func TestEncryptedStorage(t *testing.T) {
    key := []byte("0123456789abcdef0123456789abcdef") // 32 bytes for AES-256
    plaintext := "sk-test-api-key-12345"

    encrypted, err := Encrypt(key, []byte(plaintext))
    if err != nil {
        t.Fatalf("Encrypt error: %v", err)
    }
    if string(encrypted) == plaintext {
        t.Fatal("encrypted should differ from plaintext")
    }

    decrypted, err := Decrypt(key, encrypted)
    if err != nil {
        t.Fatalf("Decrypt error: %v", err)
    }
    if string(decrypted) != plaintext {
        t.Fatalf("decrypted = %q, want %q", string(decrypted), plaintext)
    }
}

func TestConfigManagerSetGet(t *testing.T) {
    dir := t.TempDir()
    path := filepath.Join(dir, "config.json")

    cfg := DefaultConfig()
    mgr := NewConfigManager()
    mgr.Save(path, cfg)

    if err := mgr.Set(path, "log_level", "warn"); err != nil {
        t.Fatalf("Set error: %v", err)
    }

    val, err := mgr.Get(path, "log_level")
    if err != nil {
        t.Fatalf("Get error: %v", err)
    }
    if val != "warn" {
        t.Fatalf("Get = %v, want %q", val, "warn")
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/config && go test -v
```

Expected: FAIL.

- [ ] **Step 3: Implement config module**

Implement three files:

**`modules/config/jsonc.go`** — JSONC parser that strips `//` and `/* */` comments before `json.Unmarshal`.

**`modules/config/crypto.go`** — AES-256-GCM encryption/decryption for API keys:
```go
package config

import (
    "crypto/aes"
    "crypto/cipher"
    "crypto/rand"
    "fmt"
    "io"
)

func Encrypt(key, plaintext []byte) ([]byte, error) {
    block, err := aes.NewCipher(key)
    if err != nil {
        return nil, fmt.Errorf("aes.NewCipher: %w", err)
    }
    gcm, err := cipher.NewGCM(block)
    if err != nil {
        return nil, fmt.Errorf("cipher.NewGCM: %w", err)
    }
    nonce := make([]byte, gcm.NonceSize())
    if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
        return nil, fmt.Errorf("generate nonce: %w", err)
    }
    return gcm.Seal(nonce, nonce, plaintext, nil), nil
}

func Decrypt(key, ciphertext []byte) ([]byte, error) {
    block, err := aes.NewCipher(key)
    if err != nil {
        return nil, fmt.Errorf("aes.NewCipher: %w", err)
    }
    gcm, err := cipher.NewGCM(block)
    if err != nil {
        return nil, fmt.Errorf("cipher.NewGCM: %w", err)
    }
    nonceSize := gcm.NonceSize()
    if len(ciphertext) < nonceSize {
        return nil, fmt.Errorf("ciphertext too short")
    }
    nonce, ciphertext := ciphertext[:nonceSize], ciphertext[nonceSize:]
    return gcm.Open(nil, nonce, ciphertext, nil)
}
```

**`modules/config/config.go`** — OrangeConfig struct + ConfigManager:
```go
package config

import (
    "encoding/json"
    "os"
    "path/filepath"
)

// OrangeConfig holds all application configuration.
type OrangeConfig struct {
    LogLevel        string            `json:"log_level"`
    DefaultProvider string            `json:"default_provider"`
    DefaultModel    string            `json:"default_model"`
    ControlPort     int               `json:"control_port"`
    Providers       map[string]ProviderConfig `json:"providers"`
    Hooks           HooksConfig       `json:"hooks"`
    Permissions     PermissionsConfig `json:"permissions"`
}

type ProviderConfig struct {
    APIKey      string            `json:"api_key"`
    APISecret   string            `json:"api_secret,omitempty"`
    BaseURL     string            `json:"base_url,omitempty"`
    DefaultModel string           `json:"default_model,omitempty"`
    TimeoutSecs uint64            `json:"timeout_secs,omitempty"`
    Extra       map[string]string `json:"extra,omitempty"`
}

type HooksConfig struct {
    PreToolCall  []string `json:"pre_tool_call,omitempty"`
    PostToolCall []string `json:"post_tool_call,omitempty"`
}

type PermissionsConfig struct {
    Bash   string `json:"bash,omitempty"`   // "allow", "deny", "ask"
    Write  string `json:"write,omitempty"`
    Edit   string `json:"edit,omitempty"`
    Read   string `json:"read,omitempty"`
    Execute string `json:"execute,omitempty"`
}

func DefaultConfig() *OrangeConfig {
    return &OrangeConfig{
        LogLevel:    "info",
        ControlPort: 3200,
        Providers:   make(map[string]ProviderConfig),
    }
}

// ConfigManager handles loading, saving, and querying config files.
type ConfigManager struct{}

func NewConfigManager() *ConfigManager { return &ConfigManager{} }

func (m *ConfigManager) Load(path string) (*OrangeConfig, error) {
    data, err := os.ReadFile(path)
    if err != nil {
        return nil, fmt.Errorf("read config: %w", err)
    }
    jsonStr, err := ParseJSONC(string(data))
    if err != nil {
        return nil, err
    }
    cfg := DefaultConfig()
    if err := json.Unmarshal([]byte(jsonStr), cfg); err != nil {
        return nil, fmt.Errorf("unmarshal config: %w", err)
    }
    return cfg, nil
}

func (m *ConfigManager) Save(path string, cfg *OrangeConfig) error {
    if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
        return err
    }
    data, err := json.MarshalIndent(cfg, "", "  ")
    if err != nil {
        return err
    }
    return os.WriteFile(path, data, 0644)
}

func (m *ConfigManager) Get(path, key string) (interface{}, error) {
    cfg, err := m.Load(path)
    if err != nil {
        return nil, err
    }
    // Use reflection or a switch for key lookup.
    switch key {
    case "log_level":
        return cfg.LogLevel, nil
    case "default_provider":
        return cfg.DefaultProvider, nil
    case "control_port":
        return cfg.ControlPort, nil
    default:
        return nil, fmt.Errorf("unknown config key: %q", key)
    }
}

func (m *ConfigManager) Set(path, key string, value interface{}) error {
    cfg, err := m.Load(path)
    if err != nil {
        return err
    }
    switch key {
    case "log_level":
        cfg.LogLevel = fmt.Sprintf("%v", value)
    case "default_provider":
        cfg.DefaultProvider = fmt.Sprintf("%v", value)
    default:
        return fmt.Errorf("unknown config key: %q", key)
    }
    return m.Save(path, cfg)
}
```

Implement `jsonc.go` with a simple state-machine comment stripper that handles `//` line comments and `/* */` block comments, then returns the cleaned JSON string.

- [ ] **Step 4: Run tests**

```bash
cd modules/config && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/config/
git commit -m "feat(config): add OrangeConfig, ConfigManager, JSONC parser, AES encryption"
```

---

### Task 7: Audit Module

**Files:**
- Create: `modules/audit/audit.go`
- Create: `modules/audit/audit_test.go`

- [ ] **Step 1: Write failing tests for audit**

```go
// modules/audit/audit_test.go
package audit

import (
    "testing"
    "time"
)

func TestAuditEntryHashChain(t *testing.T) {
    entry1 := NewEntry("tool_call", "agent-1", `{"tool":"bash"}`)
    if len(entry1.Hash) == 0 {
        t.Fatal("entry1.Hash should not be empty")
    }

    entry2 := NewEntry("tool_result", "agent-1", `{"result":"ok"}`)
    entry2.PrevHash = entry1.Hash
    entry2.ComputeHash()

    // Hash chain: entry2.Hash depends on entry1.Hash
    if entry2.Hash == entry1.Hash {
        t.Fatal("entry2.Hash should differ from entry1.Hash")
    }

    // Verify chain
    entries := []AuditEntry{entry1, entry2}
    if err := VerifyChain(entries); err != nil {
        t.Fatalf("VerifyChain error: %v", err)
    }

    // Tamper with entry1
    entries[0].Action = "tampered"
    if err := VerifyChain(entries); err == nil {
        t.Fatal("VerifyChain should fail on tampered chain")
    }
}

func TestAuditLogAppendGet(t *testing.T) {
    dir := t.TempDir()
    log, err := NewAuditLog(dir)
    if err != nil {
        t.Fatalf("NewAuditLog error: %v", err)
    }
    defer log.Close()

    log.Append("startup", "system", `{"version":"1.0"}`)
    log.Append("tool_call", "agent-1", `{"tool":"bash","cmd":"ls"}`)

    entries := log.GetEntries(time.Time{}, time.Now().UTC())
    if len(entries) != 2 {
        t.Fatalf("GetEntries returned %d, want 2", len(entries))
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/audit && go test -v
```

Expected: FAIL.

- [ ] **Step 3: Implement audit module**

Implement `modules/audit/audit.go`:
- `AuditEntry` struct: `Timestamp time.Time`, `Action string`, `AgentID string`, `Details string`, `PrevHash []byte`, `Hash []byte`
- `NewEntry(action, agentID, details string)` constructor (computes initial hash)
- `(e *AuditEntry) ComputeHash()` — `SHA256(PrevHash + Action + Timestamp + Details)`
- `VerifyChain(entries []AuditEntry) error` — iterates pairs, checks each `PrevHash` matches previous `Hash`
- `AuditLog` struct backed by bbolt (`go.etcd.io/bbolt`):
  - `NewAuditLog(dir string) (*AuditLog, error)` — opens bbolt DB, creates `audit` bucket
  - `(l *AuditLog) Append(action, agentID, details string)` — creates entry with prev hash from last entry, saves to bbolt
  - `(l *AuditLog) GetEntries(from, to time.Time) []AuditEntry` — scans bucket, filters by time range
  - `(l *AuditLog) Close() error`

- [ ] **Step 4: Run tests**

```bash
cd modules/audit && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/audit/
git commit -m "feat(audit): add AuditEntry, SHA-256 hash chain, bbolt-backed AuditLog"
```

---

### Task 8: Invariant Module

**Files:**
- Create: `modules/invariant/invariant.go`
- Create: `modules/invariant/invariant_test.go`

- [ ] **Step 1: Write failing tests**

```go
// modules/invariant/invariant_test.go
package invariant

import (
    "context"
    "errors"
    "testing"
)

func TestGuardPasses(t *testing.T) {
    inv := &alwaysPass{name: "test-pass"}
    guard := NewGuard([]Invariant{inv})
    if err := guard.Check(context.Background()); err != nil {
        t.Fatalf("guard should pass: %v", err)
    }
}

func TestGuardFails(t *testing.T) {
    inv := &alwaysFail{name: "test-fail"}
    guard := NewGuard([]Invariant{inv})
    if err := guard.Check(context.Background()); err == nil {
        t.Fatal("guard should fail")
    }
}

func TestCheckpointRollback(t *testing.T) {
    engine := NewEngine()

    state := map[string]int{"counter": 0}
    engine.Checkpoint("step-0", state)

    state["counter"] = 42
    engine.Checkpoint("step-1", state)

    // Rollback to step-0
    restored, err := engine.Rollback("step-0")
    if err != nil {
        t.Fatalf("Rollback error: %v", err)
    }
    rs := restored.(map[string]int)
    if rs["counter"] != 0 {
        t.Fatalf("restored counter = %d, want 0", rs["counter"])
    }
}

func TestSelfHealingPolicy(t *testing.T) {
    attempts := 0
    policy := NewSelfHealingPolicy(3, func(ctx context.Context) error {
        attempts++
        if attempts < 3 {
            return errors.New("not yet")
        }
        return nil
    })

    err := policy.Execute(context.Background())
    if err != nil {
        t.Fatalf("self-healing should succeed on attempt 3: %v", err)
    }
    if attempts != 3 {
        t.Fatalf("attempts = %d, want 3", attempts)
    }
}

// Test helpers

type alwaysPass struct{ name string }
func (a *alwaysPass) Name() string                     { return a.name }
func (a *alwaysPass) Check(ctx context.Context) error   { return nil }

type alwaysFail struct{ name string }
func (a *alwaysFail) Name() string                     { return a.name }
func (a *alwaysFail) Check(ctx context.Context) error   { return errors.New("invariant violated") }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/invariant && go test -v
```

Expected: FAIL.

- [ ] **Step 3: Implement invariant module**

```go
// modules/invariant/invariant.go
package invariant

import (
    "context"
    "fmt"
    "sync"
)

// Invariant defines a condition that must hold.
type Invariant interface {
    Name() string
    Check(ctx context.Context) error
}

// Guard enforces a set of invariants before an operation.
type Guard struct {
    invariants []Invariant
}

func NewGuard(invariants []Invariant) *Guard {
    return &Guard{invariants: invariants}
}

func (g *Guard) Check(ctx context.Context) error {
    for _, inv := range g.invariants {
        if err := inv.Check(ctx); err != nil {
            return fmt.Errorf("invariant %q violated: %w", inv.Name(), err)
        }
    }
    return nil
}

// Engine manages checkpoints and rollback for invariant enforcement.
type Engine struct {
    mu         sync.RWMutex
    checkpoints map[string]interface{}
}

func NewEngine() *Engine {
    return &Engine{checkpoints: make(map[string]interface{})}
}

func (e *Engine) Checkpoint(id string, state interface{}) {
    e.mu.Lock()
    defer e.mu.Unlock()
    e.checkpoints[id] = state
}

func (e *Engine) Rollback(id string) (interface{}, error) {
    e.mu.RLock()
    defer e.mu.RUnlock()
    state, ok := e.checkpoints[id]
    if !ok {
        return nil, fmt.Errorf("checkpoint %q not found", id)
    }
    return state, nil
}

// SelfHealingPolicy retries an operation up to maxAttempts times.
type SelfHealingPolicy struct {
    maxAttempts int
    fixFunc     func(ctx context.Context) error
}

func NewSelfHealingPolicy(maxAttempts int, fix func(ctx context.Context) error) *SelfHealingPolicy {
    return &SelfHealingPolicy{maxAttempts: maxAttempts, fixFunc: fix}
}

func (p *SelfHealingPolicy) Execute(ctx context.Context) error {
    var lastErr error
    for i := 0; i < p.maxAttempts; i++ {
        if err := p.fixFunc(ctx); err != nil {
            lastErr = err
            continue
        }
        return nil
    }
    return fmt.Errorf("self-healing failed after %d attempts: %w", p.maxAttempts, lastErr)
}
```

- [ ] **Step 4: Run tests**

```bash
cd modules/invariant && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/invariant/
git commit -m "feat(invariant): add Invariant, Guard, Checkpoint/Rollback, SelfHealingPolicy"
```

---

## Phase 3: Layer 1 — AI Module

### Task 9: AI Types and Provider Interface

**Files:**
- Create: `modules/ai/types.go`
- Create: `modules/ai/error.go`
- Create: `modules/ai/provider.go`
- Create: `modules/ai/types_test.go`

- [ ] **Step 1: Write failing tests**

Test `ChatMessage` constructors, `ToolDefinition` serialization, `AiError` variants, `AiProvider` interface compliance via a mock.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/ai && go test -v
```

Expected: FAIL.

- [ ] **Step 3: Implement AI types and interface**

**`modules/ai/error.go`** — `AiError` struct with kinds: `Network`, `Api`, `Auth`, `Parse`, `Stream`, `Config`, `UnsupportedProvider`, `RateLimit`, `Timeout`. Implement `error` interface, `IsRetryable()`.

**`modules/ai/types.go`** — All wire types matching the Rust `provider.rs`:
```go
package ai

// ChatMessage represents a message in the AI provider's wire format.
type ChatMessage struct {
    Role       string      `json:"role"`
    Content    string      `json:"content,omitempty"`
    Name       string      `json:"name,omitempty"`
    ToolCallID string      `json:"tool_call_id,omitempty"`
    ToolCalls  []ToolCall  `json:"tool_calls,omitempty"`
}

// ToolCall in AI wire format.
type ToolCall struct {
    ID       string       `json:"id"`
    Type     string       `json:"type"`
    Function FunctionCall `json:"function"`
}

// FunctionCall holds the function name and raw arguments string.
type FunctionCall struct {
    Name      string `json:"name"`
    Arguments string `json:"arguments"`
}

// ToolDefinition describes a tool for the AI.
type ToolDefinition struct {
    Type     string             `json:"type"`
    Function FunctionDefinition `json:"function"`
}

// FunctionDefinition describes a function tool.
type FunctionDefinition struct {
    Name        string       `json:"name"`
    Description string       `json:"description"`
    Parameters  ToolParameter `json:"parameters"`
}

// ToolParameter is the JSON Schema for a tool's input.
type ToolParameter struct {
    Type       string                 `json:"type"`
    Properties map[string]interface{} `json:"properties"`
    Required   []string               `json:"required,omitempty"`
}

// ChatOptions configures a completion request.
type ChatOptions struct {
    Model         string   `json:"model"`
    Temperature   *float64 `json:"temperature,omitempty"`
    MaxTokens     *uint32  `json:"max_tokens,omitempty"`
    TopP          *float64 `json:"top_p,omitempty"`
    StopSequences []string `json:"stop_sequences,omitempty"`
}

// TokenUsage from AI provider response.
type TokenUsage struct {
    PromptTokens     uint32 `json:"prompt_tokens"`
    CompletionTokens uint32 `json:"completion_tokens"`
    TotalTokens      uint32 `json:"total_tokens"`
}

// AiResponse is a non-streaming completion response.
type AiResponse struct {
    Content      string     `json:"content"`
    ToolCalls    []ToolCall `json:"tool_calls"`
    Usage        TokenUsage `json:"usage"`
    Model        string     `json:"model"`
    FinishReason string     `json:"finish_reason"`
}

// StreamEvent is a single event from a streaming response.
type StreamEvent struct {
    Type    string // "content_delta", "tool_call_delta", "usage", "done"
    Content string
    // For tool_call_delta:
    ToolCallID   string
    ToolCallName string
    Arguments    string
    // For usage:
    Usage *TokenUsage
}
```

**`modules/ai/provider.go`** — The `AiProvider` interface:
```go
package ai

import "context"

// AiProvider is the interface all AI providers must implement.
type AiProvider interface {
    Name() string
    ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error)
    ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error)
}

// ProviderConfig holds configuration for creating a provider.
type ProviderConfig struct {
    APIKey       string            `json:"api_key"`
    APISecret    string            `json:"api_secret,omitempty"`
    BaseURL      string            `json:"base_url,omitempty"`
    DefaultModel string            `json:"default_model,omitempty"`
    TimeoutSecs  uint64            `json:"timeout_secs,omitempty"`
    Extra        map[string]string `json:"extra,omitempty"`
}

// ProviderFactory creates AiProvider instances by name.
type ProviderFactory struct{}

func (f *ProviderFactory) CreateProvider(name string, config ProviderConfig) (AiProvider, error) {
    switch name {
    case "openai", "zai", "z.ai", "zen", "opencode-zen":
        return newOpenAIProvider(config), nil
    case "anthropic", "claude":
        return newAnthropicProvider(config), nil
    case "deepseek":
        return newDeepSeekProvider(config), nil
    case "qianwen", "tongyi", "dashscope":
        return newQianwenProvider(config), nil
    case "wenxin", "ernie", "baidu":
        return newWenxinProvider(config), nil
    default:
        return nil, &AiError{Kind: UnsupportedProvider, Message: fmt.Sprintf("unknown provider: %q", name)}
    }
}
```

Implement `newOpenAIProvider`, `newAnthropicProvider`, etc. as stubs that return `&openAIProvider{config: config}` etc. for now — full implementations come in Tasks 10-13.

- [ ] **Step 4: Run tests**

```bash
cd modules/ai && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/ai/
git commit -m "feat(ai): add ChatMessage, ToolDefinition, AiProvider interface, AiError, ProviderFactory"
```

---

### Task 10: SSE Stream Parser

**Files:**
- Create: `modules/ai/stream.go`
- Create: `modules/ai/stream_test.go`

- [ ] **Step 1: Write tests for SSE parsing**

```go
// modules/ai/stream_test.go
package ai

import (
    "strings"
    "testing"
)

func TestParseSSEStream(t *testing.T) {
    input := `data: {"choices":[{"delta":{"content":"Hello"}}]}

data: {"choices":[{"delta":{"content":" world"}}]}

data: [DONE]

`
    events := ParseSSEStream(strings.NewReader(input))
    if len(events) != 3 {
        t.Fatalf("got %d events, want 3", len(events))
    }
    if events[0] != `{"choices":[{"delta":{"content":"Hello"}}]}` {
        t.Fatalf("event[0] = %q", events[0])
    }
    if events[2] != "[DONE]" {
        t.Fatalf("event[2] = %q, want [DONE]", events[2])
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/ai && go test -run TestParseSSEStream -v
```

Expected: FAIL.

- [ ] **Step 3: Implement SSE stream parser**

```go
// modules/ai/stream.go
package ai

import (
    "bufio"
    "io"
    "strings"
)

// ParseSSEStream reads an SSE stream and returns the data payloads.
func ParseSSEStream(r io.Reader) []string {
    var events []string
    scanner := bufio.NewScanner(r)
    for scanner.Scan() {
        line := scanner.Text()
        if strings.HasPrefix(line, "data: ") {
            data := strings.TrimPrefix(line, "data: ")
            events = append(events, data)
        }
    }
    return events
}
```

- [ ] **Step 4: Run tests**

```bash
cd modules/ai && go test -run TestParseSSEStream -v
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/ai/stream.go modules/ai/stream_test.go
git commit -m "feat(ai): add generic SSE stream parser"
```

---

### Task 11: OpenAI Provider

**Files:**
- Create: `modules/ai/openai.go`
- Create: `modules/ai/openai_test.go`

- [ ] **Step 1: Write tests for OpenAI provider**

Write tests that mock the HTTP server (`httptest.NewServer`) and verify:
- Request body contains correct messages, tools, model
- Streaming response is correctly parsed into `StreamEvent` channel
- Non-streaming response returns correct `AiResponse`
- API key is sent in `Authorization` header

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/ai && go test -run TestOpenAI -v
```

Expected: FAIL.

- [ ] **Step 3: Implement OpenAI provider**

```go
// modules/ai/openai.go
package ai

import (
    "bytes"
    "context"
    "encoding/json"
    "fmt"
    "net/http"
)

type openAIProvider struct {
    config ProviderConfig
    client *http.Client
}

func newOpenAIProvider(config ProviderConfig) *openAIProvider {
    baseURL := config.BaseURL
    if baseURL == "" {
        baseURL = "https://api.openai.com/v1"
    }
    return &openAIProvider{
        config: config,
        client: &http.Client{Timeout: time.Duration(config.TimeoutSecs) * time.Second},
    }
}

func (p *openAIProvider) Name() string { return "openai" }

func (p *openAIProvider) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
    body := p.buildRequest(messages, tools, opts, false)
    // POST to /chat/completions, parse response into AiResponse.
    // ...
}

func (p *openAIProvider) ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error) {
    body := p.buildRequest(messages, tools, opts, true)
    // POST to /chat/completions with stream=true
    // Parse SSE events into StreamEvent channel
    // ...
}

func (p *openAIProvider) buildRequest(messages []ChatMessage, tools []ToolDefinition, opts ChatOptions, stream bool) map[string]interface{} {
    req := map[string]interface{}{
        "model":    opts.Model,
        "messages": messages,
        "stream":   stream,
    }
    if len(tools) > 0 {
        req["tools"] = tools
    }
    if opts.Temperature != nil {
        req["temperature"] = *opts.Temperature
    }
    if opts.MaxTokens != nil {
        req["max_tokens"] = *opts.MaxTokens
    }
    return req
}
```

Full implementation: POST to `{baseURL}/chat/completions`, set `Authorization: Bearer {apiKey}`, parse response. For streaming, read SSE lines via `ParseSSEStream`, parse each JSON line into `StreamEvent`, send on channel.

- [ ] **Step 4: Run tests**

```bash
cd modules/ai && go test -run TestOpenAI -v
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/ai/openai.go modules/ai/openai_test.go
git commit -m "feat(ai): implement OpenAI-compatible provider with streaming"
```

---

### Task 12: Anthropic Provider

**Files:**
- Create: `modules/ai/anthropic.go`
- Create: `modules/ai/anthropic_test.go`

- [ ] **Step 1-5:** Follow same pattern as Task 11. Implement Anthropic-specific API format:
- Endpoint: `https://api.anthropic.com/v1/messages`
- Headers: `x-api-key`, `anthropic-version: 2023-06-01`
- Request format: `{"model", "max_tokens", "messages", "system", "tools"}`
- Streaming: uses SSE with event types `content_block_delta`, `message_stop`, etc.
- Tools use Anthropic's `input_schema` format (not OpenAI's `parameters`)

```bash
git add modules/ai/anthropic.go modules/ai/anthropic_test.go
git commit -m "feat(ai): implement Anthropic provider with streaming"
```

---

### Task 13: Chinese Providers (DeepSeek, Qianwen, Wenxin)

**Files:**
- Create: `modules/ai/deepseek.go`
- Create: `modules/ai/qianwen.go`
- Create: `modules/ai/wenxin.go`
- Create: `modules/ai/providers_test.go`

- [ ] **Step 1-5:** Each provider follows the same pattern:
- DeepSeek: OpenAI-compatible format, base URL `https://api.deepseek.com/v1`
- Qianwen (通义千问): DashScope API format, SSE streaming
- Wenxin (文心一言): Baidu ERNIE API format, OAuth token flow

Each has its own request/response format quirks. Implement `AiProvider` interface for each.

```bash
git add modules/ai/deepseek.go modules/ai/qianwen.go modules/ai/wenxin.go modules/ai/providers_test.go
git commit -m "feat(ai): implement DeepSeek, Qianwen, and Wenxin providers"
```

---

### Task 14: FallbackChain and ModelRouter

**Files:**
- Create: `modules/ai/fallback.go`
- Create: `modules/ai/router.go`
- Create: `modules/ai/fallback_test.go`

- [ ] **Step 1: Write tests**

Test `FallbackChain`: first provider fails → falls back to second. All fail → returns error. Cooldown prevents retrying failed provider. Test `ModelRouter`: intent category maps to correct provider+model.

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Implement FallbackChain and ModelRouter**

```go
// modules/ai/fallback.go
package ai

import (
    "context"
    "sync"
    "time"
)

// FallbackChain tries providers in order, skipping those on cooldown.
type FallbackChain struct {
    providers []AiProvider
    cooldowns map[string]time.Time
    cooldownDur time.Duration
    mu        sync.RWMutex
}

func NewFallbackChain(providers []AiProvider, cooldown time.Duration) *FallbackChain {
    return &FallbackChain{
        providers:   providers,
        cooldowns:   make(map[string]time.Time),
        cooldownDur: cooldown,
    }
}

func (c *FallbackChain) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
    var lastErr error
    for _, p := range c.providers {
        if c.isOnCooldown(p.Name()) {
            continue
        }
        resp, err := p.ChatCompletion(ctx, messages, tools, opts)
        if err == nil {
            return resp, nil
        }
        lastErr = err
        c.setCooldown(p.Name())
    }
    return nil, fmt.Errorf("all providers failed, last error: %w", lastErr)
}

// Similarly for ChatCompletionStream.

func (c *FallbackChain) isOnCooldown(name string) bool {
    c.mu.RLock()
    defer c.mu.RUnlock()
    t, ok := c.cooldowns[name]
    return ok && time.Now().Before(t)
}

func (c *FallbackChain) setCooldown(name string) {
    c.mu.Lock()
    defer c.mu.Unlock()
    c.cooldowns[name] = time.Now().Add(c.cooldownDur)
}
```

```go
// modules/ai/router.go
package ai

// ModelCategory classifies the type of task.
type ModelCategory int

const (
    CategoryCoding   ModelCategory = iota
    CategoryPlanning
    CategoryReview
    CategoryAnswer
    CategoryExplore
    CategoryCreative
    CategoryAnalysis
    CategoryGeneral
)

// RoutingRule maps a category to a provider and model.
type RoutingRule struct {
    Category ModelCategory
    Provider string
    Model    string
}

// ModelRouter selects the best provider+model based on intent.
type ModelRouter struct {
    rules []RoutingRule
}

func NewModelRouter(rules []RoutingRule) *ModelRouter {
    return &ModelRouter{rules: rules}
}

func (r *ModelRouter) Route(category ModelCategory) (provider, model string) {
    for _, rule := range r.rules {
        if rule.Category == category {
            return rule.Provider, rule.Model
        }
    }
    return "openai", "gpt-4o" // default fallback
}
```

- [ ] **Step 4: Run tests**

```bash
cd modules/ai && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/ai/fallback.go modules/ai/router.go modules/ai/fallback_test.go
git commit -m "feat(ai): add FallbackChain with cooldown and ModelRouter"
```

---

## Phase 4: Layer 1 — Session, MCP, Tools

### Task 15: Session Module

**Files:**
- Create: `modules/session/session.go`
- Create: `modules/session/storage.go`
- Create: `modules/session/tree.go`
- Create: `modules/session/blob.go`
- Create: `modules/session/session_test.go`

- [ ] **Step 1: Write tests**

Test `SessionManager` CRUD, JSONL read/write, `SessionTree` fork/merge, `BlobStore` content-addressed put/get.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/session && go test -v
```

Expected: FAIL.

- [ ] **Step 3: Implement session module**

**`session.go`** — `Session` struct and `SessionManager`:
```go
type Session struct {
    ID          core.SessionId          `json:"id"`
    Messages    []core.Message          `json:"messages"`
    Metadata    map[string]string       `json:"metadata"`
    TokenUsage  core.TokenUsage         `json:"token_usage"`
    CreatedAt   time.Time               `json:"created_at"`
    UpdatedAt   time.Time               `json:"updated_at"`
    ParentID    *core.SessionId         `json:"parent_id,omitempty"`
}
```

**`storage.go`** — JSONL file storage: `WriteSession(dir, session)`, `ReadSession(dir, id)`. Each message is one JSON line.

**`tree.go`** — `SessionTree` with `Fork(parentID) -> childID`, `GetChildren(id) []SessionId`.

**`blob.go`** — `BlobStore` with `Put(data) -> hash`, `Get(hash) -> data`. Uses SHA-256 for content addressing.

- [ ] **Step 4: Run tests**

```bash
cd modules/session && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/session/
git commit -m "feat(session): add Session, SessionManager, JSONL storage, SessionTree, BlobStore"
```

---

### Task 16: MCP Module

**Files:**
- Create: `modules/mcp/jsonrpc.go`
- Create: `modules/mcp/transport.go`
- Create: `modules/mcp/client.go`
- Create: `modules/mcp/server.go`
- Create: `modules/mcp/mcp_test.go`

- [ ] **Step 1-5:** Implement JSON-RPC 2.0 types (`Request`, `Response`, `Notification`), `Transport` interface with `StdioTransport`, `McpClient` (discover tools, call them), `McpServer` (expose tools). Test with mock stdio pipes.

```bash
git add modules/mcp/
git commit -m "feat(mcp): add JSON-RPC 2.0, Transport, McpClient, McpServer"
```

---

### Task 17: Tools — Interface, Registry, Permissions

**Files:**
- Create: `modules/tools/tool.go`
- Create: `modules/tools/registry.go`
- Create: `modules/tools/permissions.go`
- Create: `modules/tools/security.go`
- Create: `modules/tools/batch.go`
- Create: `modules/tools/tool_test.go`

- [ ] **Step 1: Write tests**

Test `Tool` interface, `ToolRegistry` register/get/list, `PermissionContext` decisions, `PathValidator` blocks traversal, `BatchPartition` concurrent execution.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/tools && go test -v
```

Expected: FAIL.

- [ ] **Step 3: Implement tool framework**

```go
// modules/tools/tool.go
package tools

import (
    "context"
    "encoding/json"
)

// ToolError represents a tool execution error.
type ToolError struct {
    Kind    string // "invalid_params", "execution_error", "security_violation", "not_found"
    Message string
}

func (e *ToolError) Error() string { return e.Kind + ": " + e.Message }

// ToolMetadata describes tool capabilities.
type ToolMetadata struct {
    IsReadOnly        bool `json:"is_read_only"`
    IsConcurrencySafe bool `json:"is_concurrency_safe"`
    IsDestructive     bool `json:"is_destructive"`
    IsEnabled         bool `json:"is_enabled"`
}

func DefaultMetadata() ToolMetadata {
    return ToolMetadata{IsEnabled: true}
}

func ReadOnlyMetadata() ToolMetadata {
    return ToolMetadata{IsReadOnly: true, IsConcurrencySafe: true, IsEnabled: true}
}

func DestructiveMetadata() ToolMetadata {
    return ToolMetadata{IsDestructive: true, IsEnabled: true}
}

// Tool is the interface all tools must implement.
type Tool interface {
    Name() string
    Description() string
    Parameters() json.RawMessage
    Execute(ctx context.Context, input json.RawMessage) (string, error)
    Metadata() ToolMetadata
}
```

```go
// modules/tools/registry.go
package tools

import "sync"

type ToolRegistry struct {
    mu    sync.RWMutex
    tools map[string]Tool
}

func NewToolRegistry() *ToolRegistry {
    return &ToolRegistry{tools: make(map[string]Tool)}
}

func (r *ToolRegistry) Register(t Tool) {
    r.mu.Lock()
    defer r.mu.Unlock()
    r.tools[t.Name()] = t
}

func (r *ToolRegistry) Get(name string) (Tool, bool) {
    r.mu.RLock()
    defer r.mu.RUnlock()
    t, ok := r.tools[name]
    return t, ok
}

func (r *ToolRegistry) List() []Tool {
    r.mu.RLock()
    defer r.mu.RUnlock()
    result := make([]Tool, 0, len(r.tools))
    for _, t := range r.tools {
        result = append(result, t)
    }
    return result
}
```

Implement `permissions.go` (PermissionContext with 5 types), `security.go` (PathValidator, SecurityPolicy), `batch.go` (concurrent batch execution with `sync.WaitGroup` and result aggregation).

- [ ] **Step 4: Run tests**

```bash
cd modules/tools && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/tools/
git commit -m "feat(tools): add Tool interface, ToolRegistry, PermissionContext, SecurityPolicy, BatchPartition"
```

---

### Task 18: Tool Implementations

**Files:**
- Create: `modules/tools/bash.go`
- Create: `modules/tools/file_tools.go`
- Create: `modules/tools/search_tools.go`
- Create: `modules/tools/other_tools.go`
- Create: `modules/tools/default_registry.go`
- Create: `modules/tools/tools_test.go`

- [ ] **Step 1: Write tests for each tool**

Table-driven tests for bash (execute `echo hello`), read_file, write_file, edit_file, grep, find, glob.

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Implement all tools**

**`bash.go`** — `BashTool`: executes shell commands via `os/exec`, captures stdout/stderr, respects timeout. Returns `(string, error)`.

**`file_tools.go`** — `ReadFileTool`, `WriteFileTool`, `EditFileTool`, `DeleteFileTool`, `ListDirectoryTool`. Use `os.ReadFile`, `os.WriteFile`, `os.ReadDir`. `EditFileTool` does find-replace on file content.

**`search_tools.go`** — `GrepTool` (uses `regexp`), `FindTool` (uses `filepath.Walk`), `GlobTool` (uses `filepath.Glob`).

**`other_tools.go`** — `FetchTool` (HTTP GET via `net/http`), `PythonTool` (exec `python3`), `CalcTool` (evaluate expressions), `TaskTool` (CRUD for task list). Stub implementations for `BrowserTool`, `SshTool`, `LspTool`, `WebSearchTool`, `NotebookTool`.

**`default_registry.go`** — `CreateDefaultRegistry()` that registers all tools.

- [ ] **Step 4: Run tests**

```bash
cd modules/tools && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/tools/
git commit -m "feat(tools): implement bash, file, search tools and default registry"
```

---

## Phase 5: Layer 2 — Agent Module

### Task 19: AgentContext and ToolExecutor

**Files:**
- Create: `modules/agent/context.go`
- Create: `modules/agent/executor.go`
- Create: `modules/agent/executor_test.go`

- [ ] **Step 1: Write tests**

Test `AgentContext` message management, `ToolExecutor` single/batch execution with timeout, permission checking.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/agent && go test -v
```

Expected: FAIL.

- [ ] **Step 3: Implement**

```go
// modules/agent/context.go
package agent

import (
    "github.com/woyin/OrangeCoding/modules/core"
    "path/filepath"
)

// AgentContext holds the state for an agent's conversation.
type AgentContext struct {
    sessionID  core.SessionId
    conversation *core.Conversation
    workDir    string
    env        map[string]string
    metadata   map[string]string
}

func NewAgentContext(sessionID core.SessionId, workDir string) *AgentContext {
    return &AgentContext{
        sessionID:    sessionID,
        conversation: core.NewConversation(),
        workDir:      workDir,
        env:          make(map[string]string),
        metadata:     make(map[string]string),
    }
}

func (c *AgentContext) SetSystemPrompt(prompt string) {
    if c.conversation.IsEmpty() {
        c.conversation.AddMessage(core.NewSystemMessage(prompt))
    }
}

func (c *AgentContext) AddUserMessage(content string) {
    c.conversation.AddMessage(core.NewUserMessage(content))
}

func (c *AgentContext) AddAssistantMessage(content string) {
    c.conversation.AddMessage(core.NewAssistantMessage(content))
}

func (c *AgentContext) AddToolResult(result core.ToolResult) {
    c.conversation.AddMessage(result.ToMessage())
}

func (c *AgentContext) Conversation() *core.Conversation { return c.conversation }
func (c *AgentContext) SessionID() core.SessionId         { return c.sessionID }
func (c *AgentContext) WorkDir() string                    { return c.workDir }
```

```go
// modules/agent/executor.go
package agent

import (
    "context"
    "encoding/json"
    "fmt"
    "sync"
    "time"

    "github.com/woyin/OrangeCoding/modules/tools"
)

// ToolExecutor runs tool calls against the tool registry.
type ToolExecutor struct {
    registry *tools.ToolRegistry
    timeout  time.Duration
}

func NewToolExecutor(registry *tools.ToolRegistry) *ToolExecutor {
    return &ToolExecutor{
        registry: registry,
        timeout:  30 * time.Second,
    }
}

// ExecuteResult holds the outcome of a tool execution.
type ExecuteResult struct {
    ToolCallID string
    Content    string
    IsError    bool
    Duration   time.Duration
}

func (e *ToolExecutor) Execute(ctx context.Context, call core.ToolCall) ExecuteResult {
    tool, ok := e.registry.Get(call.FunctionName)
    if !ok {
        return ExecuteResult{
            ToolCallID: call.ID,
            Content:    fmt.Sprintf("tool not found: %s", call.FunctionName),
            IsError:    true,
        }
    }

    toolCtx, cancel := context.WithTimeout(ctx, e.timeout)
    defer cancel()

    start := time.Now()
    result, err := tool.Execute(toolCtx, call.Arguments)
    dur := time.Since(start)

    if err != nil {
        return ExecuteResult{ToolCallID: call.ID, Content: err.Error(), IsError: true, Duration: dur}
    }
    return ExecuteResult{ToolCallID: call.ID, Content: result, IsError: false, Duration: dur}
}

func (e *ToolExecutor) ExecuteBatch(ctx context.Context, calls []core.ToolCall) []ExecuteResult {
    results := make([]ExecuteResult, len(calls))
    var wg sync.WaitGroup
    for i, call := range calls {
        wg.Add(1)
        go func(idx int, c core.ToolCall) {
            defer wg.Done()
            results[idx] = e.Execute(ctx, c)
        }(i, call)
    }
    wg.Wait()
    return results
}
```

- [ ] **Step 4: Run tests**

```bash
cd modules/agent && go test -v
```

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/agent/context.go modules/agent/executor.go modules/agent/executor_test.go
git commit -m "feat(agent): add AgentContext and ToolExecutor"
```

---

### Task 20: Agent Loop

**Files:**
- Create: `modules/agent/loop.go`
- Create: `modules/agent/loop_test.go`

- [ ] **Step 1: Write tests**

Test agent loop with a mock `AiProvider` that returns a sequence of: text response → tool call → tool result → final text. Verify the loop executes tool calls and produces correct final output.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/agent && go test -run TestAgentLoop -v
```

Expected: FAIL.

- [ ] **Step 3: Implement agent loop**

```go
// modules/agent/loop.go
package agent

import (
    "context"
    "sync"
    "time"

    "github.com/woyin/OrangeCoding/modules/ai"
    "github.com/woyin/OrangeCoding/modules/core"
    "github.com/woyin/OrangeCoding/modules/tools"
)

// AgentLoopConfig configures the agent loop.
type AgentLoopConfig struct {
    MaxIterations     uint32
    Timeout           time.Duration
    AutoApproveTools  bool
}

func DefaultLoopConfig() AgentLoopConfig {
    return AgentLoopConfig{
        MaxIterations:    20,
        Timeout:          300 * time.Second,
        AutoApproveTools: false,
    }
}

// AgentLoopResult captures the outcome of an agent loop run.
type AgentLoopResult struct {
    ToolCallsMade uint32
    TokensUsed    core.TokenUsage
    Duration      time.Duration
}

// AgentLoop is the core agent execution loop.
type AgentLoop struct {
    id       core.AgentId
    provider ai.AiProvider
    executor *ToolExecutor
    context  *AgentContext
    config   AgentLoopConfig
    tools    []ai.ToolDefinition
}

func NewAgentLoop(
    id core.AgentId,
    provider ai.AiProvider,
    executor *ToolExecutor,
    ctx *AgentContext,
    config AgentLoopConfig,
    toolDefs []ai.ToolDefinition,
) *AgentLoop {
    return &AgentLoop{
        id:       id,
        provider: provider,
        executor: executor,
        context:  ctx,
        config:   config,
        tools:    toolDefs,
    }
}

// Run executes the agent loop until completion or cancellation.
func (l *AgentLoop) Run(ctx context.Context, chatOpts ai.ChatOptions, eventCh chan<- core.AgentEvent) (*AgentLoopResult, error) {
    ctx, cancel := context.WithTimeout(ctx, l.config.Timeout)
    defer cancel()

    start := time.Now()
    result := &AgentLoopResult{}
    sessionID := l.context.SessionID()

    if eventCh != nil {
        eventCh <- core.NewStartedEvent(l.id, sessionID)
    }

    for i := uint32(0); i < l.config.MaxIterations; i++ {
        // 1. Convert conversation to AI messages
        messages := conversationToAIMessages(l.context.Conversation())

        // 2. Call AI provider (streaming)
        streamCh, err := l.provider.ChatCompletionStream(ctx, messages, l.tools, chatOpts)
        if err != nil {
            if eventCh != nil {
                eventCh <- core.NewErrorEvent(l.id, sessionID, err.Error())
            }
            return result, err
        }

        // 3. Collect streaming response
        var content string
        var toolCalls []core.ToolCall
        for ev := range streamCh {
            switch ev.Type {
            case "content_delta":
                content += ev.Content
                if eventCh != nil {
                    eventCh <- core.NewStreamChunkEvent(l.id, sessionID, ev.Content)
                }
            case "tool_call_delta":
                toolCalls = append(toolCalls, core.ToolCall{
                    ID:           ev.ToolCallID,
                    FunctionName: ev.ToolCallName,
                    Arguments:    json.RawMessage(ev.Arguments),
                })
            case "done":
                // Stream complete
            }
        }

        // 4. Add assistant message to conversation
        if len(toolCalls) > 0 {
            l.context.AddAssistantMessage(content)
            // Add tool calls to the last message
            // (need to update the last message's ToolCalls field)
        } else {
            l.context.AddAssistantMessage(content)
            // No tool calls → agent is done
            if eventCh != nil {
                eventCh <- core.NewCompletedEvent(l.id, sessionID, content)
            }
            result.Duration = time.Since(start)
            return result, nil
        }

        // 5. Execute tool calls
        results := l.executor.ExecuteBatch(ctx, toolCalls)
        result.ToolCallsMade += uint32(len(toolCalls))

        for _, r := range results {
            l.context.AddToolResult(core.NewToolResultSuccess(r.ToolCallID, r.Content))
            if eventCh != nil {
                name := core.NewToolName("tool")
                eventCh <- core.NewToolCallCompletedEvent(l.id, sessionID, name, !r.IsError, uint64(r.Duration.Milliseconds()))
            }
        }
    }

    result.Duration = time.Since(start)
    return result, nil
}

func conversationToAIMessages(conv *core.Conversation) []ai.ChatMessage {
    var msgs []ai.ChatMessage
    for _, m := range conv.Messages() {
        role := m.Role.String()
        msg := ai.ChatMessage{Role: role, Content: m.Content}
        if m.ToolCallID != "" {
            msg.ToolCallID = m.ToolCallID
        }
        for _, tc := range m.ToolCalls {
            msg.ToolCalls = append(msg.ToolCalls, ai.ToolCall{
                ID:   tc.ID,
                Type: "function",
                Function: ai.FunctionCall{
                    Name:      tc.FunctionName,
                    Arguments: string(tc.Arguments),
                },
            })
        }
        msgs = append(msgs, msg)
    }
    return msgs
}
```

- [ ] **Step 4: Run tests**

```bash
cd modules/agent && go test -run TestAgentLoop -v
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add modules/agent/loop.go modules/agent/loop_test.go
git commit -m "feat(agent): implement core agent loop with streaming and tool execution"
```

---

### Task 21: IntentGate, Fork, Memory

**Files:**
- Create: `modules/agent/intent_gate.go`
- Create: `modules/agent/fork.go`
- Create: `modules/agent/memory.go`
- Create: `modules/agent/intent_test.go`

- [ ] **Step 1-5:** Implement:

**`intent_gate.go`** — `IntentGate` classifies user input into `ModelCategory` using keyword matching and regex patterns.

**`fork.go`** — `ForkAgent` clones parent `AgentContext`, creates a new `AgentLoop` with restricted tool subset, runs it in a goroutine.

**`memory.go`** — `MemoryStore` with file-backed persistence: `Write(key, value)`, `Read(key)`, `List()`, `Recall(query)`. Stores in `~/.orangecoding/memory/`.

```bash
git add modules/agent/intent_gate.go modules/agent/fork.go modules/agent/memory.go modules/agent/intent_test.go
git commit -m "feat(agent): add IntentGate, ForkAgent, and MemoryStore"
```

---

### Task 22: Hooks and Skills

**Files:**
- Create: `modules/agent/hooks.go`
- Create: `modules/agent/skills.go`
- Create: `modules/agent/hooks_test.go`

- [ ] **Step 1-5:** Implement:

**`hooks.go`** — `HookManager` with hook points: `PreToolCall`, `PostToolCall`, `PreSampling`, `PostSampling`, `SessionStart`, `SessionEnd`. Each hook is a shell command that runs via `os/exec`.

**`skills.go`** — `Skill` struct (`Name`, `Description`, `Tools []string`, `Prompt string`), `SkillRegistry` with 6 built-in skills.

```bash
git add modules/agent/hooks.go modules/agent/skills.go modules/agent/hooks_test.go
git commit -m "feat(agent): add HookManager and SkillRegistry"
```

---

### Task 23: Compaction and TTSR

**Files:**
- Create: `modules/agent/compaction.go`
- Create: `modules/agent/ttsr.go`
- Create: `modules/agent/compaction_test.go`

- [ ] **Step 1-5:** Implement:

**`compaction.go`** — `Compactor` reduces conversation size when approaching context limits. Strategies: summarize old messages, drop tool results, keep recent N messages.

**`ttsr.go`** — `TTSR` (Regex-Triggered Streaming Rule Injection): watches streaming output for regex patterns and injects additional rules/prompts.

```bash
git add modules/agent/compaction.go modules/agent/ttsr.go modules/agent/compaction_test.go
git commit -m "feat(agent): add Compactor and TTSR"
```

---

### Task 24: Sub-Agents and Workflows

**Files:**
- Create: `modules/agent/agents/` directory with 11 agent files
- Create: `modules/agent/workflows/` directory with 4 workflow files
- Create: `modules/agent/agents_test.go`

- [ ] **Step 1-5:** Implement:

**Sub-agents** — Each agent (Sisyphus, Hephaestus, Prometheus, Atlas, Oracle, Librarian, Explore, Metis, Momus, Junior, Multimodal) is a struct implementing:
```go
type Agent interface {
    ID() core.AgentId
    Role() core.AgentRole
    Run(ctx context.Context, task string) error
    Stop() error
    Status() core.AgentStatus
}
```

Each agent has a specialized system prompt and tool subset. `Sisyphus` is the main general-purpose agent with all tools.

**Workflows:**
- `UltraWork` — autonomous mode with step/token budget tracking
- `Planning` — Prometheus decomposes tasks into plans
- `Execution` — Atlas executes plans step by step
- `Boulder` — recovery when agent gets stuck

```bash
git add modules/agent/agents/ modules/agent/workflows/ modules/agent/agents_test.go
git commit -m "feat(agent): implement 11 sub-agents and 4 workflows"
```

---

## Phase 6: Layer 2 — Mesh Module

### Task 25: Mesh Module

**Files:**
- Create: `modules/mesh/bus.go`
- Create: `modules/mesh/orchestrator.go`
- Create: `modules/mesh/registry.go`
- Create: `modules/mesh/negotiator.go`
- Create: `modules/mesh/mesh_test.go`

- [ ] **Step 1-5:** Implement:

**`bus.go`** — `MessageBus` with pub/sub, topic-based routing via channels.

**`orchestrator.go`** — `TaskOrchestrator` with DAG-based scheduling. `AddTask(id, deps, fn)`, `Run(ctx)`. Uses topological sort for execution order.

**`registry.go`** — `AgentRegistry` for agent discovery and capability lookup.

**`negotiator.go`** — `Negotiator` for inter-agent task handoff, `BuddyObserver` for async reactions.

```bash
git add modules/mesh/
git commit -m "feat(mesh): add MessageBus, TaskOrchestrator, AgentRegistry, Negotiator"
```

---

## Phase 7: Layer 3 — Interface Modules

### Task 26: Control Protocol

**Files:**
- Create: `modules/control-protocol/messages.go`
- Create: `modules/control-protocol/messages_test.go`

- [ ] **Step 1-5:** Implement `ClientCommand` and `ServerEvent` interfaces with concrete types: `SendTaskCommand`, `ApproveCommand`, `CancelCommand`, `TaskUpdateEvent`, `ToolCallEvent`, `ApprovalRequestEvent`. All with JSON serialization.

```bash
git add modules/control-protocol/
git commit -m "feat(control-protocol): add ClientCommand and ServerEvent message types"
```

---

### Task 27: Control Server

**Files:**
- Create: `modules/control-server/server.go`
- Create: `modules/control-server/handlers.go`
- Create: `modules/control-server/ws.go`
- Create: `modules/control-server/middleware.go`
- Create: `modules/control-server/server_test.go`

- [ ] **Step 1-5:** Implement Gin HTTP server with:
- REST endpoints: `POST /sessions`, `GET /sessions/:id`, `POST /sessions/:id/task`, `DELETE /sessions/:id`, `GET /status`
- WebSocket: `/ws` endpoint for real-time event streaming
- Middleware: CORS, request logging, authentication (API key header)
- Default bind: `127.0.0.1:3200`

```bash
git add modules/control-server/
git commit -m "feat(control-server): add Gin HTTP+WebSocket server"
```

---

### Task 28: Worker Module

**Files:**
- Create: `modules/worker/runtime.go`
- Create: `modules/worker/executor.go`
- Create: `modules/worker/worker_test.go`

- [ ] **Step 1-5:** Implement `WorkerRuntime` that spawns agent goroutines, `AgentExecutor` that wraps agent execution with progress reporting via channels.

```bash
git add modules/worker/
git commit -m "feat(worker): add WorkerRuntime and AgentExecutor"
```

---

### Task 29: TUI Module

**Files:**
- Create: `modules/tui/app.go`
- Create: `modules/tui/model.go`
- Create: `modules/tui/update.go`
- Create: `modules/tui/view.go`
- Create: `modules/tui/markdown.go`
- Create: `modules/tui/theme.go`
- Create: `modules/tui/components/` directory
- Create: `modules/tui/tui_test.go`

- [ ] **Step 1-5:** Implement Bubble Tea TUI:

**`model.go`** — `Model` struct implementing `tea.Model`:
```go
type Model struct {
    messages   []core.Message
    input      textarea.Model
    viewport   viewport.Model
    sidebar    bool
    status     string
    mode       string // "normal", "plan", "goal", "ultra"
    theme      Theme
    agentLoop  *agent.AgentLoop
    eventCh    chan core.AgentEvent
}
```

**`update.go`** — `Update(msg tea.Msg)` handles keyboard input (Enter to send, Ctrl+C to quit, Tab to toggle sidebar, slash commands).

**`view.go`** — `View()` renders markdown messages, input area, status bar.

**`markdown.go`** — Uses `glamour` to render markdown for terminal.

**`theme.go`** — `Theme` struct with color definitions, `LightTheme`, `DarkTheme`.

**`components/`** — Reusable components: `statusbar.go`, `sidebar.go`, `sessionview.go`.

```bash
git add modules/tui/
git commit -m "feat(tui): add Bubble Tea TUI with markdown rendering and theme system"
```

---

### Task 30: CLI Module

**Files:**
- Create: `modules/cli/main.go`
- Create: `modules/cli/root.go`
- Create: `modules/cli/launch.go`
- Create: `modules/cli/init.go`
- Create: `modules/cli/config.go`
- Create: `modules/cli/status.go`
- Create: `modules/cli/serve.go`
- Create: `modules/cli/version.go`

- [ ] **Step 1: Write tests for CLI commands**

```go
// modules/cli/cli_test.go
package main

import (
    "testing"
)

func TestVersionCommand(t *testing.T) {
    // Test that version command outputs version string
}

func TestInitCommand(t *testing.T) {
    // Test that init creates config file in temp dir
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd modules/cli && go test -v
```

Expected: FAIL.

- [ ] **Step 3: Implement CLI**

**`main.go`**:
```go
package main

import "os"

func main() {
    if err := rootCmd.Execute(); err != nil {
        os.Exit(1)
    }
}
```

**`root.go`** — Cobra root command with persistent flags (`--log-level`, `--json-log`).

**`launch.go`** — `launchCmd` with `-p/--prompt` flag. Three modes:
- No prompt → TUI mode (creates Bubble Tea program)
- With prompt → single-shot mode (runs agent loop once)
- `--text` flag → text REPL mode

**`init.go`** — Creates `~/.orangecoding/config.json` with defaults.

**`config.go`** — `config get <key>` and `config set <key> <value>` subcommands.

**`status.go`** — Shows system status (version, providers configured, sessions count).

**`serve.go`** — Starts control server on configured port.

**`version.go`** — Prints version string.

- [ ] **Step 4: Run tests**

```bash
cd modules/cli && go test -v
```

Expected: PASS.

- [ ] **Step 5: Build and verify**

```bash
go build -o orangecoding ./modules/cli
./orangecoding version
./orangecoding --help
```

Expected: Binary builds and runs.

- [ ] **Step 6: Commit**

```bash
git add modules/cli/
git commit -m "feat(cli): add Cobra CLI with launch, init, config, status, serve, version commands"
```

---

## Phase 8: Integration and Polish

### Task 31: Integration Tests

**Files:**
- Create: `tests/integration/agent_loop_test.go`
- Create: `tests/integration/session_test.go`
- Create: `tests/integration/tools_test.go`

- [ ] **Step 1:** Write integration tests that exercise the full stack:
- Agent loop with mock AI provider → tool execution → session persistence
- Session manager CRUD with real file I/O
- Tool registry with real bash/file operations

- [ ] **Step 2: Run all tests**

```bash
go test ./...
```

Expected: All tests pass across all 15 modules.

- [ ] **Step 3: Commit**

```bash
git add tests/
git commit -m "test: add integration tests for agent loop, session, and tools"
```

---

### Task 32: Build Verification and Cross-Compilation

- [ ] **Step 1: Verify clean build**

```bash
go work sync
go build ./modules/...
go test ./...
go vet ./...
```

- [ ] **Step 2: Cross-compile for all targets**

```bash
GOOS=linux GOARCH=amd64 go build -o orangecoding-linux-amd64 ./modules/cli
GOOS=linux GOARCH=arm64 go build -o orangecoding-linux-arm64 ./modules/cli
GOOS=darwin GOARCH=amd64 go build -o orangecoding-darwin-amd64 ./modules/cli
GOOS=darwin GOARCH=arm64 go build -o orangecoding-darwin-arm64 ./modules/cli
```

- [ ] **Step 3: Test the binary**

```bash
./orangecoding version
./orangecoding --help
./orangecoding init
./orangecoding status
```

- [ ] **Step 4: Final commit**

```bash
git add .
git commit -m "feat: complete Go rewrite with all 15 modules"
```

---

## Summary

| Phase | Tasks | Modules |
|-------|-------|---------|
| 0: Workspace | 1 | All 15 scaffolds |
| 1: Core | 2-5 | core |
| 2: Layer 0 | 6-8 | config, audit, invariant |
| 3: AI | 9-14 | ai |
| 4: Layer 1 | 15-18 | session, mcp, tools |
| 5: Agent | 19-24 | agent |
| 6: Mesh | 25 | mesh |
| 7: Interface | 26-30 | control-protocol, control-server, worker, tui, cli |
| 8: Integration | 31-32 | tests, build |

Total: 32 tasks across 8 phases. Each task is designed to produce compilable, testable code that can be committed independently.
