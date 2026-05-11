package core

import (
	"context"
	"encoding/json"
	"sync"
	"testing"
	"time"
)

// ---------------------------------------------------------------------------
// Helper: create fixed agent/session IDs for deterministic tests
// ---------------------------------------------------------------------------

func testAgentID(t *testing.T) AgentId {
	t.Helper()
	aid, err := ParseAgentId("agent-00000000-0000-0000-0000-000000000001")
	if err != nil {
		t.Fatalf("parse agent ID: %v", err)
	}
	return aid
}

func testSessionID(t *testing.T) SessionId {
	t.Helper()
	sid, err := ParseSessionId("session-00000000-0000-0000-0000-000000000002")
	if err != nil {
		t.Fatalf("parse session ID: %v", err)
	}
	return sid
}

// ---------------------------------------------------------------------------
// BaseEvent interface compliance tests for all 11 event types
// ---------------------------------------------------------------------------

// testBaseEventFields verifies the 4 AgentEvent interface methods on ev.
func testBaseEventFields(t *testing.T, ev AgentEvent, wantType string, aid AgentId, sid SessionId) {
	t.Helper()

	if ev.EventType() != wantType {
		t.Errorf("EventType() = %q, want %q", ev.EventType(), wantType)
	}
	if ev.AgentID() != aid {
		t.Errorf("AgentID() = %v, want %v", ev.AgentID(), aid)
	}
	if ev.SessionID() != sid {
		t.Errorf("SessionID() = %v, want %v", ev.SessionID(), sid)
	}
	ts := ev.Timestamp()
	if ts.IsZero() {
		t.Error("Timestamp() is zero, expected non-zero")
	}
}

func TestStartedEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewStartedEvent(aid, sid)
	testBaseEventFields(t, ev, "started", aid, sid)
}

func TestCompletedEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewCompletedEvent(aid, sid, "all done")
	testBaseEventFields(t, ev, "completed", aid, sid)
	if ev.Summary != "all done" {
		t.Errorf("Summary = %q, want %q", ev.Summary, "all done")
	}
}

func TestMessageReceivedEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewMessageReceivedEvent(aid, sid, "hello preview")
	testBaseEventFields(t, ev, "message_received", aid, sid)
	if ev.ContentPreview != "hello preview" {
		t.Errorf("ContentPreview = %q, want %q", ev.ContentPreview, "hello preview")
	}
}

func TestToolCallRequestedEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	tc := ToolCall{ID: "tc-1", FunctionName: "read_file"}
	ev := NewToolCallRequestedEvent(aid, sid, tc)
	testBaseEventFields(t, ev, "tool_call_requested", aid, sid)
	if ev.ToolCall.ID != "tc-1" {
		t.Errorf("ToolCall.ID = %q, want %q", ev.ToolCall.ID, "tc-1")
	}
}

func TestToolCallCompletedEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewToolCallCompletedEvent(aid, sid, "read_file", true, 150)
	testBaseEventFields(t, ev, "tool_call_completed", aid, sid)
	if ev.ToolName != "read_file" {
		t.Errorf("ToolName = %q, want %q", ev.ToolName, "read_file")
	}
	if ev.Success != true {
		t.Errorf("Success = %v, want true", ev.Success)
	}
	if ev.DurationMs != 150 {
		t.Errorf("DurationMs = %d, want 150", ev.DurationMs)
	}
}

func TestTokenUsageUpdatedEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	usage := NewTokenUsage(100, 50)
	ev := NewTokenUsageUpdatedEvent(aid, sid, usage)
	testBaseEventFields(t, ev, "token_usage_updated", aid, sid)
	if ev.Usage.TotalTokens != 150 {
		t.Errorf("Usage.TotalTokens = %d, want 150", ev.Usage.TotalTokens)
	}
}

func TestStreamChunkEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewStreamChunkEvent(aid, sid, "chunk-content")
	testBaseEventFields(t, ev, "stream_chunk", aid, sid)
	if ev.Content != "chunk-content" {
		t.Errorf("Content = %q, want %q", ev.Content, "chunk-content")
	}
}

func TestErrorEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewErrorEvent(aid, sid, "something broke")
	testBaseEventFields(t, ev, "error", aid, sid)
	if ev.ErrorMessage != "something broke" {
		t.Errorf("ErrorMessage = %q, want %q", ev.ErrorMessage, "something broke")
	}
}

func TestGoalPhaseChangedEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewGoalPhaseChangedEvent(aid, sid, "execution", 3)
	testBaseEventFields(t, ev, "goal_phase_changed", aid, sid)
	if ev.Phase != "execution" {
		t.Errorf("Phase = %q, want %q", ev.Phase, "execution")
	}
	if ev.Cycle != 3 {
		t.Errorf("Cycle = %d, want 3", ev.Cycle)
	}
}

func TestGoalTaskCompletedEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewGoalTaskCompletedEvent(aid, sid, "task-42", "implement feature", true)
	testBaseEventFields(t, ev, "goal_task_completed", aid, sid)
	if ev.TaskID != "task-42" {
		t.Errorf("TaskID = %q, want %q", ev.TaskID, "task-42")
	}
	if ev.TaskDescription != "implement feature" {
		t.Errorf("TaskDescription = %q, want %q", ev.TaskDescription, "implement feature")
	}
	if ev.Success != true {
		t.Errorf("Success = %v, want true", ev.Success)
	}
}

func TestGoalCycleCompleteEvent(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewGoalCycleCompleteEvent(aid, sid, 5, 10, 2, true)
	testBaseEventFields(t, ev, "goal_cycle_complete", aid, sid)
	if ev.Cycle != 5 {
		t.Errorf("Cycle = %d, want 5", ev.Cycle)
	}
	if ev.TasksCompleted != 10 {
		t.Errorf("TasksCompleted = %d, want 10", ev.TasksCompleted)
	}
	if ev.TasksFailed != 2 {
		t.Errorf("TasksFailed = %d, want 2", ev.TasksFailed)
	}
	if ev.VerificationPassed != true {
		t.Errorf("VerificationPassed = %v, want true", ev.VerificationPassed)
	}
}

// ---------------------------------------------------------------------------
// JSON round-trip test
// ---------------------------------------------------------------------------

func TestEventJSONRoundTrip(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)
	ev := NewCompletedEvent(aid, sid, "summary text")

	data, err := json.Marshal(ev)
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}

	var got CompletedEvent
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("Unmarshal: %v", err)
	}
	if got.Type != "completed" {
		t.Errorf("Type after round-trip = %q, want %q", got.Type, "completed")
	}
	if got.Summary != "summary text" {
		t.Errorf("Summary after round-trip = %q, want %q", got.Summary, "summary text")
	}
}

// ---------------------------------------------------------------------------
// EventHandlerFunc adapter
// ---------------------------------------------------------------------------

func TestEventHandlerFunc(t *testing.T) {
	aid, sid := testAgentID(t), testSessionID(t)

	var received AgentEvent
	handler := NewEventHandlerFunc("test-handler", func(ev AgentEvent) error {
		received = ev
		return nil
	})

	if handler.Name() != "test-handler" {
		t.Errorf("Name() = %q, want %q", handler.Name(), "test-handler")
	}

	ev := NewErrorEvent(aid, sid, "oops")
	if err := handler.Handle(ev); err != nil {
		t.Fatalf("Handle() returned error: %v", err)
	}
	if received.EventType() != "error" {
		t.Errorf("received EventType() = %q, want %q", received.EventType(), "error")
	}
}

// ---------------------------------------------------------------------------
// EventBus tests
// ---------------------------------------------------------------------------

