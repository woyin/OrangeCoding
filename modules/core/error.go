package core

import "fmt"

// ---------------------------------------------------------------------------
// ErrorKind (iota enum)
// ---------------------------------------------------------------------------

// ErrorKind classifies the category of an error.
type ErrorKind int

const (
	ErrConfig       ErrorKind = iota // configuration error
	ErrIO                             // I/O error
	ErrNetwork                        // network error
	ErrProvider                       // LLM provider error
	ErrAgent                          // agent error
	ErrTool                           // tool error
	ErrProtocol                       // protocol error
	ErrSerialization                  // serialization error
	ErrAuth                           // authentication error
	ErrInternal                       // internal error
)

// String returns the human-readable name of the error kind.
func (k ErrorKind) String() string {
	switch k {
	case ErrConfig:
		return "config"
	case ErrIO:
		return "io"
	case ErrNetwork:
		return "network"
	case ErrProvider:
		return "provider"
	case ErrAgent:
		return "agent"
	case ErrTool:
		return "tool"
	case ErrProtocol:
		return "protocol"
	case ErrSerialization:
		return "serialization"
	case ErrAuth:
		return "auth"
	case ErrInternal:
		return "internal"
	default:
		return fmt.Sprintf("unknown-error-kind(%d)", k)
	}
}

// ---------------------------------------------------------------------------
// OrangeError
// ---------------------------------------------------------------------------

// OrangeError is the unified error type for the OrangeCoding system.
type OrangeError struct {
	kind    ErrorKind
	message string
	cause   error
}

// Error formats the error as "kind: message" or "kind: message: cause".
func (e *OrangeError) Error() string {
	if e.cause != nil {
		return fmt.Sprintf("%s: %s: %s", e.kind, e.message, e.cause)
	}
	return fmt.Sprintf("%s: %s", e.kind, e.message)
}

// Unwrap returns the underlying cause, enabling errors.Is and errors.As.
func (e *OrangeError) Unwrap() error {
	return e.cause
}

// Kind returns the error kind.
func (e *OrangeError) Kind() ErrorKind {
	return e.kind
}

// IsRetryable returns true for errors that may succeed on retry.
// Only ErrNetwork and ErrProvider are considered retryable.
func (e *OrangeError) IsRetryable() bool {
	return e.kind == ErrNetwork || e.kind == ErrProvider
}

// ---------------------------------------------------------------------------
// WrapError
// ---------------------------------------------------------------------------

// WrapError wraps an existing error with a kind and message.
func WrapError(cause error, kind ErrorKind, message string) *OrangeError {
	return &OrangeError{
		kind:    kind,
		message: message,
		cause:   cause,
	}
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

// NewConfigError creates a configuration error.
func NewConfigError(msg string) *OrangeError {
	return &OrangeError{kind: ErrConfig, message: msg}
}

// NewIOError creates an I/O error.
func NewIOError(msg string) *OrangeError {
	return &OrangeError{kind: ErrIO, message: msg}
}

// NewNetworkError creates a network error.
func NewNetworkError(msg string) *OrangeError {
	return &OrangeError{kind: ErrNetwork, message: msg}
}

// NewProviderError creates an LLM provider error.
func NewProviderError(msg string) *OrangeError {
	return &OrangeError{kind: ErrProvider, message: msg}
}

// NewProtocolError creates a protocol error.
func NewProtocolError(msg string) *OrangeError {
	return &OrangeError{kind: ErrProtocol, message: msg}
}

// NewSerializationError creates a serialization error.
func NewSerializationError(msg string) *OrangeError {
	return &OrangeError{kind: ErrSerialization, message: msg}
}

// NewAuthError creates an authentication error.
func NewAuthError(msg string) *OrangeError {
	return &OrangeError{kind: ErrAuth, message: msg}
}

// NewInternalError creates an internal error.
func NewInternalError(msg string) *OrangeError {
	return &OrangeError{kind: ErrInternal, message: msg}
}

// NewToolError creates a tool error with the message formatted as "[toolName] msg".
func NewToolError(toolName string, msg string) *OrangeError {
	return &OrangeError{kind: ErrTool, message: fmt.Sprintf("[%s] %s", toolName, msg)}
}

// NewAgentError creates an agent error with the message formatted as "[agentId] msg".
func NewAgentError(agentId string, msg string) *OrangeError {
	return &OrangeError{kind: ErrAgent, message: fmt.Sprintf("[%s] %s", agentId, msg)}
}
