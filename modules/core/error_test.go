package core

import (
	"errors"
	"fmt"
	"testing"
)

// ---------------------------------------------------------------------------
// NewConfigError: sets correct Kind, Error() format
// ---------------------------------------------------------------------------

func TestNewConfigError_SetsCorrectKind(t *testing.T) {
	err := NewConfigError("bad setting")
	if err.Kind() != ErrConfig {
		t.Errorf("expected kind %v, got %v", ErrConfig, err.Kind())
	}
}

func TestNewConfigError_ErrorFormat(t *testing.T) {
	err := NewConfigError("bad setting")
	want := "config: bad setting"
	if got := err.Error(); got != want {
		t.Errorf("Error() = %q, want %q", got, want)
	}
}

// ---------------------------------------------------------------------------
// WrapError: wraps inner error, errors.Is / errors.Unwrap work
// ---------------------------------------------------------------------------

func TestWrapError_WrapsCause(t *testing.T) {
	inner := fmt.Errorf("disk full")
	wrapped := WrapError(inner, ErrIO, "failed to write file")

	if wrapped.Kind() != ErrIO {
		t.Errorf("expected kind %v, got %v", ErrIO, wrapped.Kind())
	}
	if wrapped.Unwrap() != inner {
		t.Errorf("Unwrap() did not return the inner error")
	}
}

func TestWrapError_ErrorFormat_WithCause(t *testing.T) {
	inner := fmt.Errorf("disk full")
	wrapped := WrapError(inner, ErrIO, "failed to write file")

	want := "io: failed to write file: disk full"
	if got := wrapped.Error(); got != want {
		t.Errorf("Error() = %q, want %q", got, want)
	}
}

func TestWrapError_ErrorFormat_WithoutCause(t *testing.T) {
	err := NewConfigError("bad setting")
	// OrangeError created via convenience constructor has no cause
	want := "config: bad setting"
	if got := err.Error(); got != want {
		t.Errorf("Error() = %q, want %q", got, want)
	}
}

func TestErrorsIs_WorksThroughWrap(t *testing.T) {
	inner := fmt.Errorf("base error")
	wrapped := WrapError(inner, ErrIO, "something failed")

	if !errors.Is(wrapped, inner) {
		t.Error("errors.Is should find the inner error through WrapError")
	}
}

func TestErrorsUnwrap_WorksThroughWrap(t *testing.T) {
	inner := fmt.Errorf("base error")
	wrapped := WrapError(inner, ErrIO, "something failed")

	unwrapped := errors.Unwrap(wrapped)
	if unwrapped != inner {
		t.Error("errors.Unwrap should return the inner error")
	}
}

// ---------------------------------------------------------------------------
// IsRetryable: table-driven test for all error kinds
// ---------------------------------------------------------------------------

func TestIsRetryable(t *testing.T) {
	tests := []struct {
		kind       ErrorKind
		retryable  bool
	}{
		{ErrConfig, false},
		{ErrIO, false},
		{ErrNetwork, true},
		{ErrProvider, true},
		{ErrAgent, false},
		{ErrTool, false},
		{ErrProtocol, false},
		{ErrSerialization, false},
		{ErrAuth, false},
		{ErrInternal, false},
	}
	for _, tt := range tests {
		t.Run(tt.kind.String(), func(t *testing.T) {
			err := &OrangeError{kind: tt.kind, message: "test"}
			got := err.IsRetryable()
			if got != tt.retryable {
				t.Errorf("IsRetryable() for kind %v = %v, want %v", tt.kind, got, tt.retryable)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// All convenience constructors set correct Kind
// ---------------------------------------------------------------------------

func TestConvenienceConstructors(t *testing.T) {
	tests := []struct {
		name string
		err  *OrangeError
		kind ErrorKind
	}{
		{"NewConfigError", NewConfigError("msg"), ErrConfig},
		{"NewIOError", NewIOError("msg"), ErrIO},
		{"NewNetworkError", NewNetworkError("msg"), ErrNetwork},
		{"NewProviderError", NewProviderError("msg"), ErrProvider},
		{"NewProtocolError", NewProtocolError("msg"), ErrProtocol},
		{"NewSerializationError", NewSerializationError("msg"), ErrSerialization},
		{"NewAuthError", NewAuthError("msg"), ErrAuth},
		{"NewInternalError", NewInternalError("msg"), ErrInternal},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.err.Kind() != tt.kind {
				t.Errorf("%s: expected kind %v, got %v", tt.name, tt.kind, tt.err.Kind())
			}
		})
	}
}

// ---------------------------------------------------------------------------
// NewToolError: formats message as "[toolName] msg"
// ---------------------------------------------------------------------------

func TestNewToolError_Format(t *testing.T) {
	err := NewToolError("bash", "command not found")
	if err.Kind() != ErrTool {
		t.Errorf("expected kind %v, got %v", ErrTool, err.Kind())
	}
	want := "tool: [bash] command not found"
	if got := err.Error(); got != want {
		t.Errorf("Error() = %q, want %q", got, want)
	}
}

// ---------------------------------------------------------------------------
// NewAgentError: formats message as "[agentId] msg"
// ---------------------------------------------------------------------------

func TestNewAgentError_Format(t *testing.T) {
	err := NewAgentError("agent-123", "crashed")
	if err.Kind() != ErrAgent {
		t.Errorf("expected kind %v, got %v", ErrAgent, err.Kind())
	}
	want := "agent: [agent-123] crashed"
	if got := err.Error(); got != want {
		t.Errorf("Error() = %q, want %q", got, want)
	}
}

// ---------------------------------------------------------------------------
// errors.As works to extract OrangeError from wrapped chain
// ---------------------------------------------------------------------------

func TestErrorsAs_ExtractsOrangeError(t *testing.T) {
	inner := fmt.Errorf("base error")
	wrapped := WrapError(inner, ErrNetwork, "connection refused")

	// Wrap one more level with fmt.Errorf to create a chain
	outer := fmt.Errorf("outer: %w", wrapped)

	var orangeErr *OrangeError
	if !errors.As(outer, &orangeErr) {
		t.Error("errors.As should extract *OrangeError from the chain")
	}
	if orangeErr.Kind() != ErrNetwork {
		t.Errorf("extracted kind = %v, want %v", orangeErr.Kind(), ErrNetwork)
	}
	if orangeErr.Error() != "network: connection refused: base error" {
		t.Errorf("extracted Error() = %q", orangeErr.Error())
	}
}

// ---------------------------------------------------------------------------
// ErrorKind.String()
// ---------------------------------------------------------------------------

func TestErrorKind_String(t *testing.T) {
	tests := []struct {
		kind ErrorKind
		want string
	}{
		{ErrConfig, "config"},
		{ErrIO, "io"},
		{ErrNetwork, "network"},
		{ErrProvider, "provider"},
		{ErrAgent, "agent"},
		{ErrTool, "tool"},
		{ErrProtocol, "protocol"},
		{ErrSerialization, "serialization"},
		{ErrAuth, "auth"},
		{ErrInternal, "internal"},
	}
	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			if got := tt.kind.String(); got != tt.want {
				t.Errorf("String() = %q, want %q", got, tt.want)
			}
		})
	}
}
