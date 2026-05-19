package agent

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"sync"
	"time"
)

// HookPoint identifies a point in the agent lifecycle where hooks can run.
type HookPoint string

const (
	HookPreToolCall  HookPoint = "pre_tool_call"
	HookPostToolCall HookPoint = "post_tool_call"
	HookPreSampling  HookPoint = "pre_sampling"
	HookPostSampling HookPoint = "post_sampling"
)

// Hook represents a shell command to run at a specific hook point.
type Hook struct {
	Point   HookPoint
	Command string
}

// HookManager manages and executes hooks at various lifecycle points.
type HookManager struct {
	mu    sync.RWMutex
	hooks map[HookPoint][]Hook
}

// NewHookManager creates a new HookManager with no registered hooks.
func NewHookManager() *HookManager {
	return &HookManager{
		hooks: make(map[HookPoint][]Hook),
	}
}

// Register adds a hook to be executed at the specified hook point.
func (m *HookManager) Register(hook Hook) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.hooks[hook.Point] = append(m.hooks[hook.Point], hook)
}

// Run executes all hooks registered for the given point.
// The data parameter is passed to each hook command via stdin.
// If any hook fails, execution continues but the first error is returned.
func (m *HookManager) Run(ctx context.Context, point HookPoint, data string) error {
	m.mu.RLock()
	hooks := make([]Hook, len(m.hooks[point]))
	copy(hooks, m.hooks[point])
	m.mu.RUnlock()

	var firstErr error
	for _, hook := range hooks {
		cmdCtx, cancel := context.WithTimeout(ctx, 10*time.Second)
		cmd := exec.CommandContext(cmdCtx, "sh", "-c", hook.Command)
		cmd.Stdin = strings.NewReader(data)

		if err := cmd.Run(); err != nil && firstErr == nil {
			firstErr = fmt.Errorf("hook %q failed: %w", hook.Command, err)
		}
		cancel()
	}

	return firstErr
}
