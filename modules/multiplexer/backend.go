package multiplexer

import (
	"context"
	"fmt"
	"os/exec"
)

// Backend abstracts terminal multiplexer operations.
// Implementations wrap CLI commands for zellij or tmux.
type Backend interface {
	// Name returns "zellij", "tmux", or "none".
	Name() string

	// IsAvailable checks whether this backend can be used in the current environment.
	IsAvailable() bool

	// CreatePane spawns a new pane running the given command.
	CreatePane(ctx context.Context, name string, command string) (PaneInfo, error)

	// ClosePane terminates the named pane.
	ClosePane(ctx context.Context, paneID string) error

	// SendText writes text into the pane's stdin (as if typed).
	SendText(ctx context.Context, paneID string, text string) error

	// FocusPane brings the pane to foreground focus.
	FocusPane(ctx context.Context, paneID string) error

	// CaptureOutput reads the current visible buffer of the pane.
	CaptureOutput(ctx context.Context, paneID string) (string, error)

	// ListPanes returns all panes managed by this backend.
	ListPanes(ctx context.Context) ([]PaneInfo, error)
}

// DetectBackend returns the best available backend.
// Priority: zellij > tmux > nil.
func DetectBackend() Backend {
	z := &ZellijBackend{}
	if z.IsAvailable() {
		return z
	}
	t := &TmuxBackend{}
	if t.IsAvailable() {
		return t
	}
	return nil
}

// NewBackendFromConfig returns a backend based on the config preference.
func NewBackendFromConfig(cfg MultiplexerConfig) Backend {
	switch cfg.PreferredBackend {
	case "zellij":
		return &ZellijBackend{}
	case "tmux":
		return &TmuxBackend{}
	case "auto", "":
		return DetectBackend()
	default:
		return nil
	}
}

// execCommand is a wrapper around os/exec.CommandContext for testability.
var execCommand = func(ctx context.Context, name string, args ...string) *exec.Cmd {
	return exec.CommandContext(ctx, name, args...)
}

// runCommand executes a command and returns its combined stdout+stderr.
func runCommand(ctx context.Context, name string, args ...string) (string, error) {
	cmd := execCommand(ctx, name, args...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("%s: %w (output: %s)", name, err, string(out))
	}
	return string(out), nil
}
