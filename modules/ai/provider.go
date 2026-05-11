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

// CreateProvider creates an AiProvider for the given name using the provided config.
// Supported names (case-insensitive):
//   - "openai", "zai", "z.ai", "zen", "opencode-zen" -> OpenAI
//   - "anthropic", "claude" -> Anthropic
//   - "deepseek" -> DeepSeek
//   - "qianwen", "tongyi", "dashscope" -> Qianwen
//   - "wenxin", "ernie", "baidu" -> Wenxin
func (f *ProviderFactory) CreateProvider(name string, config ProviderConfig) (AiProvider, error) {
	normalized := strings.ToLower(strings.TrimSpace(name))
	switch normalized {
	case "openai", "zai", "z.ai", "zen", "opencode-zen":
		return NewOpenAIProvider(config), nil
	case "anthropic", "claude":
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
