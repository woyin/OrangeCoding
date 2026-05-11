// Package agents provides concrete agent implementations built on the agent loop.
package agents

import (
	"context"
	"fmt"
	"sync"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
)

// Agent is the interface that all sub-agents must implement.
type Agent interface {
	// ID returns the agent's unique identifier.
	ID() core.AgentId
	// Role returns the agent's role.
	Role() core.AgentRole
	// Run starts the agent on the given task.
	Run(ctx context.Context, task string) error
	// Stop cancels the running agent.
	Stop() error
	// Status returns the agent's current status.
	Status() core.AgentStatus
}

// BaseAgent provides common implementation for all agents.
type BaseAgent struct {
	id     core.AgentId
	role   core.AgentRole
	loop   *agent.AgentLoop
	status core.AgentStatus
	cancel context.CancelFunc
	mu     sync.Mutex
}

// NewBaseAgent creates a new BaseAgent with the given role and agent loop.
func NewBaseAgent(role core.AgentRole, loop *agent.AgentLoop) *BaseAgent {
	return &BaseAgent{
		id:     core.NewAgentId(),
		role:   role,
		loop:   loop,
		status: core.StatusIdle,
	}
}

// ID returns the agent's unique identifier.
func (a *BaseAgent) ID() core.AgentId {
	return a.id
}

// Role returns the agent's role.
func (a *BaseAgent) Role() core.AgentRole {
	return a.role
}

// Status returns the agent's current status.
func (a *BaseAgent) Status() core.AgentStatus {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.status
}

// Stop cancels the agent's current execution.
func (a *BaseAgent) Stop() error {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.cancel != nil {
		a.cancel()
		a.cancel = nil
	}
	a.status = core.StatusCompleted
	return nil
}

// Run executes the agent loop for the given task. It sets the status to Running,
// calls loop.Run, and updates the status to Completed or Failed.
func (a *BaseAgent) Run(ctx context.Context, task string) error {
	a.mu.Lock()
	a.status = core.StatusRunning
	ctx, a.cancel = context.WithCancel(ctx)
	a.mu.Unlock()

	// Add the task as a user message
	a.loop.Context().AddUserMessage(task)

	eventCh := make(chan core.AgentEvent, 100)
	go func() {
		for range eventCh {
			// Drain events (could forward to an event bus)
		}
	}()

	result, err := a.loop.Run(ctx, ai.ChatOptions{}, eventCh)
	close(eventCh)

	a.mu.Lock()
	defer a.mu.Unlock()
	if a.cancel != nil {
		a.cancel()
		a.cancel = nil
	}

	if err != nil {
		a.status = core.StatusFailed
		return fmt.Errorf("agent %s failed: %w", a.id, err)
	}

	_ = result
	a.status = core.StatusCompleted
	return nil
}

// Loop returns the underlying AgentLoop.
func (a *BaseAgent) Loop() *agent.AgentLoop {
	return a.loop
}
