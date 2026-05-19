package multiplexer

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"time"
)

// ManagedPane tracks a single pane's state and IPC channel.
type ManagedPane struct {
	Info       PaneInfo
	Transport  *SocketTransport
	SocketPath string
	Cancel     context.CancelFunc
}

// PaneManager coordinates pane creation, IPC setup, and cleanup.
type PaneManager struct {
	backend     Backend
	config      MultiplexerConfig
	panes       map[string]*ManagedPane
	mu          sync.RWMutex
	paneCounter int
}

// NewPaneManager creates a PaneManager with the given backend and config.
func NewPaneManager(backend Backend, config MultiplexerConfig) *PaneManager {
	config.Normalize()
	return &PaneManager{
		backend: backend,
		config:  config,
		panes:   make(map[string]*ManagedPane),
	}
}

// SpawnAgentPane creates a new pane, sets up IPC, and returns a managed pane
// for bidirectional communication with the child agent process.
func (pm *PaneManager) SpawnAgentPane(ctx context.Context, agentName string, task string) (*ManagedPane, error) {
	if pm.backend == nil {
		return nil, fmt.Errorf("no multiplexer backend available")
	}

	pm.mu.Lock()
	pm.paneCounter++
	paneID := fmt.Sprintf("pane-%d", pm.paneCounter)
	pm.mu.Unlock()

	socketPath := SocketPath(pm.config.SocketDir, paneID)

	// 1. Create Unix socket listener.
	ln, err := CreateListener(socketPath)
	if err != nil {
		return nil, fmt.Errorf("create listener: %w", err)
	}
	defer ln.Close()

	// 2. Spawn pane running the pane-agent command.
	command := fmt.Sprintf("orange-code pane-agent --socket %s", socketPath)
	info, err := pm.backend.CreatePane(ctx, agentName, command)
	if err != nil {
		CleanupSocket(socketPath)
		return nil, fmt.Errorf("create pane: %w", err)
	}
	// Override the backend-generated ID with our tracked ID.
	info.ID = paneID

	// 3. Wait for the child process to connect.
	timeout := time.Duration(pm.config.CommandTimeoutMs) * time.Millisecond
	conn, err := WaitForConnection(ln, timeout)
	if err != nil {
		pm.backend.ClosePane(ctx, info.ID)
		CleanupSocket(socketPath)
		return nil, fmt.Errorf("wait for pane connection: %w", err)
	}

	transport := NewSocketTransport(conn)

	// 4. Send the task payload.
	taskPayload := TaskPayload{
		Task:    task,
		AgentID: agentName,
	}
	payloadBytes, _ := jsonMarshal(taskPayload)
	if err := transport.Send(IPCMessage{
		Type:    IPCTask,
		ID:      paneID,
		Payload: payloadBytes,
	}); err != nil {
		transport.Close()
		pm.backend.ClosePane(ctx, info.ID)
		CleanupSocket(socketPath)
		return nil, fmt.Errorf("send task: %w", err)
	}

	paneCtx, cancel := context.WithCancel(ctx)
	managed := &ManagedPane{
		Info:       info,
		Transport:  transport,
		SocketPath: socketPath,
		Cancel:     cancel,
	}

	pm.mu.Lock()
	pm.panes[paneID] = managed
	pm.mu.Unlock()

	// Start a goroutine to keep the pane context alive and handle cleanup.
	go func() {
		<-paneCtx.Done()
		pm.ClosePane(paneID)
	}()

	return managed, nil
}

// ClosePane tears down a pane and cleans up its socket.
func (pm *PaneManager) ClosePane(paneID string) error {
	pm.mu.Lock()
	managed, ok := pm.panes[paneID]
	if !ok {
		pm.mu.Unlock()
		return nil
	}
	delete(pm.panes, paneID)
	pm.mu.Unlock()

	if managed.Cancel != nil {
		managed.Cancel()
	}
	if managed.Transport != nil {
		managed.Transport.Close()
	}
	pm.backend.ClosePane(context.Background(), paneID)
	CleanupSocket(managed.SocketPath)
	return nil
}

// CloseAll terminates all managed panes.
func (pm *PaneManager) CloseAll() error {
	pm.mu.Lock()
	ids := make([]string, 0, len(pm.panes))
	for id := range pm.panes {
		ids = append(ids, id)
	}
	pm.mu.Unlock()

	for _, id := range ids {
		pm.ClosePane(id)
	}
	return nil
}

// ActivePanes returns all currently tracked panes.
func (pm *PaneManager) ActivePanes() []PaneInfo {
	pm.mu.RLock()
	defer pm.mu.RUnlock()

	infos := make([]PaneInfo, 0, len(pm.panes))
	for _, p := range pm.panes {
		infos = append(infos, p.Info)
	}
	return infos
}

// jsonMarshal is a helper that marshals to json.RawMessage.
func jsonMarshal(v interface{}) (json.RawMessage, error) {
	return json.Marshal(v)
}
