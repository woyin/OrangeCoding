package core

import (
	"encoding/json"
	"strings"
	"testing"
)

// --- AgentId ---

func TestNewAgentId(t *testing.T) {
	id := NewAgentId()
	s := id.String()
	if !strings.HasPrefix(s, "agent-") {
		t.Fatalf("AgentId.String() = %q, want prefix %q", s, "agent-")
	}
	uuidPart := strings.TrimPrefix(s, "agent-")
	if len(uuidPart) != 36 { // standard UUID format: 8-4-4-4-12
		t.Fatalf("UUID part = %q, want 36 chars", uuidPart)
	}
}

func TestAgentIdStringFormat(t *testing.T) {
	id := NewAgentId()
	s := id.String()
	if s == "" {
		t.Fatal("AgentId.String() returned empty string")
	}
	parts := strings.SplitN(s, "-", 2)
	if len(parts) != 2 || parts[0] != "agent" {
		t.Fatalf("AgentId.String() = %q, want format %q", s, "agent-{uuid}")
	}
}

func TestAgentIdRoundTrip(t *testing.T) {
	original := NewAgentId()
	s := original.String()
	parsed, err := ParseAgentId(s)
	if err != nil {
		t.Fatalf("ParseAgentId(%q) error: %v", s, err)
	}
	if parsed != original {
		t.Fatalf("round-trip failed: parsed = %v, original = %v", parsed, original)
	}
}

func TestParseAgentIdInvalid(t *testing.T) {
	tests := []string{
		"",
		"session-550e8400-e29b-41d4-a716-446655440000", // wrong prefix
		"agent-",                                      // missing UUID
		"agent-not-a-uuid",                            // invalid UUID
		"550e8400-e29b-41d4-a716-446655440000",       // missing prefix
	}
	for _, input := range tests {
		_, err := ParseAgentId(input)
		if err == nil {
			t.Errorf("ParseAgentId(%q) expected error, got nil", input)
		}
	}
}

func TestParseAgentIdValid(t *testing.T) {
	input := "agent-550e8400-e29b-41d4-a716-446655440000"
	id, err := ParseAgentId(input)
	if err != nil {
		t.Fatalf("ParseAgentId(%q) unexpected error: %v", input, err)
	}
	if id.String() != input {
		t.Fatalf("ParseAgentId(%q) = %q, want %q", input, id.String(), input)
	}
}

// --- SessionId ---

func TestNewSessionId(t *testing.T) {
	id := NewSessionId()
	s := id.String()
	if !strings.HasPrefix(s, "session-") {
		t.Fatalf("SessionId.String() = %q, want prefix %q", s, "session-")
	}
	uuidPart := strings.TrimPrefix(s, "session-")
	if len(uuidPart) != 36 {
		t.Fatalf("UUID part = %q, want 36 chars", uuidPart)
	}
}

func TestSessionIdStringFormat(t *testing.T) {
	id := NewSessionId()
	s := id.String()
	parts := strings.SplitN(s, "-", 2)
	if len(parts) != 2 || parts[0] != "session" {
		t.Fatalf("SessionId.String() = %q, want format %q", s, "session-{uuid}")
	}
}

func TestSessionIdRoundTrip(t *testing.T) {
	original := NewSessionId()
	s := original.String()
	parsed, err := ParseSessionId(s)
	if err != nil {
		t.Fatalf("ParseSessionId(%q) error: %v", s, err)
	}
	if parsed != original {
		t.Fatalf("round-trip failed: parsed = %v, original = %v", parsed, original)
	}
}

func TestParseSessionIdInvalid(t *testing.T) {
	tests := []string{
		"",
		"agent-550e8400-e29b-41d4-a716-446655440000", // wrong prefix
		"session-",                                   // missing UUID
		"session-not-a-uuid",                         // invalid UUID
		"550e8400-e29b-41d4-a716-446655440000",      // missing prefix
	}
	for _, input := range tests {
		_, err := ParseSessionId(input)
		if err == nil {
			t.Errorf("ParseSessionId(%q) expected error, got nil", input)
		}
	}
}

func TestParseSessionIdValid(t *testing.T) {
	input := "session-550e8400-e29b-41d4-a716-446655440000"
	id, err := ParseSessionId(input)
	if err != nil {
		t.Fatalf("ParseSessionId(%q) unexpected error: %v", input, err)
	}
	if id.String() != input {
		t.Fatalf("ParseSessionId(%q) = %q, want %q", input, id.String(), input)
	}
}

