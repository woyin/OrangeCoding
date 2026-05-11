package ai

import (
	"context"
	"net/http"
)

// ---------------------------------------------------------------------------
// Qianwen / Tongyi / DashScope provider (OpenAI-compatible)
// ---------------------------------------------------------------------------

const defaultQianwenBaseURL = "https://dashscope.aliyuncs.com/compatible-mode/v1"

// QianwenProvider implements AiProvider for the Qianwen (Tongyi/DashScope) API.
// Qianwen uses an OpenAI-compatible API format via the compatible-mode endpoint.
type QianwenProvider struct {
	config  ProviderConfig
	client  *http.Client
	baseURL string
}

// NewQianwenProvider creates a new Qianwen provider with the given config.
func NewQianwenProvider(config ProviderConfig) *QianwenProvider {
	baseURL := config.BaseURL
	if baseURL == "" {
		baseURL = defaultQianwenBaseURL
	}
	return &QianwenProvider{
		config:  config,
		client:  &http.Client{Timeout: config.Timeout()},
		baseURL: baseURL,
	}
}

// Name returns "qianwen".
func (p *QianwenProvider) Name() string { return "qianwen" }

// ---------------------------------------------------------------------------
// ChatCompletion (non-streaming) — delegates to OpenAI-compatible format
// ---------------------------------------------------------------------------

// ChatCompletion sends a non-streaming request to the Qianwen API.
func (p *QianwenProvider) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
	model := opts.Model
	if model == "" {
		model = p.config.DefaultModel
	}

	reqBody := openAIRequest{
		Model:       model,
		Messages:    messages,
		Tools:       tools,
		Temperature: opts.Temperature,
		MaxTokens:   opts.MaxTokens,
		TopP:        opts.TopP,
		Stop:        opts.StopSequences,
	}

	return doOpenAIRequest(ctx, p.client, p.baseURL+"/chat/completions", p.config.APIKey, reqBody)
}

// ---------------------------------------------------------------------------
// ChatCompletionStream (streaming)
// ---------------------------------------------------------------------------

// ChatCompletionStream sends a streaming request and returns a channel of events.
func (p *QianwenProvider) ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error) {
	model := opts.Model
	if model == "" {
		model = p.config.DefaultModel
	}

	reqBody := openAIRequest{
		Model:       model,
		Messages:    messages,
		Tools:       tools,
		Stream:      true,
		Temperature: opts.Temperature,
		MaxTokens:   opts.MaxTokens,
		TopP:        opts.TopP,
		Stop:        opts.StopSequences,
	}

	return doOpenAIStreamRequest(ctx, p.client, p.baseURL+"/chat/completions", p.config.APIKey, reqBody)
}
