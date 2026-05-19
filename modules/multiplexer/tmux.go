package multiplexer

import (
	"context"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"
)

// TmuxBackend implements Backend for the tmux terminal multiplexer.
type TmuxBackend struct {
	mu    sync.RWMutex
	panes map[string]*PaneInfo
}

func NewTmuxBackend() *TmuxBackend {
	return &TmuxBackend{
		panes: make(map[string]*PaneInfo),
	}
}

func (t *TmuxBackend) Name() string { return "tmux" }

func (t *TmuxBackend) IsAvailable() bool {
	_, ok := os.LookupEnv("TMUX")
	return ok
}

func (t *TmuxBackend) CreatePane(ctx context.Context, name string, command string) (PaneInfo, error) {
	// -P prints pane ID, -F sets format, -h splits horizontally
	args := []string{"split-window", "-h", "-P", "-F", "#{pane_id}", "-t", name, "sh", "-c", command}
	out, err := runCommand(ctx, "tmux", args...)
	if err != nil {
		return PaneInfo{}, fmt.Errorf("tmux create pane: %w", err)
	}

	paneID := parseTmuxPaneID(out)
	if paneID == "" {
		paneID = fmt.Sprintf("tmux-%s-%d", name, time.Now().UnixNano())
	}

	info := PaneInfo{
		ID:        paneID,
		Name:      name,
		State:     PaneStateRunning,
		CreatedAt: time.Now(),
		Backend:   "tmux",
	}

	t.mu.Lock()
	t.panes[paneID] = &info
	t.mu.Unlock()

	return info, nil
}

func (t *TmuxBackend) ClosePane(ctx context.Context, paneID string) error {
	_, err := runCommand(ctx, "tmux", "kill-pane", "-t", paneID)
	if err != nil {
		return fmt.Errorf("tmux close pane %s: %w", paneID, err)
	}

	t.mu.Lock()
	if p, ok := t.panes[paneID]; ok {
		p.State = PaneStateExited
		delete(t.panes, paneID)
	}
	t.mu.Unlock()

	return nil
}

func (t *TmuxBackend) SendText(ctx context.Context, paneID string, text string) error {
	_, err := runCommand(ctx, "tmux", "send-keys", "-t", paneID, text, "Enter")
	if err != nil {
		return fmt.Errorf("tmux send text to %s: %w", paneID, err)
	}
	return nil
}

func (t *TmuxBackend) FocusPane(ctx context.Context, paneID string) error {
	_, err := runCommand(ctx, "tmux", "select-pane", "-t", paneID)
	if err != nil {
		return fmt.Errorf("tmux focus pane %s: %w", paneID, err)
	}
	return nil
}

func (t *TmuxBackend) CaptureOutput(ctx context.Context, paneID string) (string, error) {
	out, err := runCommand(ctx, "tmux", "capture-pane", "-t", paneID, "-p")
	if err != nil {
		return "", fmt.Errorf("tmux capture output %s: %w", paneID, err)
	}
	return out, nil
}

func (t *TmuxBackend) ListPanes(ctx context.Context) ([]PaneInfo, error) {
	t.mu.RLock()
	defer t.mu.RUnlock()

	panes := make([]PaneInfo, 0, len(t.panes))
	for _, p := range t.panes {
		panes = append(panes, *p)
	}
	return panes, nil
}

// parseTmuxPaneID extracts the pane ID from tmux output (e.g., "%5").
func parseTmuxPaneID(output string) string {
	output = strings.TrimSpace(output)
	if output == "" {
		return ""
	}
	// tmux pane IDs start with %
	for _, line := range strings.Split(output, "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "%") {
			return line
		}
	}
	return output
}
