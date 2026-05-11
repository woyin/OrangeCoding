package core

import (
	"context"
	"sync"
	"time"
)

// ---------------------------------------------------------------------------
// AgentEvent interface
// ---------------------------------------------------------------------------

// AgentEvent is the universal interface for all events emitted by an agent.
type AgentEvent interface {
	EventType() string
	AgentID() AgentId
	SessionID() SessionId
	Timestamp() time.Time
}

// ---------------------------------------------------------------------------
// BaseEvent
// ---------------------------------------------------------------------------

// BaseEvent is embedded in every concrete event type and provides the four
// fields required by the AgentEvent interface.
type BaseEvent struct {
	Type    string    `json:"type"`
	Agent   AgentId   `json:"agent_id"`
	Session SessionId `json:"session_id"`
	Time    time.Time `json:"timestamp"`
}

// EventType returns the event type string.
func (b BaseEvent) EventType() string { return b.Type }

// AgentID returns the agent that produced this event.
func (b BaseEvent) AgentID() AgentId { return b.Agent }

// SessionID returns the session this event belongs to.
func (b BaseEvent) SessionID() SessionId { return b.Session }

// Timestamp returns the wall-clock time when the event was created.
func (b BaseEvent) Timestamp() time.Time { return b.Time }

// ---------------------------------------------------------------------------
// Concrete event types (11)
// ---------------------------------------------------------------------------

// 1. StartedEvent — agent has started.
type StartedEvent struct{ BaseEvent }

