package multiplexer

import (
	"context"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"
)

// ZellijBackend implements Backend for the zellij terminal multiplexer.
type ZellijBackend struct {
	mu    sync.RWMutex
	panes map[string]*PaneInfo
}

func NewZellijBackend() *ZellijBackend {
	return &ZellijBackend{
		panes: make(map[string]*PaneInfo),
	}
}

func (z *ZellijBackend) Name() string { return "zellij" }

func (z *ZellijBackend) IsAvailable() bool {
	_, ok := os.LookupEnv("ZELLIJ_SESSION_NAME")
	return ok
}

func (z *ZellijBackend) CreatePane(ctx context.Context, name string, command string) (PaneInfo, error) {
	args := []string{"action", "new-pane", "--name", name, "--", "sh", "-c", command}
	out, err := runCommand(ctx, "zellij", args...)
	if err != nil {
		return PaneInfo{}, fmt.Errorf("zellij create pane: %w", err)
	}

	paneID := parseZellijPaneID(out, name)
	info := PaneInfo{
		ID:        paneID,
		Name:      name,
		State:     PaneStateRunning,
		CreatedAt: time.Now(),
		Backend:   "zellij",
	}

	z.mu.Lock()
	z.panes[paneID] = &info
	z.mu.Unlock()

	return info, nil
}

func (z *ZellijBackend) ClosePane(ctx context.Context, paneID string) error {
	_, err := runCommand(ctx, "zellij", "action", "close-pane", "--pane-id", paneID)
	if err != nil {
		return fmt.Errorf("zellij close pane %s: %w", paneID, err)
	}

	z.mu.Lock()
	if p, ok := z.panes[paneID]; ok {
		p.State = PaneStateExited
		delete(z.panes, paneID)
	}
	z.mu.Unlock()

	return nil
}

func (z *ZellijBackend) SendText(ctx context.Context, paneID string, text string) error {
	_, err := runCommand(ctx, "zellij", "action", "write-chars", "--pane-id", paneID, text)
	if err != nil {
		return fmt.Errorf("zellij send text to %s: %w", paneID, err)
	}
	return nil
}

func (z *ZellijBackend) FocusPane(ctx context.Context, paneID string) error {
	_, err := runCommand(ctx, "zellij", "action", "focus-pane", "--pane-id", paneID)
	if err != nil {
		return fmt.Errorf("zellij focus pane %s: %w", paneID, err)
	}
	return nil
}

func (z *ZellijBackend) CaptureOutput(ctx context.Context, paneID string) (string, error) {
	out, err := runCommand(ctx, "zellij", "action", "dump-screen", "--pane-id", paneID)
	if err != nil {
		return "", fmt.Errorf("zellij capture output %s: %w", paneID, err)
	}
	return out, nil
}

func (z *ZellijBackend) ListPanes(ctx context.Context) ([]PaneInfo, error) {
	z.mu.RLock()
	defer z.mu.RUnlock()

	panes := make([]PaneInfo, 0, len(z.panes))
	for _, p := range z.panes {
		panes = append(panes, *p)
	}
	return panes, nil
}

// parseZellijPaneID extracts a pane identifier from zellij command output.
// Falls back to a generated ID if parsing fails.
func parseZellijPaneID(output string, name string) string {
	output = strings.TrimSpace(output)
	if output != "" {
		// zellij may output pane info in various formats
		for _, line := range strings.Split(output, "\n") {
			line = strings.TrimSpace(line)
			if line != "" {
				return line
			}
		}
	}
	return fmt.Sprintf("zellij-%s-%d", name, time.Now().UnixNano())
}
