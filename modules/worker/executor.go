package worker

import (
	"context"
	"fmt"
	"sync"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/control-protocol"
)

// AgentExecutor manages the lifecycle of a single agent session.
// It wraps an AgentLoop and tracks its execution status.
type AgentExecutor struct {
	mu        sync.RWMutex
	sessionID string
	loop      *agent.AgentLoop
	cancel    context.CancelFunc
	status    string // "pending", "running", "completed", "failed"
	eventCh   chan controlprotocol.ServerEvent
}

// NewAgentExecutor creates a new AgentExecutor for the given session.
// The executor starts in "pending" status.
func NewAgentExecutor(sessionID string, loop *agent.AgentLoop) *AgentExecutor {
	return &AgentExecutor{
		sessionID: sessionID,
		loop:      loop,
		status:    "pending",
	}
}

// SetEventCh sets the channel for emitting server events.
func (e *AgentExecutor) SetEventCh(ch chan controlprotocol.ServerEvent) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.eventCh = ch
}

// SessionID returns the session identifier.
func (e *AgentExecutor) SessionID() string {
	return e.sessionID
}

// Status returns the current execution status.
func (e *AgentExecutor) Status() string {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.status
}

// SetCancel sets the context cancellation function for this executor.
func (e *AgentExecutor) SetCancel(cancel context.CancelFunc) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.cancel = cancel
}

// Cancel stops the agent execution by canceling its context.
func (e *AgentExecutor) Cancel() {
	e.mu.RLock()
	defer e.mu.RUnlock()
	if e.cancel != nil {
		e.cancel()
	}
}

// Run executes the agent loop. It updates status to "running" during execution
// and sets "completed" or "failed" when done.
// If the loop is nil, it immediately completes successfully (useful for testing).
func (e *AgentExecutor) Run(ctx context.Context) error {
	e.mu.Lock()
	e.status = "running"
	ch := e.eventCh
	e.mu.Unlock()

	// Check context first
	if ctx.Err() != nil {
		e.mu.Lock()
		e.status = "failed"
		e.mu.Unlock()

		if ch != nil {
			ch <- &controlprotocol.ErrorEvent{
				SessionID: e.sessionID,
				Error:     ctx.Err().Error(),
			}
		}
		return fmt.Errorf("agent executor: context already canceled: %w", ctx.Err())
	}

	// Emit task_update: running
	if ch != nil {
		ch <- &controlprotocol.TaskUpdateEvent{
			SessionID: e.sessionID,
			Status:    "running",
			Message:   "agent started",
		}
	}

	// Handle nil loop (for testing or stub scenarios)
	if e.loop == nil {
		e.mu.Lock()
		e.status = "completed"
		e.mu.Unlock()

		if ch != nil {
			ch <- &controlprotocol.TaskUpdateEvent{
				SessionID: e.sessionID,
				Status:    "completed",
				Message:   "agent completed (no loop)",
			}
		}
		return nil
	}

	// Run the agent loop
	_, err := e.loop.Run(ctx, ai.ChatOptions{}, nil)

	e.mu.Lock()
	defer e.mu.Unlock()

	if err != nil {
		e.status = "failed"
		if ch != nil {
			ch <- &controlprotocol.ErrorEvent{
				SessionID: e.sessionID,
				Error:     err.Error(),
			}
		}
		return fmt.Errorf("agent executor: run failed: %w", err)
	}

	e.status = "completed"
	if ch != nil {
		ch <- &controlprotocol.TaskUpdateEvent{
			SessionID: e.sessionID,
			Status:    "completed",
			Message:   "agent completed",
		}
	}
	return nil
}
