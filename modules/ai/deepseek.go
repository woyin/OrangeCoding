package ai

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
)

// ---------------------------------------------------------------------------
// DeepSeek provider (OpenAI-compatible)
// ---------------------------------------------------------------------------

const defaultDeepSeekBaseURL = "https://api.deepseek.com/v1"

// DeepSeekProvider implements AiProvider for the DeepSeek API.
// DeepSeek uses an OpenAI-compatible API format.
type DeepSeekProvider struct {
	config  ProviderConfig
	client  *http.Client
	baseURL string
}

// NewDeepSeekProvider creates a new DeepSeek provider with the given config.
func NewDeepSeekProvider(config ProviderConfig) *DeepSeekProvider {
	baseURL := config.BaseURL
	if baseURL == "" {
		baseURL = defaultDeepSeekBaseURL
	}
	return &DeepSeekProvider{
		config:  config,
		client:  &http.Client{Timeout: config.Timeout()},
		baseURL: baseURL,
	}
}

// Name returns "deepseek".
func (p *DeepSeekProvider) Name() string { return "deepseek" }

// ---------------------------------------------------------------------------
// ChatCompletion (non-streaming) — delegates to OpenAI-compatible format
// ---------------------------------------------------------------------------

// ChatCompletion sends a non-streaming request to the DeepSeek API.
func (p *DeepSeekProvider) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
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
func (p *DeepSeekProvider) ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error) {
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

// ---------------------------------------------------------------------------
// Shared OpenAI-compatible helpers
// ---------------------------------------------------------------------------

// doOpenAIRequest performs a non-streaming OpenAI-compatible request.
// Used by DeepSeek and Qianwen providers.
func doOpenAIRequest(ctx context.Context, client *http.Client, url, apiKey string, reqBody openAIRequest) (*AiResponse, error) {
	body, err := json.Marshal(reqBody)
	if err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to marshal request: %s", err))
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, NewAiNetworkError(fmt.Sprintf("failed to create request: %s", err))
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+apiKey)

	resp, err := client.Do(req)
	if err != nil {
		return nil, NewAiNetworkError(fmt.Sprintf("request failed: %s", err))
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		respBody, _ := io.ReadAll(resp.Body)
		return nil, NewAiApiError(
			fmt.Sprintf("API returned status %d: %s", resp.StatusCode, string(respBody)),
			uint16(resp.StatusCode),
		)
	}

	var result openAIResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to decode response: %s", err))
	}

	aiResp := &AiResponse{
		Model: result.Model,
		Usage: TokenUsage{
			PromptTokens:     result.Usage.PromptTokens,
			CompletionTokens: result.Usage.CompletionTokens,
			TotalTokens:      result.Usage.TotalTokens,
		},
	}

	if len(result.Choices) > 0 {
		choice := result.Choices[0]
		aiResp.Content = choice.Message.Content
		aiResp.FinishReason = choice.FinishReason
		aiResp.ToolCalls = choice.Message.ToolCalls
	}

	return aiResp, nil
}

// doOpenAIStreamRequest performs a streaming OpenAI-compatible request.
// Used by DeepSeek and Qianwen providers.
func doOpenAIStreamRequest(ctx context.Context, client *http.Client, url, apiKey string, reqBody openAIRequest) (<-chan StreamEvent, error) {
	body, err := json.Marshal(reqBody)
	if err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to marshal request: %s", err))
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, NewAiNetworkError(fmt.Sprintf("failed to create request: %s", err))
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+apiKey)

	resp, err := client.Do(req)
	if err != nil {
		return nil, NewAiNetworkError(fmt.Sprintf("request failed: %s", err))
	}

	if resp.StatusCode != http.StatusOK {
		respBody, _ := io.ReadAll(resp.Body)
		resp.Body.Close()
		return nil, NewAiApiError(
			fmt.Sprintf("API returned status %d: %s", resp.StatusCode, string(respBody)),
			uint16(resp.StatusCode),
		)
	}

	ch := make(chan StreamEvent, 64)
	go readOpenAIStream(resp, ch)
	return ch, nil
}

// readOpenAIStream reads an OpenAI-compatible SSE stream and emits events.
func readOpenAIStream(resp *http.Response, ch chan<- StreamEvent) {
	defer close(ch)
	defer resp.Body.Close()

	payloads, _ := ParseSSEStream(resp.Body)

	type toolCallAcc struct {
		id        string
		name      string
		arguments string
	}
	toolCalls := make(map[int]*toolCallAcc)

	for _, payload := range payloads {
		var chunk openAIStreamChunk
		if err := json.Unmarshal([]byte(payload), &chunk); err != nil {
			ch <- StreamEvent{Type: "done"}
			return
		}

		for _, choice := range chunk.Choices {
			if choice.Delta.Content != "" {
				ch <- StreamEvent{
					Type:    "content_delta",
					Content: choice.Delta.Content,
				}
			}

			for _, tc := range choice.Delta.ToolCalls {
				acc, ok := toolCalls[tc.Index]
				if !ok {
					acc = &toolCallAcc{}
					toolCalls[tc.Index] = acc
				}
				if tc.ID != "" {
					acc.id = tc.ID
				}
				if tc.Function != nil {
					if tc.Function.Name != "" {
						acc.name = tc.Function.Name
					}
					if tc.Function.Arguments != "" {
						acc.arguments += tc.Function.Arguments
					}
				}
			}

			if choice.FinishReason != nil {
				for i := 0; i < len(toolCalls); i++ {
					if acc, ok := toolCalls[i]; ok {
						ch <- StreamEvent{
							Type:         "tool_call_delta",
							ToolCallID:   acc.id,
							ToolCallName: acc.name,
							Arguments:    acc.arguments,
						}
					}
				}
				ch <- StreamEvent{Type: "done"}
				return
			}
		}
	}

	ch <- StreamEvent{Type: "done"}
}