func TestEventBus_SubscribePublish(t *testing.T) {
	bus := NewEventBus(16)

	aid, sid := testAgentID(t), testSessionID(t)

	var mu sync.Mutex
	var received []AgentEvent

	handler := NewEventHandlerFunc("sub1", func(ev AgentEvent) error {
		mu.Lock()
		received = append(received, ev)
		mu.Unlock()
		return nil
	})

	bus.Subscribe(handler)

	ctx := context.Background()
	ev1 := NewStartedEvent(aid, sid)
	ev2 := NewStreamChunkEvent(aid, sid, "hello")

	bus.Publish(ctx, ev1)
	bus.Publish(ctx, ev2)

	// Give goroutines time to process
	time.Sleep(50 * time.Millisecond)

	mu.Lock()
	defer mu.Unlock()
	if len(received) != 2 {
		t.Fatalf("expected 2 events, got %d", len(received))
	}
	// Order is not guaranteed because Publish dispatches via goroutines.
	// Verify both event types are present.
	gotTypes := map[string]int{
		received[0].EventType(): 1,
	}
	if received[1].EventType() == received[0].EventType() {
		gotTypes[received[1].EventType()]++
	} else {
		gotTypes[received[1].EventType()] = 1
	}
	if gotTypes["started"] != 1 || gotTypes["stream_chunk"] != 1 {
		t.Errorf("expected one 'started' and one 'stream_chunk', got %v", gotTypes)
	}
}

func TestEventBus_Unsubscribe(t *testing.T) {
	bus := NewEventBus(16)

	aid, sid := testAgentID(t), testSessionID(t)

	var mu sync.Mutex
	var received []AgentEvent

	handler := NewEventHandlerFunc("sub-unsub", func(ev AgentEvent) error {
		mu.Lock()
		received = append(received, ev)
		mu.Unlock()
		return nil
	})

	bus.Subscribe(handler)

	ctx := context.Background()
	bus.Publish(ctx, NewStartedEvent(aid, sid))
	time.Sleep(30 * time.Millisecond)

	bus.Unsubscribe("sub-unsub")

	bus.Publish(ctx, NewErrorEvent(aid, sid, "after unsub"))
	time.Sleep(30 * time.Millisecond)

	mu.Lock()
	defer mu.Unlock()
	if len(received) != 1 {
		t.Errorf("expected 1 event after unsubscribe, got %d", len(received))
	}
}

func TestEventBus_MultipleSubscribers(t *testing.T) {
	bus := NewEventBus(16)

	aid, sid := testAgentID(t), testSessionID(t)

	var mu sync.Mutex
	count1, count2 := 0, 0

	h1 := NewEventHandlerFunc("h1", func(ev AgentEvent) error {
		mu.Lock()
		count1++
		mu.Unlock()
		return nil
	})
	h2 := NewEventHandlerFunc("h2", func(ev AgentEvent) error {
		mu.Lock()
		count2++
		mu.Unlock()
		return nil
	})

	bus.Subscribe(h1)
	bus.Subscribe(h2)

	ctx := context.Background()
	bus.Publish(ctx, NewStartedEvent(aid, sid))
	time.Sleep(50 * time.Millisecond)

	mu.Lock()
	defer mu.Unlock()
	if count1 != 1 {
		t.Errorf("h1 received %d events, want 1", count1)
	}
	if count2 != 1 {
		t.Errorf("h2 received %d events, want 1", count2)
	}
}

func TestEventBus_PublishCanceledContext(t *testing.T) {
	bus := NewEventBus(16)

	aid, sid := testAgentID(t), testSessionID(t)

	var mu sync.Mutex
	var received []AgentEvent

	handler := NewEventHandlerFunc("cancel-test", func(ev AgentEvent) error {
		mu.Lock()
		received = append(received, ev)
		mu.Unlock()
		return nil
	})

	bus.Subscribe(handler)

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately

	bus.Publish(ctx, NewStartedEvent(aid, sid))
	time.Sleep(30 * time.Millisecond)

	mu.Lock()
	defer mu.Unlock()
	if len(received) != 0 {
		t.Errorf("expected 0 events with canceled context, got %d", len(received))
	}
}