// --- ToolName ---

func TestNewToolName(t *testing.T) {
	name := NewToolName("bash")
	if name.String() != "bash" {
		t.Fatalf("ToolName.String() = %q, want %q", name.String(), "bash")
	}
}

func TestToolNameEmpty(t *testing.T) {
	name := NewToolName("")
	if name.String() != "" {
		t.Fatalf("empty ToolName.String() = %q, want %q", name.String(), "")
	}
}

// --- TokenUsage ---

func TestNewTokenUsage(t *testing.T) {
	tu := NewTokenUsage(100, 50)
	if tu.PromptTokens != 100 {
		t.Errorf("PromptTokens = %d, want 100", tu.PromptTokens)
	}
	if tu.CompletionTokens != 50 {
		t.Errorf("CompletionTokens = %d, want 50", tu.CompletionTokens)
	}
	if tu.TotalTokens != 150 {
		t.Errorf("TotalTokens = %d, want 150", tu.TotalTokens)
	}
}

func TestTokenUsageZero(t *testing.T) {
	tu := NewTokenUsage(0, 0)
	if !tu.IsEmpty() {
		t.Error("zero TokenUsage should be empty")
	}
	if tu.TotalTokens != 0 {
		t.Errorf("TotalTokens = %d, want 0", tu.TotalTokens)
	}
}

func TestTokenUsageIsNotEmpty(t *testing.T) {
	tu := NewTokenUsage(1, 0)
	if tu.IsEmpty() {
		t.Error("TokenUsage{1,0} should not be empty")
	}
}

func TestTokenUsageAccumulate(t *testing.T) {
	tu := NewTokenUsage(100, 50)
	other := NewTokenUsage(50, 25)
	tu.Accumulate(other)
	if tu.PromptTokens != 150 {
		t.Errorf("after Accumulate, PromptTokens = %d, want 150", tu.PromptTokens)
	}
	if tu.CompletionTokens != 75 {
		t.Errorf("after Accumulate, CompletionTokens = %d, want 75", tu.CompletionTokens)
	}
	if tu.TotalTokens != 225 {
		t.Errorf("after Accumulate, TotalTokens = %d, want 225", tu.TotalTokens)
	}
}

func TestTokenUsageAccumulateZero(t *testing.T) {
	tu := NewTokenUsage(100, 50)
	zero := NewTokenUsage(0, 0)
	tu.Accumulate(zero)
	if tu.TotalTokens != 150 {
		t.Errorf("after Accumulate zero, TotalTokens = %d, want 150", tu.TotalTokens)
	}
}

// --- AgentRole ---

func TestAgentRoleValues(t *testing.T) {
	roles := []AgentRole{RoleCoder, RoleReviewer, RolePlanner, RoleExecutor, RoleObserver}
	seen := make(map[AgentRole]bool)
	for _, r := range roles {
		if seen[r] {
			t.Errorf("duplicate AgentRole value: %d", r)
		}
		seen[r] = true
	}
	if len(seen) != 5 {
		t.Errorf("expected 5 distinct AgentRole values, got %d", len(seen))
	}
}

func TestAgentRoleString(t *testing.T) {
	tests := []struct {
		role AgentRole
		want string
	}{
		{RoleCoder, "coder"},
		{RoleReviewer, "reviewer"},
		{RolePlanner, "planner"},
		{RoleExecutor, "executor"},
		{RoleObserver, "observer"},
	}
	for _, tt := range tests {
		if got := tt.role.String(); got != tt.want {
			t.Errorf("AgentRole(%d).String() = %q, want %q", tt.role, got, tt.want)
		}
	}
}

func TestAgentRoleMarshalJSON(t *testing.T) {
	data, err := json.Marshal(RoleCoder)
	if err != nil {
		t.Fatalf("Marshal(RoleCoder) error: %v", err)
	}
	if string(data) != `"coder"` {
		t.Errorf("Marshal(RoleCoder) = %s, want %q", data, `"coder"`)
	}
}

// --- AgentStatus ---

func TestAgentStatusIsTerminal(t *testing.T) {
	tests := []struct {
		status AgentStatus
		want   bool
	}{
		{StatusIdle, false},
		{StatusRunning, false},
		{StatusWaiting, false},
		{StatusCompleted, true},
		{StatusFailed, true},
	}
	for _, tt := range tests {
		if got := tt.status.IsTerminal(); got != tt.want {
			t.Errorf("AgentStatus(%d).IsTerminal() = %v, want %v", tt.status, got, tt.want)
		}
	}
}

