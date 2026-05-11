package controlprotocol

import (
	"encoding/json"
	"testing"
)

func TestSendTaskCommandType(t *testing.T) {
	cmd := &SendTaskCommand{SessionID: "s1", Task: "do something"}
	if got := cmd.CommandType(); got != "send_task" {
		t.Errorf("SendTaskCommand.CommandType() = %q, want %q", got, "send_task")
	}
}

func TestApproveCommandType(t *testing.T) {
	cmd := &ApproveCommand{RequestID: "r1", Approved: true}
	if got := cmd.CommandType(); got != "approve" {
		t.Errorf("ApproveCommand.CommandType() = %q, want %q", got, "approve")
	}
}

func TestCancelCommandType(t *testing.T) {
	cmd := &CancelCommand{SessionID: "s1"}
	if got := cmd.CommandType(); got != "cancel" {
		t.Errorf("CancelCommand.CommandType() = %q, want %q", got, "cancel")
	}
}

func TestTaskUpdateEventType(t *testing.T) {
	ev := &TaskUpdateEvent{SessionID: "s1", Status: "running", Message: "working"}
	if got := ev.EventType(); got != "task_update" {
		t.Errorf("TaskUpdateEvent.EventType() = %q, want %q", got, "task_update")
	}
}

func TestToolCallEventType(t *testing.T) {
	ev := &ToolCallEvent{SessionID: "s1", ToolName: "bash", Input: "ls", Output: "files", IsError: false}
	if got := ev.EventType(); got != "tool_call" {
		t.Errorf("ToolCallEvent.EventType() = %q, want %q", got, "tool_call")
	}
}

func TestApprovalRequestEventType(t *testing.T) {
	ev := &ApprovalRequestEvent{RequestID: "r1", ToolName: "bash", Input: "rm -rf /", Message: "dangerous"}
	if got := ev.EventType(); got != "approval_request" {
		t.Errorf("ApprovalRequestEvent.EventType() = %q, want %q", got, "approval_request")
	}
}

func TestErrorEventType(t *testing.T) {
	ev := &ErrorEvent{SessionID: "s1", Error: "something went wrong"}
	if got := ev.EventType(); got != "error" {
		t.Errorf("ErrorEvent.EventType() = %q, want %q", got, "error")
	}
}

// JSON round-trip tests for ClientCommands

func TestSendTaskCommandJSONRoundTrip(t *testing.T) {
	original := &SendTaskCommand{SessionID: "sess-123", Task: "write a hello world"}

	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("Marshal SendTaskCommand: %v", err)
	}

	var decoded SendTaskCommand
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal SendTaskCommand: %v", err)
	}

	if decoded.SessionID != original.SessionID {
		t.Errorf("SessionID = %q, want %q", decoded.SessionID, original.SessionID)
	}
	if decoded.Task != original.Task {
		t.Errorf("Task = %q, want %q", decoded.Task, original.Task)
	}
}

func TestApproveCommandJSONRoundTrip(t *testing.T) {
	original := &ApproveCommand{RequestID: "req-456", Approved: true}

	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("Marshal ApproveCommand: %v", err)
	}

	var decoded ApproveCommand
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal ApproveCommand: %v", err)
	}

	if decoded.RequestID != original.RequestID {
		t.Errorf("RequestID = %q, want %q", decoded.RequestID, original.RequestID)
	}
	if decoded.Approved != original.Approved {
		t.Errorf("Approved = %v, want %v", decoded.Approved, original.Approved)
	}
}

func TestCancelCommandJSONRoundTrip(t *testing.T) {
	original := &CancelCommand{SessionID: "sess-789"}

	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("Marshal CancelCommand: %v", err)
	}

	var decoded CancelCommand
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal CancelCommand: %v", err)
	}

	if decoded.SessionID != original.SessionID {
		t.Errorf("SessionID = %q, want %q", decoded.SessionID, original.SessionID)
	}
}

// JSON round-trip tests for ServerEvents

func TestTaskUpdateEventJSONRoundTrip(t *testing.T) {
	original := &TaskUpdateEvent{SessionID: "s1", Status: "completed", Message: "done"}

	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("Marshal TaskUpdateEvent: %v", err)
	}

	var decoded TaskUpdateEvent
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal TaskUpdateEvent: %v", err)
	}

	if decoded.SessionID != original.SessionID {
		t.Errorf("SessionID = %q, want %q", decoded.SessionID, original.SessionID)
	}
	if decoded.Status != original.Status {
		t.Errorf("Status = %q, want %q", decoded.Status, original.Status)
	}
	if decoded.Message != original.Message {
		t.Errorf("Message = %q, want %q", decoded.Message, original.Message)
	}
}

func TestToolCallEventJSONRoundTrip(t *testing.T) {
	original := &ToolCallEvent{
		SessionID: "s1",
		ToolName:  "bash",
		Input:     "echo hello",
		Output:    "hello\n",
		IsError:   true,
	}

	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("Marshal ToolCallEvent: %v", err)
	}

	var decoded ToolCallEvent
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal ToolCallEvent: %v", err)
	}

	if decoded.SessionID != original.SessionID {
		t.Errorf("SessionID = %q, want %q", decoded.SessionID, original.SessionID)
	}
	if decoded.ToolName != original.ToolName {
		t.Errorf("ToolName = %q, want %q", decoded.ToolName, original.ToolName)
	}
	if decoded.Input != original.Input {
		t.Errorf("Input = %q, want %q", decoded.Input, original.Input)
	}
	if decoded.Output != original.Output {
		t.Errorf("Output = %q, want %q", decoded.Output, original.Output)
	}
	if decoded.IsError != original.IsError {
		t.Errorf("IsError = %v, want %v", decoded.IsError, original.IsError)
	}
}

