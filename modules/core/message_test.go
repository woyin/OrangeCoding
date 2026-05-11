package core

import (
	"encoding/json"
	"testing"
	"time"
)

// ---------------------------------------------------------------------------
// Message constructors
// ---------------------------------------------------------------------------

func TestNewSystemMessage(t *testing.T) {
	msg := NewSystemMessage("you are a helpful assistant")
	if msg.Role != RoleSystem {
		t.Errorf("expected RoleSystem, got %v", msg.Role)
	}
	if msg.Content != "you are a helpful assistant" {
		t.Errorf("expected content %q, got %q", "you are a helpful assistant", msg.Content)
	}
	if msg.CreatedAt.IsZero() {
		t.Error("expected CreatedAt to be set")
	}
	if msg.CreatedAt.Location() != time.UTC {
		t.Errorf("expected UTC, got %v", msg.CreatedAt.Location())
	}
	if msg.HasToolCalls() {
		t.Error("expected no tool calls on system message")
	}
}

func TestNewUserMessage(t *testing.T) {
	msg := NewUserMessage("hello world")
	if msg.Role != RoleUser {
		t.Errorf("expected RoleUser, got %v", msg.Role)
	}
	if msg.Content != "hello world" {
		t.Errorf("expected content %q, got %q", "hello world", msg.Content)
	}
	if msg.CreatedAt.IsZero() {
		t.Error("expected CreatedAt to be set")
	}
}

func TestNewAssistantMessage(t *testing.T) {
	msg := NewAssistantMessage("I can help with that")
	if msg.Role != RoleAssistant {
		t.Errorf("expected RoleAssistant, got %v", msg.Role)
	}
	if msg.Content != "I can help with that" {
		t.Errorf("expected content %q, got %q", "I can help with that", msg.Content)
	}
	if msg.HasToolCalls() {
		t.Error("expected no tool calls")
	}
	if len(msg.ToolCalls) != 0 {
		t.Errorf("expected empty ToolCalls, got %d", len(msg.ToolCalls))
	}
}

func TestNewAssistantMessageWithToolCalls(t *testing.T) {
	tcs := []ToolCall{
		{ID: "call_1", FunctionName: "read_file", Arguments: json.RawMessage(`{"path":"/tmp/a"}`)},
		{ID: "call_2", FunctionName: "write_file", Arguments: json.RawMessage(`{"path":"/tmp/b"}`)},
	}
	msg := NewAssistantMessageWithToolCalls("let me check", tcs)
	if msg.Role != RoleAssistant {
		t.Errorf("expected RoleAssistant, got %v", msg.Role)
	}
	if msg.Content != "let me check" {
		t.Errorf("expected content %q, got %q", "let me check", msg.Content)
	}
	if !msg.HasToolCalls() {
		t.Error("expected HasToolCalls to be true")
	}
	if len(msg.ToolCalls) != 2 {
		t.Errorf("expected 2 tool calls, got %d", len(msg.ToolCalls))
	}
	if msg.ToolCalls[0].ID != "call_1" {
		t.Errorf("expected tool call ID %q, got %q", "call_1", msg.ToolCalls[0].ID)
	}
	if msg.ToolCalls[1].FunctionName != "write_file" {
		t.Errorf("expected function name %q, got %q", "write_file", msg.ToolCalls[1].FunctionName)
	}
}

func TestNewAssistantMessageWithToolCallsEmpty(t *testing.T) {
	msg := NewAssistantMessageWithToolCalls("", nil)
	if msg.HasToolCalls() {
		t.Error("expected HasToolCalls to be false with nil slice")
	}
}

func TestNewToolResultMessage(t *testing.T) {
	msg := NewToolResultMessage("call_42", "file contents here", false)
	if msg.Role != RoleTool {
		t.Errorf("expected RoleTool, got %v", msg.Role)
	}
	if msg.Content != "file contents here" {
		t.Errorf("expected content %q, got %q", "file contents here", msg.Content)
	}
	if msg.ToolCallID != "call_42" {
		t.Errorf("expected tool_call_id %q, got %q", "call_42", msg.ToolCallID)
	}
}

func TestNewToolResultMessageWithError(t *testing.T) {
	msg := NewToolResultMessage("call_99", "something went wrong", true)
	if msg.Role != RoleTool {
		t.Errorf("expected RoleTool, got %v", msg.Role)
	}
	if msg.Content != "something went wrong" {
		t.Errorf("expected content %q, got %q", "something went wrong", msg.Content)
	}
}

// ---------------------------------------------------------------------------
// HasToolCalls
// ---------------------------------------------------------------------------