// NewStartedEvent creates a StartedEvent with the current timestamp.
func NewStartedEvent(agentID AgentId, sessionID SessionId) StartedEvent {
	return StartedEvent{
		BaseEvent: BaseEvent{
			Type:    "started",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
	}
}

// 2. CompletedEvent — agent has finished.
type CompletedEvent struct {
	BaseEvent
	Summary string `json:"summary"`
}

// NewCompletedEvent creates a CompletedEvent with a summary.
func NewCompletedEvent(agentID AgentId, sessionID SessionId, summary string) CompletedEvent {
	return CompletedEvent{
		BaseEvent: BaseEvent{
			Type:    "completed",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		Summary: summary,
	}
}

// 3. MessageReceivedEvent — agent received a message.
type MessageReceivedEvent struct {
	BaseEvent
	ContentPreview string `json:"content_preview"`
}

// NewMessageReceivedEvent creates a MessageReceivedEvent with a content preview.
func NewMessageReceivedEvent(agentID AgentId, sessionID SessionId, preview string) MessageReceivedEvent {
	return MessageReceivedEvent{
		BaseEvent: BaseEvent{
			Type:    "message_received",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		ContentPreview: preview,
	}
}

// 4. ToolCallRequestedEvent — agent is requesting a tool call.
type ToolCallRequestedEvent struct {
	BaseEvent
	ToolCall ToolCall `json:"tool_call"`
}

// NewToolCallRequestedEvent creates a ToolCallRequestedEvent.
func NewToolCallRequestedEvent(agentID AgentId, sessionID SessionId, tc ToolCall) ToolCallRequestedEvent {
	return ToolCallRequestedEvent{
		BaseEvent: BaseEvent{
			Type:    "tool_call_requested",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		ToolCall: tc,
	}
}

// 5. ToolCallCompletedEvent — a tool call finished.
type ToolCallCompletedEvent struct {
	BaseEvent
	ToolName   string `json:"tool_name"`
	Success    bool   `json:"success"`
	DurationMs uint64 `json:"duration_ms"`
}

// NewToolCallCompletedEvent creates a ToolCallCompletedEvent.
func NewToolCallCompletedEvent(agentID AgentId, sessionID SessionId, name string, success bool, durMs uint64) ToolCallCompletedEvent {
	return ToolCallCompletedEvent{
		BaseEvent: BaseEvent{
			Type:    "tool_call_completed",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		ToolName:   name,
		Success:    success,
		DurationMs: durMs,
	}
}

// 6. TokenUsageUpdatedEvent — token usage has been updated.
type TokenUsageUpdatedEvent struct {
	BaseEvent
	Usage TokenUsage `json:"usage"`
}

// NewTokenUsageUpdatedEvent creates a TokenUsageUpdatedEvent.
func NewTokenUsageUpdatedEvent(agentID AgentId, sessionID SessionId, usage TokenUsage) TokenUsageUpdatedEvent {
	return TokenUsageUpdatedEvent{
		BaseEvent: BaseEvent{
			Type:    "token_usage_updated",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		Usage: usage,
	}
}

// 7. StreamChunkEvent — a chunk of streaming content.
type StreamChunkEvent struct {
	BaseEvent
	Content string `json:"content"`
}

// NewStreamChunkEvent creates a StreamChunkEvent.
func NewStreamChunkEvent(agentID AgentId, sessionID SessionId, content string) StreamChunkEvent {
	return StreamChunkEvent{
		BaseEvent: BaseEvent{
			Type:    "stream_chunk",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		Content: content,
	}
}

// 8. ErrorEvent — an error occurred.
type ErrorEvent struct {
	BaseEvent
	ErrorMessage string `json:"error_message"`
}

// NewErrorEvent creates an ErrorEvent.
func NewErrorEvent(agentID AgentId, sessionID SessionId, errMsg string) ErrorEvent {
	return ErrorEvent{
		BaseEvent: BaseEvent{
			Type:    "error",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		ErrorMessage: errMsg,
	}
}

// 9. GoalPhaseChangedEvent — goal phase has changed.
type GoalPhaseChangedEvent struct {
	BaseEvent
	Phase string `json:"phase"`
	Cycle uint32 `json:"cycle"`
}

// NewGoalPhaseChangedEvent creates a GoalPhaseChangedEvent.
func NewGoalPhaseChangedEvent(agentID AgentId, sessionID SessionId, phase string, cycle uint32) GoalPhaseChangedEvent {
	return GoalPhaseChangedEvent{
		BaseEvent: BaseEvent{
			Type:    "goal_phase_changed",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		Phase: phase,
		Cycle: cycle,
	}
}

// 10. GoalTaskCompletedEvent — a task within a goal has completed.
type GoalTaskCompletedEvent struct {
	BaseEvent
	TaskID          string `json:"task_id"`
	TaskDescription string `json:"task_description"`
	Success         bool   `json:"success"`
}

// NewGoalTaskCompletedEvent creates a GoalTaskCompletedEvent.
func NewGoalTaskCompletedEvent(agentID AgentId, sessionID SessionId, taskID, desc string, success bool) GoalTaskCompletedEvent {
	return GoalTaskCompletedEvent{
		BaseEvent: BaseEvent{
			Type:    "goal_task_completed",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		TaskID:          taskID,
		TaskDescription: desc,
		Success:         success,
	}
}

// 11. GoalCycleCompleteEvent — a goal cycle has completed.
type GoalCycleCompleteEvent struct {
	BaseEvent
	Cycle              uint32 `json:"cycle"`
	TasksCompleted     uint32 `json:"tasks_completed"`
	TasksFailed        uint32 `json:"tasks_failed"`
	VerificationPassed bool   `json:"verification_passed"`
}

// NewGoalCycleCompleteEvent creates a GoalCycleCompleteEvent.
func NewGoalCycleCompleteEvent(agentID AgentId, sessionID SessionId, cycle, completed, failed uint32, verified bool) GoalCycleCompleteEvent {
	return GoalCycleCompleteEvent{
		BaseEvent: BaseEvent{
			Type:    "goal_cycle_complete",
			Agent:   agentID,
			Session: sessionID,
			Time:    time.Now().UTC(),
		},
		Cycle:              cycle,
		TasksCompleted:     completed,
		TasksFailed:        failed,
		VerificationPassed: verified,
	}
}

// ---------------------------------------------------------------------------
// EventHandler interface
// ---------------------------------------------------------------------------

// EventHandler processes events. Implementations must be safe for concurrent use
// when registered with an EventBus.
type EventHandler interface {
	Handle(ev AgentEvent) error
	Name() string
}

// ---------------------------------------------------------------------------
// EventHandlerFunc adapter
// ---------------------------------------------------------------------------

// EventHandlerFunc wraps a function to satisfy the EventHandler interface.
type EventHandlerFunc struct {
	name string
	fn   func(AgentEvent) error
}

// NewEventHandlerFunc creates an EventHandlerFunc with the given name and function.
func NewEventHandlerFunc(name string, fn func(AgentEvent) error) EventHandlerFunc {
	return EventHandlerFunc{name: name, fn: fn}
}

// Handle invokes the wrapped function.
func (h EventHandlerFunc) Handle(ev AgentEvent) error { return h.fn(ev) }

// Name returns the handler name.
func (h EventHandlerFunc) Name() string { return h.name }

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

// EventBus dispatches events to all registered handlers concurrently.
type EventBus struct {
	mu        sync.RWMutex
	subs      map[string]EventHandler
	buffer    int
}

// NewEventBus creates a new EventBus. The buffer parameter controls the channel
// buffer size used internally for each handler during publish.
func NewEventBus(buffer int) *EventBus {
	return &EventBus{
		subs:   make(map[string]EventHandler),
		buffer: buffer,
	}
}

// Subscribe registers a handler. The handler's Name() is used as its unique ID.
// If a handler with the same name already exists it is replaced.
func (b *EventBus) Subscribe(handler EventHandler) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.subs[handler.Name()] = handler
}

// Unsubscribe removes the handler identified by the given ID.
// If no handler with that ID exists, Unsubscribe is a no-op.
func (b *EventBus) Unsubscribe(id string) {
	b.mu.Lock()
	defer b.mu.Unlock()
	delete(b.subs, id)
}

// Publish dispatches ev to every registered handler via a goroutine.
// If ctx is canceled before dispatch begins, no handlers are invoked.
func (b *EventBus) Publish(ctx context.Context, ev AgentEvent) {
	// Check context before acquiring the lock.
	if ctx.Err() != nil {
		return
	}

	b.mu.RLock()
	handlers := make([]EventHandler, 0, len(b.subs))
	for _, h := range b.subs {
		handlers = append(handlers, h)
	}
	b.mu.RUnlock()

	for _, h := range handlers {
		go func(handler EventHandler) {
			_ = handler.Handle(ev)
		}(h)
	}
}
