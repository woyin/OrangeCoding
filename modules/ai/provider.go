package ai

import (
	"context"
	"fmt"
	"strings"
	"time"
)

// ---------------------------------------------------------------------------
// AiProvider interface
// ---------------------------------------------------------------------------

// AiProvider is the interface that all AI provider adapters must implement.
type AiProvider interface {
	// Name returns the provider's display name.
	Name() string

	// ChatCompletion sends a non-streaming chat completion request.
	ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error)

	// ChatCompletionStream sends a streaming chat completion request and returns
	// a channel of StreamEvent. The channel is closed when the stream ends.
	ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error)
}

// ---------------------------------------------------------------------------
// ProviderConfig
// ---------------------------------------------------------------------------

// ProviderConfig holds the configuration for creating an AI provider.
type ProviderConfig struct {
	APIKey       string
	APISecret    string
	BaseURL      string
	DefaultModel string
	TimeoutSecs  uint64
	Extra        map[string]string
}

// Timeout returns the configured timeout or a default of 120 seconds.
func (c ProviderConfig) Timeout() time.Duration {
	if c.TimeoutSecs > 0 {
		return time.Duration(c.TimeoutSecs) * time.Second
	}
	return 120 * time.Second
}

// ---------------------------------------------------------------------------
// ProviderFactory
// ---------------------------------------------------------------------------

// ProviderFactory creates AiProvider instances by name.
type ProviderFactory struct{}

const (
	defaultMoonshotBaseURL = "https://api.moonshot.ai/v1"
	defaultBigModelBaseURL = "https://api.z.ai/api/paas/v4"

	defaultOpenAIModel    = "gpt-5.1"
	defaultAnthropicModel = "claude-opus-4-7"
	defaultKimiModel      = "kimi-k2.6"
	defaultGLMModel       = "glm-5.1"
)

// NormalizeProviderConfig fills compatibility defaults for common model families.
func NormalizeProviderConfig(name string, config ProviderConfig) ProviderConfig {
	normalized := strings.ToLower(strings.TrimSpace(name))
	if config.DefaultModel == "" {
		config.DefaultModel = DefaultModelForProvider(normalized)
	}
	if config.BaseURL == "" {
		config.BaseURL = DefaultBaseURLForProvider(normalized)
	}
	if usesThinkingReasoningFormat(normalized) {
		if config.Extra == nil {
			config.Extra = make(map[string]string)
		}
		if config.Extra["reasoning_format"] == "" {
			config.Extra["reasoning_format"] = "thinking"
		}
	}
	return config
}

// DefaultModelForProvider returns the model used when a compatible provider has no explicit model.
func DefaultModelForProvider(name string) string {
	switch strings.ToLower(strings.TrimSpace(name)) {
	case "openai", "gpt":
		return defaultOpenAIModel
	case "anthropic", "claude", "opus":
		return defaultAnthropicModel
	case "kimi", "moonshot":
		return defaultKimiModel
	case "glm", "bigmodel", "zhipu":
		return defaultGLMModel
	default:
		return ""
	}
}

func usesThinkingReasoningFormat(name string) bool {
	switch strings.ToLower(strings.TrimSpace(name)) {
	case "kimi", "moonshot", "glm", "bigmodel", "zhipu":
		return true
	default:
		return false
	}
}

// DefaultBaseURLForProvider returns a compatibility endpoint for provider aliases.
func DefaultBaseURLForProvider(name string) string {
	switch strings.ToLower(strings.TrimSpace(name)) {
	case "kimi", "moonshot":
		return defaultMoonshotBaseURL
	case "glm", "bigmodel", "zhipu":
		return defaultBigModelBaseURL
	default:
		return ""
	}
}

// CreateProvider creates an AiProvider for the given name using the provided config.
// Supported names (case-insensitive):
//   - "openai", "gpt", "zai", "z.ai", "zen", "opencode-zen", "kimi", "moonshot", "glm", "bigmodel", "zhipu" -> OpenAI-compatible
//   - "anthropic", "claude", "opus" -> Anthropic-compatible
//   - "deepseek" -> DeepSeek
//   - "qianwen", "tongyi", "dashscope" -> Qianwen
//   - "wenxin", "ernie", "baidu" -> Wenxin
func (f *ProviderFactory) CreateProvider(name string, config ProviderConfig) (AiProvider, error) {
	normalized := strings.ToLower(strings.TrimSpace(name))
	config = NormalizeProviderConfig(normalized, config)
	switch normalized {
	case "openai", "gpt", "zai", "z.ai", "zen", "opencode-zen", "kimi", "moonshot", "glm", "bigmodel", "zhipu":
		return NewOpenAIProvider(config), nil
	case "anthropic", "claude", "opus":
		return NewAnthropicProvider(config), nil
	case "deepseek":
		return NewDeepSeekProvider(config), nil
	case "qianwen", "tongyi", "dashscope":
		return NewQianwenProvider(config), nil
	case "wenxin", "ernie", "baidu":
		return NewWenxinProvider(config), nil
	default:
		return nil, NewAiUnsupportedProviderError(
			fmt.Sprintf("unsupported provider: %q", name),
		)
	}
}
