package multiplexer

import "time"

// PaneState represents the lifecycle state of a managed pane.
type PaneState int

const (
	PaneStateCreated PaneState = iota
	PaneStateRunning
	PaneStateExited
	PaneStateError
)

func (s PaneState) String() string {
	switch s {
	case PaneStateCreated:
		return "created"
	case PaneStateRunning:
		return "running"
	case PaneStateExited:
		return "exited"
	case PaneStateError:
		return "error"
	default:
		return "unknown"
	}
}

// PaneInfo holds metadata about a managed terminal pane.
type PaneInfo struct {
	ID        string    // backend-specific pane identifier
	Name      string    // human-readable agent name
	PID       int       // OS process ID of the pane's shell (if available)
	State     PaneState
	CreatedAt time.Time
	Backend   string // "zellij" or "tmux"
}