func TestApprovalRequestEventJSONRoundTrip(t *testing.T) {
	original := &ApprovalRequestEvent{
		RequestID: "r1",
		ToolName:  "file_write",
		Input:     "/etc/passwd",
		Message:   "writes to system file",
	}

	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("Marshal ApprovalRequestEvent: %v", err)
	}

	var decoded ApprovalRequestEvent
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal ApprovalRequestEvent: %v", err)
	}

	if decoded.RequestID != original.RequestID {
		t.Errorf("RequestID = %q, want %q", decoded.RequestID, original.RequestID)
	}
	if decoded.ToolName != original.ToolName {
		t.Errorf("ToolName = %q, want %q", decoded.ToolName, original.ToolName)
	}
	if decoded.Input != original.Input {
		t.Errorf("Input = %q, want %q", decoded.Input, original.Input)
	}
	if decoded.Message != original.Message {
		t.Errorf("Message = %q, want %q", decoded.Message, original.Message)
	}
}

func TestErrorEventJSONRoundTrip(t *testing.T) {
	original := &ErrorEvent{SessionID: "s1", Error: "timeout exceeded"}

	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("Marshal ErrorEvent: %v", err)
	}

	var decoded ErrorEvent
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal ErrorEvent: %v", err)
	}

	if decoded.SessionID != original.SessionID {
		t.Errorf("SessionID = %q, want %q", decoded.SessionID, original.SessionID)
	}
	if decoded.Error != original.Error {
		t.Errorf("Error = %q, want %q", decoded.Error, original.Error)
	}
}

// JSON field name tests

func TestSendTaskCommandJSONKeys(t *testing.T) {
	cmd := &SendTaskCommand{SessionID: "s", Task: "t"}
	data, err := json.Marshal(cmd)
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}

	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatalf("Unmarshal to map: %v", err)
	}

	expectedKeys := []string{"session_id", "task"}
	for _, key := range expectedKeys {
		if _, ok := raw[key]; !ok {
			t.Errorf("missing JSON key %q in %s", key, string(data))
		}
	}
}

func TestApproveCommandJSONKeys(t *testing.T) {
	cmd := &ApproveCommand{RequestID: "r", Approved: true}
	data, err := json.Marshal(cmd)
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}

	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatalf("Unmarshal to map: %v", err)
	}

	expectedKeys := []string{"request_id", "approved"}
	for _, key := range expectedKeys {
		if _, ok := raw[key]; !ok {
			t.Errorf("missing JSON key %q in %s", key, string(data))
		}
	}
}

func TestToolCallEventJSONKeys(t *testing.T) {
	ev := &ToolCallEvent{SessionID: "s", ToolName: "t", Input: "i", Output: "o", IsError: false}
	data, err := json.Marshal(ev)
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}

	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatalf("Unmarshal to map: %v", err)
	}

	expectedKeys := []string{"session_id", "tool_name", "input", "output", "is_error"}
	for _, key := range expectedKeys {
		if _, ok := raw[key]; !ok {
			t.Errorf("missing JSON key %q in %s", key, string(data))
		}
	}
}

func TestApprovalRequestEventJSONKeys(t *testing.T) {
	ev := &ApprovalRequestEvent{RequestID: "r", ToolName: "t", Input: "i", Message: "m"}
	data, err := json.Marshal(ev)
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}

	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatalf("Unmarshal to map: %v", err)
	}

	expectedKeys := []string{"request_id", "tool_name", "input", "message"}
	for _, key := range expectedKeys {
		if _, ok := raw[key]; !ok {
			t.Errorf("missing JSON key %q in %s", key, string(data))
		}
	}
}

// Interface compliance tests

func TestClientCommandInterface(t *testing.T) {
	var cmd ClientCommand
	cmd = &SendTaskCommand{}
	if cmd.CommandType() != "send_task" {
		t.Errorf("SendTaskCommand does not satisfy ClientCommand interface correctly")
	}
	cmd = &ApproveCommand{}
	if cmd.CommandType() != "approve" {
		t.Errorf("ApproveCommand does not satisfy ClientCommand interface correctly")
	}
	cmd = &CancelCommand{}
	if cmd.CommandType() != "cancel" {
		t.Errorf("CancelCommand does not satisfy ClientCommand interface correctly")
	}
}

func TestServerEventInterface(t *testing.T) {
	var ev ServerEvent
	ev = &TaskUpdateEvent{}
	if ev.EventType() != "task_update" {
		t.Errorf("TaskUpdateEvent does not satisfy ServerEvent interface correctly")
	}
	ev = &ToolCallEvent{}
	if ev.EventType() != "tool_call" {
		t.Errorf("ToolCallEvent does not satisfy ServerEvent interface correctly")
	}
	ev = &ApprovalRequestEvent{}
	if ev.EventType() != "approval_request" {
		t.Errorf("ApprovalRequestEvent does not satisfy ServerEvent interface correctly")
	}
	ev = &ErrorEvent{}
	if ev.EventType() != "error" {
		t.Errorf("ErrorEvent does not satisfy ServerEvent interface correctly")
	}
}