func TestAgentStatusIsActive(t *testing.T) {
	tests := []struct {
		status AgentStatus
		want   bool
	}{
		{StatusIdle, false},
		{StatusRunning, true},
		{StatusWaiting, true},
		{StatusCompleted, false},
		{StatusFailed, false},
	}
	for _, tt := range tests {
		if got := tt.status.IsActive(); got != tt.want {
			t.Errorf("AgentStatus(%d).IsActive() = %v, want %v", tt.status, got, tt.want)
		}
	}
}

func TestAgentStatusString(t *testing.T) {
	tests := []struct {
		status AgentStatus
		want   string
	}{
		{StatusIdle, "idle"},
		{StatusRunning, "running"},
		{StatusWaiting, "waiting"},
		{StatusCompleted, "completed"},
		{StatusFailed, "failed"},
	}
	for _, tt := range tests {
		if got := tt.status.String(); got != tt.want {
			t.Errorf("AgentStatus(%d).String() = %q, want %q", tt.status, got, tt.want)
		}
	}
}

// --- Role ---

func TestRoleString(t *testing.T) {
	tests := []struct {
		role Role
		want string
	}{
		{RoleSystem, "system"},
		{RoleUser, "user"},
		{RoleAssistant, "assistant"},
		{RoleTool, "tool"},
	}
	for _, tt := range tests {
		if got := tt.role.String(); got != tt.want {
			t.Errorf("Role(%d).String() = %q, want %q", tt.role, got, tt.want)
		}
	}
}

func TestRoleJSONRoundTrip(t *testing.T) {
	roles := []Role{RoleSystem, RoleUser, RoleAssistant, RoleTool}
	for _, r := range roles {
		data, err := json.Marshal(r)
		if err != nil {
			t.Fatalf("Marshal(%v) error: %v", r, err)
		}
		var got Role
		if err := json.Unmarshal(data, &got); err != nil {
			t.Fatalf("Unmarshal(%s) error: %v", data, err)
		}
		if got != r {
			t.Errorf("round-trip: got %v, want %v", got, r)
		}
	}
}

func TestRoleUnmarshalJSONInvalid(t *testing.T) {
	var r Role
	err := json.Unmarshal([]byte(`"invalid"`), &r)
	if err == nil {
		t.Error("expected error for invalid role string, got nil")
	}
}

func TestRoleMarshalJSONFormat(t *testing.T) {
	data, err := json.Marshal(RoleAssistant)
	if err != nil {
		t.Fatalf("Marshal(RoleAssistant) error: %v", err)
	}
	if string(data) != `"assistant"` {
		t.Errorf("Marshal(RoleAssistant) = %s, want %q", data, `"assistant"`)
	}
}

// --- AgentCapability ---

func TestAgentCapabilitySupportsTool(t *testing.T) {
	cap := AgentCapability{
		Name:        "code-generation",
		Description: "Can generate code",
		SupportedTools: []ToolName{
			NewToolName("bash"),
			NewToolName("edit"),
		},
	}
	if !cap.SupportsTool(NewToolName("bash")) {
		t.Error("expected SupportsTool(bash) = true")
	}
	if !cap.SupportsTool(NewToolName("edit")) {
		t.Error("expected SupportsTool(edit) = true")
	}
	if cap.SupportsTool(NewToolName("unknown")) {
		t.Error("expected SupportsTool(unknown) = false")
	}
}

func TestAgentCapabilityEmptyTools(t *testing.T) {
	cap := AgentCapability{
		Name:            "read-only",
		Description:     "Read-only capability",
		SupportedTools:  []ToolName{},
	}
	if cap.SupportsTool(NewToolName("bash")) {
		t.Error("expected SupportsTool(bash) = false for empty tools")
	}
}

// --- AgentId uniqueness ---

func TestAgentIdUniqueness(t *testing.T) {
	id1 := NewAgentId()
	id2 := NewAgentId()
	if id1 == id2 {
		t.Error("two generated AgentIds should be distinct")
	}
}

func TestSessionIdUniqueness(t *testing.T) {
	id1 := NewSessionId()
	id2 := NewSessionId()
	if id1 == id2 {
		t.Error("two generated SessionIds should be distinct")
	}
}
