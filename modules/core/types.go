package core

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/google/uuid"
)

// ---------------------------------------------------------------------------
// AgentId
// ---------------------------------------------------------------------------

// AgentId uniquely identifies an agent instance. It wraps a UUID v4.
type AgentId struct {
	id uuid.UUID
}

// NewAgentId generates a new random AgentId.
func NewAgentId() AgentId {
	return AgentId{id: uuid.New()}
}

// String returns the agent ID in the format "agent-{uuid}".
func (a AgentId) String() string {
	return "agent-" + a.id.String()
}

// MarshalJSON serializes AgentId as a quoted JSON string.
func (a AgentId) MarshalJSON() ([]byte, error) {
	return json.Marshal(a.String())
}

// UnmarshalJSON deserializes a JSON string into AgentId.
func (a *AgentId) UnmarshalJSON(data []byte) error {
	var str string
	if err := json.Unmarshal(data, &str); err != nil {
		return err
	}
	parsed, err := ParseAgentId(str)
	if err != nil {
		return err
	}
	*a = parsed
	return nil
}

// ParseAgentId parses a string in the format "agent-{uuid}" and returns an AgentId.
func ParseAgentId(s string) (AgentId, error) {
	if !strings.HasPrefix(s, "agent-") {
		return AgentId{}, fmt.Errorf("invalid agent ID format: missing 'agent-' prefix in %q", s)
	}
	uuidStr := strings.TrimPrefix(s, "agent-")
	if uuidStr == "" {
		return AgentId{}, fmt.Errorf("invalid agent ID format: empty UUID in %q", s)
	}
	id, err := uuid.Parse(uuidStr)
	if err != nil {
		return AgentId{}, fmt.Errorf("invalid agent ID format: %w", err)
	}
	return AgentId{id: id}, nil
}

// ---------------------------------------------------------------------------
// SessionId
// ---------------------------------------------------------------------------

// SessionId uniquely identifies a session. It wraps a UUID v4.
type SessionId struct {
	id uuid.UUID
}

// NewSessionId generates a new random SessionId.
func NewSessionId() SessionId {
	return SessionId{id: uuid.New()}
}

// String returns the session ID in the format "session-{uuid}".
func (s SessionId) String() string {
	return "session-" + s.id.String()
}

// MarshalJSON serializes SessionId as a quoted JSON string.
func (s SessionId) MarshalJSON() ([]byte, error) {
	return json.Marshal(s.String())
}

// UnmarshalJSON deserializes a JSON string into SessionId.
func (s *SessionId) UnmarshalJSON(data []byte) error {
	var str string
	if err := json.Unmarshal(data, &str); err != nil {
		return err
	}
	parsed, err := ParseSessionId(str)
	if err != nil {
		return err
	}
	*s = parsed
	return nil
}

// ParseSessionId parses a string in the format "session-{uuid}" and returns a SessionId.
func ParseSessionId(str string) (SessionId, error) {
	if !strings.HasPrefix(str, "session-") {
		return SessionId{}, fmt.Errorf("invalid session ID format: missing 'session-' prefix in %q", str)
	}
	uuidStr := strings.TrimPrefix(str, "session-")
	if uuidStr == "" {
		return SessionId{}, fmt.Errorf("invalid session ID format: empty UUID in %q", str)
	}
	id, err := uuid.Parse(uuidStr)
	if err != nil {
		return SessionId{}, fmt.Errorf("invalid session ID format: %w", err)
	}
	return SessionId{id: id}, nil
}

// ---------------------------------------------------------------------------
// ToolName
// ---------------------------------------------------------------------------

// ToolName wraps a string identifying a tool.
type ToolName struct {
	name string
}

// NewToolName creates a ToolName from the given string.
func NewToolName(name string) ToolName {
	return ToolName{name: name}
}

// String returns the tool name as a string.
func (t ToolName) String() string {
	return t.name
}

// MarshalJSON serializes ToolName as a plain JSON string.
func (t ToolName) MarshalJSON() ([]byte, error) {
	return json.Marshal(t.name)
}

// UnmarshalJSON deserializes a JSON string into ToolName.
func (t *ToolName) UnmarshalJSON(data []byte) error {
	return json.Unmarshal(data, &t.name)
}

// ---------------------------------------------------------------------------
// TokenUsage
// ---------------------------------------------------------------------------

// TokenUsage tracks token consumption for a single LLM call.
type TokenUsage struct {
	PromptTokens     uint64 `json:"prompt_tokens"`
	CompletionTokens uint64 `json:"completion_tokens"`
	TotalTokens      uint64 `json:"total_tokens"`
}

// NewTokenUsage creates a TokenUsage with the given prompt and completion token
// counts. TotalTokens is computed automatically.
func NewTokenUsage(prompt, completion uint64) TokenUsage {
	return TokenUsage{
		PromptTokens:     prompt,
		CompletionTokens: completion,
		TotalTokens:      prompt + completion,
	}
}

