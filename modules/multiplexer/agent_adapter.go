package multiplexer

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"

	"github.com/woyin/OrangeCoding/modules/core"
)

// MultiplexerAgentAdapter wraps an agent to run in a separate terminal pane.
// It implements the same interface as agents.Agent (ID, Role, Run, Stop, Status).
type MultiplexerAgentAdapter struct {
	id      core.AgentId
	role    core.AgentRole
	manager *PaneManager
	status  core.AgentStatus
	cancel  context.CancelFunc
	mu      sync.Mutex
}

// NewMultiplexerAgentAdapter creates an adapter that runs agents in isolated panes.
func NewMultiplexerAgentAdapter(
	role core.AgentRole,
	manager *PaneManager,
) *MultiplexerAgentAdapter {
	return &MultiplexerAgentAdapter{
		id:      core.NewAgentId(),
		role:    role,
		manager: manager,
		status:  core.StatusIdle,
	}
}

func (a *MultiplexerAgentAdapter) ID() core.AgentId   { return a.id }
func (a *MultiplexerAgentAdapter) Role() core.AgentRole { return a.role }

func (a *MultiplexerAgentAdapter) Status() core.AgentStatus {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.status
}

// Stop cancels the running agent and closes its pane.
func (a *MultiplexerAgentAdapter) Stop() error {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.cancel != nil {
		a.cancel()
		a.cancel = nil
	}
	a.status = core.StatusCompleted
	return nil
}

// Run spawns a pane, sends the task, and waits for the result.
func (a *MultiplexerAgentAdapter) Run(ctx context.Context, task string) error {
	a.mu.Lock()
	a.status = core.StatusRunning
	ctx, a.cancel = context.WithCancel(ctx)
	a.mu.Unlock()

	defer func() {
		a.mu.Lock()
		if a.cancel != nil {
			a.cancel()
			a.cancel = nil
		}
		a.mu.Unlock()
	}()

	// Spawn a pane and set up IPC.
	managed, err := a.manager.SpawnAgentPane(ctx, a.id.String(), task)
	if err != nil {
		a.mu.Lock()
		a.status = core.StatusFailed
		a.mu.Unlock()
		return fmt.Errorf("spawn pane for agent %s: %w", a.id, err)
	}

	// Receive loop: process events and wait for result.
	for {
		msg, err := managed.Transport.Receive()
		if err != nil {
			a.mu.Lock()
			a.status = core.StatusFailed
			a.mu.Unlock()
			return fmt.Errorf("receive from pane: %w", err)
		}

		switch msg.Type {
		case IPCResult:
			var result ResultPayload
			if err := json.Unmarshal(msg.Payload, &result); err != nil {
				return fmt.Errorf("unmarshal result: %w", err)
			}
			a.mu.Lock()
			if result.Success {
				a.status = core.StatusCompleted
			} else {
				a.status = core.StatusFailed
			}
			a.mu.Unlock()
			if !result.Success && result.Error != "" {
				return fmt.Errorf("agent error: %s", result.Error)
			}
			return nil

		case IPCEvent:
			// Events are informational; continue receiving.
			continue

		case IPCKeepalive:
			// Keepalive; continue receiving.
			continue

		default:
			// Unknown message type; skip.
			continue
		}
	}
}
