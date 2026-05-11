package tools

import (
	"context"
	"encoding/json"
)

// Tool is the interface that every tool must implement.
type Tool interface {
	// Name returns the unique tool identifier (e.g. "bash", "read_file").
	Name() string

	// Description returns a human-readable description of what the tool does.
	Description() string

	// Parameters returns a JSON Schema describing the tool's input parameters.
	Parameters() json.RawMessage

	// Execute runs the tool with the given JSON input and returns a string result.
	Execute(ctx context.Context, input json.RawMessage) (string, error)

	// Metadata returns metadata about the tool's behaviour.
	Metadata() ToolMetadata
}

// ToolMetadata describes behavioural properties of a tool.
type ToolMetadata struct {
	IsReadOnly        bool `json:"is_read_only"`
	IsConcurrencySafe bool `json:"is_concurrency_safe"`
	IsDestructive     bool `json:"is_destructive"`
	IsEnabled         bool `json:"is_enabled"`
}

// DefaultMetadata returns metadata with only IsEnabled set to true.
func DefaultMetadata() ToolMetadata {
	return ToolMetadata{IsEnabled: true}
}

// ReadOnlyMetadata returns metadata for read-only, concurrency-safe tools.
func ReadOnlyMetadata() ToolMetadata {
	return ToolMetadata{IsReadOnly: true, IsConcurrencySafe: true, IsEnabled: true}
}

// DestructiveMetadata returns metadata for tools that modify the filesystem or state.
func DestructiveMetadata() ToolMetadata {
	return ToolMetadata{IsDestructive: true, IsEnabled: true}
}

// ToolError is a structured error returned by tool execution.
type ToolError struct {
	Kind    string // "invalid_params", "execution_error", "security_violation", "not_found"
	Message string
}

// Error implements the error interface.
func (e *ToolError) Error() string {
	return e.Kind + ": " + e.Message
}
