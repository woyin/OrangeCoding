package ai

import "fmt"

// ---------------------------------------------------------------------------
// AiErrorKind (iota enum)
// ---------------------------------------------------------------------------

// AiErrorKind classifies the category of an AI-related error.
type AiErrorKind int

const (
	AiErrNetwork            AiErrorKind = iota // network connectivity error
	AiErrApi                                    // API returned an error response
	AiErrAuth                                   // authentication/authorization error
	AiErrParse                                  // response parsing error
	AiErrStream                                 // streaming error
	AiErrConfig                                 // configuration error
	AiErrUnsupportedProvider                    // unknown provider name
	AiErrRateLimit                              // rate limit exceeded
	AiErrTimeout                                // request timeout
)

// String returns the human-readable name of the error kind.
func (k AiErrorKind) String() string {
	switch k {
	case AiErrNetwork:
		return "network"
	case AiErrApi:
		return "api"
	case AiErrAuth:
		return "auth"
	case AiErrParse:
		return "parse"
	case AiErrStream:
		return "stream"
	case AiErrConfig:
		return "config"
	case AiErrUnsupportedProvider:
		return "unsupported-provider"
	case AiErrRateLimit:
		return "rate-limit"
	case AiErrTimeout:
		return "timeout"
	default:
		return fmt.Sprintf("unknown-ai-error(%d)", k)
	}
}

// ---------------------------------------------------------------------------
// AiError
// ---------------------------------------------------------------------------

// AiError is the error type for AI provider operations.
type AiError struct {
	Kind       AiErrorKind
	Message    string
	StatusCode uint16 // HTTP status code, relevant for AiErrApi
	RetryAfter uint64 // seconds to wait, relevant for AiErrRateLimit
}

// Error formats the error as "ai: kind: message".
func (e *AiError) Error() string {
	return fmt.Sprintf("ai: %s: %s", e.Kind, e.Message)
}

// IsRetryable returns true for error kinds that may succeed on retry.
func (e *AiError) IsRetryable() bool {
	return e.Kind == AiErrNetwork || e.Kind == AiErrRateLimit || e.Kind == AiErrTimeout
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

// NewAiNetworkError creates a network error.
func NewAiNetworkError(msg string) *AiError {
	return &AiError{Kind: AiErrNetwork, Message: msg}
}

// NewAiApiError creates an API error with a status code.
func NewAiApiError(msg string, statusCode uint16) *AiError {
	return &AiError{Kind: AiErrApi, Message: msg, StatusCode: statusCode}
}

// NewAiAuthError creates an authentication error.
func NewAiAuthError(msg string) *AiError {
	return &AiError{Kind: AiErrAuth, Message: msg}
}

// NewAiParseError creates a response parsing error.
func NewAiParseError(msg string) *AiError {
	return &AiError{Kind: AiErrParse, Message: msg}
}

// NewAiStreamError creates a streaming error.
func NewAiStreamError(msg string) *AiError {
	return &AiError{Kind: AiErrStream, Message: msg}
}

// NewAiConfigError creates a configuration error.
func NewAiConfigError(msg string) *AiError {
	return &AiError{Kind: AiErrConfig, Message: msg}
}

// NewAiUnsupportedProviderError creates an unsupported provider error.
func NewAiUnsupportedProviderError(msg string) *AiError {
	return &AiError{Kind: AiErrUnsupportedProvider, Message: msg}
}

// NewAiRateLimitError creates a rate limit error with retry-after duration.
func NewAiRateLimitError(msg string, retryAfter uint64) *AiError {
	return &AiError{Kind: AiErrRateLimit, Message: msg, RetryAfter: retryAfter}
}

// NewAiTimeoutError creates a timeout error.
func NewAiTimeoutError(msg string) *AiError {
	return &AiError{Kind: AiErrTimeout, Message: msg}
}
