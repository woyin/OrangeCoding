package worker

import (
	"context"
	"fmt"
	"sync"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/control-protocol"
)

// WorkerRuntime manages multiple agent sessions. It tracks running agents
// and provides lifecycle operations (start, stop, list, status).
type WorkerRuntime struct {
	mu      sync.RWMutex
	agents  map[string]*AgentExecutor
	eventCh chan controlprotocol.ServerEvent
}

// NewWorkerRuntime creates a new WorkerRuntime that emits events to eventCh.
func NewWorkerRuntime(eventCh chan controlprotocol.ServerEvent) *WorkerRuntime {
	return &WorkerRuntime{
		agents:  make(map[string]*AgentExecutor),
		eventCh: eventCh,
	}
}

// StartSession creates a new agent executor and starts it in a goroutine.
// Returns an error if a session with the same ID already exists.
// If agentLoop is nil, the executor will complete immediately (useful for testing).
func (r *WorkerRuntime) StartSession(sessionID string, agentLoop *agent.AgentLoop) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	if _, exists := r.agents[sessionID]; exists {
		return fmt.Errorf("worker runtime: session %q already exists", sessionID)
	}

	executor := NewAgentExecutor(sessionID, agentLoop)
	executor.SetEventCh(r.eventCh)

	ctx, cancel := context.WithCancel(context.Background())
	executor.SetCancel(cancel)

	r.agents[sessionID] = executor

	go func() {
		executor.Run(ctx)
	}()

	return nil
}

// StopSession cancels and removes a running agent session.
// Returns an error if the session does not exist.
func (r *WorkerRuntime) StopSession(sessionID string) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	executor, exists := r.agents[sessionID]
	if !exists {
		return fmt.Errorf("worker runtime: session %q not found", sessionID)
	}

	executor.Cancel()
	delete(r.agents, sessionID)

	return nil
}

// ListSessions returns the IDs of all active sessions.
func (r *WorkerRuntime) ListSessions() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()

	sessions := make([]string, 0, len(r.agents))
	for id := range r.agents {
		sessions = append(sessions, id)
	}
	return sessions
}

// GetStatus returns the execution status of a session.
// The second return value is false if the session does not exist.
func (r *WorkerRuntime) GetStatus(sessionID string) (string, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()

	executor, exists := r.agents[sessionID]
	if !exists {
		return "", false
	}
	return executor.Status(), true
}