// Accumulate adds the values from other into this TokenUsage.
func (t *TokenUsage) Accumulate(other TokenUsage) {
	t.PromptTokens += other.PromptTokens
	t.CompletionTokens += other.CompletionTokens
	t.TotalTokens += other.TotalTokens
}

// IsEmpty returns true when TotalTokens is zero.
func (t TokenUsage) IsEmpty() bool {
	return t.TotalTokens == 0
}

// ---------------------------------------------------------------------------
// AgentRole (iota enum)
// ---------------------------------------------------------------------------

// AgentRole represents the role an agent plays in the system.
type AgentRole int

const (
	RoleCoder    AgentRole = iota
	RoleReviewer
	RolePlanner
	RoleExecutor
	RoleObserver
)

func (r AgentRole) String() string {
	switch r {
	case RoleCoder:
		return "coder"
	case RoleReviewer:
		return "reviewer"
	case RolePlanner:
		return "planner"
	case RoleExecutor:
		return "executor"
	case RoleObserver:
		return "observer"
	default:
		return fmt.Sprintf("unknown-agent-role(%d)", r)
	}
}

// MarshalJSON returns the role as a quoted JSON string.
func (r AgentRole) MarshalJSON() ([]byte, error) {
	return json.Marshal(r.String())
}

// UnmarshalJSON parses a quoted JSON string into an AgentRole.
func (r *AgentRole) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}
	switch s {
	case "coder":
		*r = RoleCoder
	case "reviewer":
		*r = RoleReviewer
	case "planner":
		*r = RolePlanner
	case "executor":
		*r = RoleExecutor
	case "observer":
		*r = RoleObserver
	default:
		return fmt.Errorf("unknown agent role: %q", s)
	}
	return nil
}

// ---------------------------------------------------------------------------
// AgentStatus (iota enum)
// ---------------------------------------------------------------------------

// AgentStatus represents the current status of an agent.
type AgentStatus int

const (
	StatusIdle      AgentStatus = iota
	StatusRunning
	StatusWaiting
	StatusCompleted
	StatusFailed
)

func (s AgentStatus) String() string {
	switch s {
	case StatusIdle:
		return "idle"
	case StatusRunning:
		return "running"
	case StatusWaiting:
		return "waiting"
	case StatusCompleted:
		return "completed"
	case StatusFailed:
		return "failed"
	default:
		return fmt.Sprintf("unknown-agent-status(%d)", s)
	}
}

// IsTerminal returns true for Completed and Failed statuses.
func (s AgentStatus) IsTerminal() bool {
	return s == StatusCompleted || s == StatusFailed
}

// IsActive returns true for Running and Waiting statuses.
func (s AgentStatus) IsActive() bool {
	return s == StatusRunning || s == StatusWaiting
}

// MarshalJSON returns the status as a quoted JSON string.
func (s AgentStatus) MarshalJSON() ([]byte, error) {
	return json.Marshal(s.String())
}

// UnmarshalJSON parses a quoted JSON string into an AgentStatus.
func (s *AgentStatus) UnmarshalJSON(data []byte) error {
	var str string
	if err := json.Unmarshal(data, &str); err != nil {
		return err
	}
	switch str {
	case "idle":
		*s = StatusIdle
	case "running":
		*s = StatusRunning
	case "waiting":
		*s = StatusWaiting
	case "completed":
		*s = StatusCompleted
	case "failed":
		*s = StatusFailed
	default:
		return fmt.Errorf("unknown agent status: %q", str)
	}
	return nil
}

// ---------------------------------------------------------------------------
// Role (iota enum) — message role
// ---------------------------------------------------------------------------

// Role represents the role of a message sender in a conversation.
type Role int

const (
	RoleSystem    Role = iota
	RoleUser
	RoleAssistant
	RoleTool
)

func (r Role) String() string {
	switch r {
	case RoleSystem:
		return "system"
	case RoleUser:
		return "user"
	case RoleAssistant:
		return "assistant"
	case RoleTool:
		return "tool"
	default:
		return fmt.Sprintf("unknown-role(%d)", r)
	}
}

// MarshalJSON returns the role as a quoted JSON string.
func (r Role) MarshalJSON() ([]byte, error) {
	return json.Marshal(r.String())
}

// UnmarshalJSON parses a quoted JSON string into a Role.
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

// ---------------------------------------------------------------------------
// AgentCapability
// ---------------------------------------------------------------------------

// AgentCapability describes a capability that an agent possesses,
// including which tools it supports.
type AgentCapability struct {
	Name           string     `json:"name"`
	Description    string     `json:"description"`
	SupportedTools []ToolName `json:"supported_tools"`
}

// SupportsTool checks whether this capability supports the given tool.
func (c AgentCapability) SupportsTool(name ToolName) bool {
	for _, t := range c.SupportedTools {
		if t == name {
			return true
		}
	}
	return false
}