func TestHasToolCalls(t *testing.T) {
	tests := []struct {
		name     string
		msg      Message
		expected bool
	}{
		{
			name:     "no tool calls",
			msg:      Message{Role: RoleAssistant, Content: "hi"},
			expected: false,
		},
		{
			name:     "nil tool calls",
			msg:      Message{Role: RoleAssistant, Content: "hi", ToolCalls: nil},
			expected: false,
		},
		{
			name:     "empty tool calls",
			msg:      Message{Role: RoleAssistant, Content: "hi", ToolCalls: []ToolCall{}},
			expected: false,
		},
		{
			name: "has tool calls",
			msg: Message{
				Role:    RoleAssistant,
				Content: "hi",
				ToolCalls: []ToolCall{
					{ID: "call_1", FunctionName: "foo", Arguments: json.RawMessage(`{}`)},
				},
			},
			expected: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.msg.HasToolCalls(); got != tt.expected {
				t.Errorf("HasToolCalls() = %v, want %v", got, tt.expected)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// ToolCall struct
// ---------------------------------------------------------------------------

func TestToolCallJSON(t *testing.T) {
	tc := ToolCall{
		ID:           "call_abc",
		FunctionName: "search",
		Arguments:    json.RawMessage(`{"query":"golang test"}`),
	}
	data, err := json.Marshal(tc)
	if err != nil {
		t.Fatalf("failed to marshal ToolCall: %v", err)
	}
	var tc2 ToolCall
	if err := json.Unmarshal(data, &tc2); err != nil {
		t.Fatalf("failed to unmarshal ToolCall: %v", err)
	}
	if tc2.ID != tc.ID {
		t.Errorf("ID mismatch: got %q, want %q", tc2.ID, tc.ID)
	}
	if tc2.FunctionName != tc.FunctionName {
		t.Errorf("FunctionName mismatch: got %q, want %q", tc2.FunctionName, tc.FunctionName)
	}
}

// ---------------------------------------------------------------------------
// ToolResult constructors
// ---------------------------------------------------------------------------

func TestNewToolResultSuccess(t *testing.T) {
	tr := NewToolResultSuccess("call_1", "ok")
	if tr.ToolCallID != "call_1" {
		t.Errorf("expected ToolCallID %q, got %q", "call_1", tr.ToolCallID)
	}
	if tr.Content != "ok" {
		t.Errorf("expected Content %q, got %q", "ok", tr.Content)
	}
	if tr.IsError {
		t.Error("expected IsError to be false for success")
	}
}

func TestNewToolResultError(t *testing.T) {
	tr := NewToolResultError("call_2", "bad input")
	if tr.ToolCallID != "call_2" {
		t.Errorf("expected ToolCallID %q, got %q", "call_2", tr.ToolCallID)
	}
	if tr.Content != "bad input" {
		t.Errorf("expected Content %q, got %q", "bad input", tr.Content)
	}
	if !tr.IsError {
		t.Error("expected IsError to be true for error")
	}
}

// ---------------------------------------------------------------------------
// ToolResult.ToMessage
// ---------------------------------------------------------------------------

func TestToolResultToMessageSuccess(t *testing.T) {
	tr := NewToolResultSuccess("call_x", "file read ok")
	msg := tr.ToMessage()
	if msg.Role != RoleTool {
		t.Errorf("expected RoleTool, got %v", msg.Role)
	}
	if msg.ToolCallID != "call_x" {
		t.Errorf("expected ToolCallID %q, got %q", "call_x", msg.ToolCallID)
	}
	if msg.Content != "file read ok" {
		t.Errorf("expected Content %q, got %q", "file read ok", msg.Content)
	}
}

func TestToolResultToMessageError(t *testing.T) {
	tr := NewToolResultError("call_y", "file not found")
	msg := tr.ToMessage()
	if msg.Role != RoleTool {
		t.Errorf("expected RoleTool, got %v", msg.Role)
	}
	if msg.ToolCallID != "call_y" {
		t.Errorf("expected ToolCallID %q, got %q", "call_y", msg.ToolCallID)
	}
	if msg.Content != "file not found" {
		t.Errorf("expected Content %q, got %q", "file not found", msg.Content)
	}
}

// ---------------------------------------------------------------------------
// Conversation
// ---------------------------------------------------------------------------

func TestNewConversation(t *testing.T) {
	conv := NewConversation()
	if conv == nil {
		t.Fatal("expected non-nil Conversation")
	}
	if !conv.IsEmpty() {
		t.Error("expected new conversation to be empty")
	}
	if conv.Len() != 0 {
		t.Errorf("expected Len 0, got %d", conv.Len())
	}
	if msgs := conv.Messages(); len(msgs) != 0 {
		t.Errorf("expected empty Messages, got %d", len(msgs))
	}
}

func TestNewConversationWithSystemPrompt(t *testing.T) {
	conv := NewConversationWithSystemPrompt("you are a coder")
	if conv.Len() != 1 {
		t.Fatalf("expected Len 1, got %d", conv.Len())
	}
	sp := conv.SystemPrompt()
	if sp == nil {
		t.Fatal("expected non-nil system prompt")
	}
	if *sp != "you are a coder" {
		t.Errorf("expected %q, got %q", "you are a coder", *sp)
	}
}

func TestConversationAddMessage(t *testing.T) {
	conv := NewConversation()
	conv.AddMessage(NewUserMessage("hello"))
	conv.AddMessage(NewAssistantMessage("hi there"))
	if conv.Len() != 2 {
		t.Errorf("expected Len 2, got %d", conv.Len())
	}
	msgs := conv.Messages()
	if msgs[0].Role != RoleUser {
		t.Errorf("expected first message RoleUser, got %v", msgs[0].Role)
	}
	if msgs[1].Role != RoleAssistant {
		t.Errorf("expected second message RoleAssistant, got %v", msgs[1].Role)
	}
}

func TestConversationIsEmpty(t *testing.T) {
	conv := NewConversation()
	if !conv.IsEmpty() {
		t.Error("expected empty")
	}
	conv.AddMessage(NewUserMessage("hi"))
	if conv.IsEmpty() {
		t.Error("expected non-empty after adding message")
	}
}

func TestConversationSystemPrompt(t *testing.T) {
	t.Run("with system prompt", func(t *testing.T) {
		conv := NewConversationWithSystemPrompt("be helpful")
		sp := conv.SystemPrompt()
		if sp == nil || *sp != "be helpful" {
			t.Errorf("expected %q, got %v", "be helpful", sp)
		}
	})

	t.Run("without system prompt - empty", func(t *testing.T) {
		conv := NewConversation()
		sp := conv.SystemPrompt()
		if sp != nil {
			t.Errorf("expected nil, got %q", *sp)
		}
	})

	t.Run("without system prompt - user first", func(t *testing.T) {
		conv := NewConversation()
		conv.AddMessage(NewUserMessage("hello"))
		sp := conv.SystemPrompt()
		if sp != nil {
			t.Errorf("expected nil when first message is not system, got %q", *sp)
		}
	})
}

func TestConversationLastMessage(t *testing.T) {
	t.Run("empty conversation", func(t *testing.T) {
		conv := NewConversation()
		if lm := conv.LastMessage(); lm != nil {
			t.Errorf("expected nil, got %+v", lm)
		}
	})

	t.Run("with messages", func(t *testing.T) {
		conv := NewConversation()
		conv.AddMessage(NewUserMessage("first"))
		conv.AddMessage(NewAssistantMessage("second"))
		conv.AddMessage(NewUserMessage("third"))
		lm := conv.LastMessage()
		if lm == nil {
			t.Fatal("expected non-nil")
		}
		if lm.Content != "third" {
			t.Errorf("expected %q, got %q", "third", lm.Content)
		}
	})
}

func TestConversationLastAssistantMessage(t *testing.T) {
	t.Run("empty", func(t *testing.T) {
		conv := NewConversation()
		if lam := conv.LastAssistantMessage(); lam != nil {
			t.Errorf("expected nil, got %+v", lam)
		}
	})

	t.Run("no assistant message", func(t *testing.T) {
		conv := NewConversation()
		conv.AddMessage(NewSystemMessage("sys"))
		conv.AddMessage(NewUserMessage("hello"))
		if lam := conv.LastAssistantMessage(); lam != nil {
			t.Errorf("expected nil, got %+v", lam)
		}
	})

	t.Run("finds last assistant", func(t *testing.T) {
		conv := NewConversation()
		conv.AddMessage(NewSystemMessage("sys"))
		conv.AddMessage(NewUserMessage("hello"))
		conv.AddMessage(NewAssistantMessage("first reply"))
		conv.AddMessage(NewUserMessage("follow up"))
		conv.AddMessage(NewAssistantMessage("second reply"))
		lam := conv.LastAssistantMessage()
		if lam == nil {
			t.Fatal("expected non-nil")
		}
		if lam.Content != "second reply" {
			t.Errorf("expected %q, got %q", "second reply", lam.Content)
		}
	})
}

func TestConversationPendingToolCalls(t *testing.T) {
	t.Run("no assistant message", func(t *testing.T) {
		conv := NewConversation()
		if ptc := conv.PendingToolCalls(); len(ptc) != 0 {
			t.Errorf("expected empty, got %d", len(ptc))
		}
	})

	t.Run("assistant without tool calls", func(t *testing.T) {
		conv := NewConversation()
		conv.AddMessage(NewAssistantMessage("just text"))
		if ptc := conv.PendingToolCalls(); len(ptc) != 0 {
			t.Errorf("expected empty, got %d", len(ptc))
		}
	})

	t.Run("assistant with tool calls", func(t *testing.T) {
		conv := NewConversation()
		tcs := []ToolCall{
			{ID: "call_1", FunctionName: "read", Arguments: json.RawMessage(`{}`)},
			{ID: "call_2", FunctionName: "write", Arguments: json.RawMessage(`{}`)},
		}
		conv.AddMessage(NewAssistantMessageWithToolCalls("", tcs))
		ptc := conv.PendingToolCalls()
		if len(ptc) != 2 {
			t.Fatalf("expected 2, got %d", len(ptc))
		}
		if ptc[0].ID != "call_1" {
			t.Errorf("expected ID %q, got %q", "call_1", ptc[0].ID)
		}
		if ptc[1].FunctionName != "write" {
			t.Errorf("expected FunctionName %q, got %q", "write", ptc[1].FunctionName)
		}
	})

	t.Run("tool calls from most recent assistant", func(t *testing.T) {
		conv := NewConversation()
		conv.AddMessage(NewAssistantMessageWithToolCalls("", []ToolCall{
			{ID: "old_call", FunctionName: "old_fn", Arguments: json.RawMessage(`{}`)},
		}))
		conv.AddMessage(NewToolResultMessage("old_call", "done", false))
		conv.AddMessage(NewAssistantMessageWithToolCalls("", []ToolCall{
			{ID: "new_call", FunctionName: "new_fn", Arguments: json.RawMessage(`{}`)},
		}))
		ptc := conv.PendingToolCalls()
		if len(ptc) != 1 {
			t.Fatalf("expected 1, got %d", len(ptc))
		}
		if ptc[0].ID != "new_call" {
			t.Errorf("expected ID %q, got %q", "new_call", ptc[0].ID)
		}
	})
}

func TestConversationClear(t *testing.T) {
	conv := NewConversationWithSystemPrompt("sys")
	conv.AddMessage(NewUserMessage("hello"))
	if conv.Len() != 2 {
		t.Fatalf("expected Len 2, got %d", conv.Len())
	}
	conv.Clear()
	if !conv.IsEmpty() {
		t.Error("expected empty after clear")
	}
	if conv.Len() != 0 {
		t.Errorf("expected Len 0 after clear, got %d", conv.Len())
	}
}

func TestConversationTokenEstimate(t *testing.T) {
	t.Run("empty", func(t *testing.T) {
		conv := NewConversation()
		if est := conv.TokenEstimate(); est != 0 {
			t.Errorf("expected 0, got %d", est)
		}
	})

	t.Run("with content", func(t *testing.T) {
		conv := NewConversation()
		// 20 chars total
		conv.AddMessage(NewUserMessage("1234567890"))     // 10 chars
		conv.AddMessage(NewAssistantMessage("1234567890")) // 10 chars
		est := conv.TokenEstimate()
		// 20 chars / 4 = 5 tokens
		if est != 5 {
			t.Errorf("expected 5, got %d", est)
		}
	})
}

// ---------------------------------------------------------------------------
// Message JSON round-trip
// ---------------------------------------------------------------------------

func TestMessageJSONRoundTrip(t *testing.T) {
	original := NewAssistantMessageWithToolCalls("thinking", []ToolCall{
		{ID: "call_1", FunctionName: "bash", Arguments: json.RawMessage(`{"cmd":"ls"}`)},
	})
	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var restored Message
	if err := json.Unmarshal(data, &restored); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if restored.Role != RoleAssistant {
		t.Errorf("role mismatch: got %v, want %v", restored.Role, RoleAssistant)
	}
	if restored.Content != "thinking" {
		t.Errorf("content mismatch: got %q, want %q", restored.Content, "thinking")
	}
	if !restored.HasToolCalls() {
		t.Error("expected tool calls after round-trip")
	}
	if restored.ToolCalls[0].FunctionName != "bash" {
		t.Errorf("function name mismatch: got %q", restored.ToolCalls[0].FunctionName)
	}
}

func TestToolResultJSONRoundTrip(t *testing.T) {
	original := NewToolResultError("call_err", "permission denied")
	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var restored ToolResult
	if err := json.Unmarshal(data, &restored); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if restored.ToolCallID != "call_err" {
		t.Errorf("ToolCallID mismatch: got %q", restored.ToolCallID)
	}
	if restored.Content != "permission denied" {
		t.Errorf("Content mismatch: got %q", restored.Content)
	}
	if !restored.IsError {
		t.Error("expected IsError to be true")
	}
}
