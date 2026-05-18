package ai

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
)

// ---------------------------------------------------------------------------
// OpenAI-compatible provider
// ---------------------------------------------------------------------------

const defaultOpenAIBaseURL = "https://api.openai.com/v1"

// OpenAIProvider implements AiProvider for OpenAI and compatible APIs.
type OpenAIProvider struct {
	config  ProviderConfig
	client  *http.Client
	baseURL string
}

// NewOpenAIProvider creates a new OpenAI provider with the given config.
func NewOpenAIProvider(config ProviderConfig) *OpenAIProvider {
	baseURL := config.BaseURL
	if baseURL == "" {
		baseURL = defaultOpenAIBaseURL
	}
	return &OpenAIProvider{
		config:  config,
		client:  &http.Client{Timeout: config.Timeout()},
		baseURL: baseURL,
	}
}

// Name returns "openai".
func (p *OpenAIProvider) Name() string { return "openai" }

// ---------------------------------------------------------------------------
// Request / Response wire types (OpenAI format)
// ---------------------------------------------------------------------------

type openAIRequest struct {
	Model           string           `json:"model"`
	Messages        []ChatMessage    `json:"messages"`
	Tools           []ToolDefinition `json:"tools,omitempty"`
	Stream          bool             `json:"stream,omitempty"`
	Temperature     *float64         `json:"temperature,omitempty"`
	MaxTokens       *uint32          `json:"max_tokens,omitempty"`
	TopP            *float64         `json:"top_p,omitempty"`
	Stop            []string         `json:"stop,omitempty"`
	ReasoningEffort string           `json:"reasoning_effort,omitempty"`
	Thinking        *openAIThinking  `json:"thinking,omitempty"`
}

type openAIThinking struct {
	Type string `json:"type"`
}

type openAIResponse struct {
	ID      string         `json:"id"`
	Object  string         `json:"object"`
	Model   string         `json:"model"`
	Choices []openAIChoice `json:"choices"`
	Usage   openAIUsage    `json:"usage"`
}

type openAIChoice struct {
	Index        int       `json:"index"`
	Message      openAIMsg `json:"message"`
	FinishReason string    `json:"finish_reason"`
}

type openAIMsg struct {
	Role      string     `json:"role"`
	Content   string     `json:"content"`
	ToolCalls []ToolCall `json:"tool_calls,omitempty"`
}

type openAIUsage struct {
	PromptTokens     uint32 `json:"prompt_tokens"`
	CompletionTokens uint32 `json:"completion_tokens"`
	TotalTokens      uint32 `json:"total_tokens"`
}

// Streaming wire types
type openAIStreamChunk struct {
	ID      string              `json:"id"`
	Object  string              `json:"object"`
	Choices []openAIDeltaChoice `json:"choices"`
}

type openAIDeltaChoice struct {
	Index        int         `json:"index"`
	Delta        openAIDelta `json:"delta"`
	FinishReason *string     `json:"finish_reason"`
}

type openAIDelta struct {
	Role      string            `json:"role,omitempty"`
	Content   string            `json:"content,omitempty"`
	ToolCalls []openAIToolDelta `json:"tool_calls,omitempty"`
}

type openAIToolDelta struct {
	Index    int              `json:"index"`
	ID       string           `json:"id,omitempty"`
	Type     string           `json:"type,omitempty"`
	Function *openAIFuncDelta `json:"function,omitempty"`
}

type openAIFuncDelta struct {
	Name      string `json:"name,omitempty"`
	Arguments string `json:"arguments,omitempty"`
}

// ---------------------------------------------------------------------------
// ChatCompletion (non-streaming)
// ---------------------------------------------------------------------------

// ChatCompletion sends a non-streaming request to the OpenAI API.
func (p *OpenAIProvider) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
	model := opts.Model
	if model == "" {
		model = p.config.DefaultModel
	}

	reqBody := p.newOpenAIRequest(model, messages, tools, opts)
	reqBody.Stream = false

	body, err := json.Marshal(reqBody)
	if err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to marshal request: %s", err))
	}

	url := p.baseURL + "/chat/completions"
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, NewAiNetworkError(fmt.Sprintf("failed to create request: %s", err))
	}

	p.setHeaders(req)

	resp, err := p.client.Do(req)
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

	return p.convertResponse(&result), nil
}

func (p *OpenAIProvider) newOpenAIRequest(model string, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) openAIRequest {
	reqBody := openAIRequest{
		Model:           model,
		Messages:        messages,
		Tools:           tools,
		Temperature:     opts.Temperature,
		MaxTokens:       opts.MaxTokens,
		TopP:            opts.TopP,
		Stop:            opts.StopSequences,
		ReasoningEffort: opts.ReasoningEffort,
	}
	if p.config.Extra["reasoning_format"] == "thinking" && opts.ReasoningEffort != "" && opts.ReasoningEffort != "none" {
		reqBody.ReasoningEffort = ""
		reqBody.Thinking = &openAIThinking{Type: "enabled"}
	}
	return reqBody
}

// ---------------------------------------------------------------------------
// ChatCompletionStream (streaming)
// ---------------------------------------------------------------------------

// ChatCompletionStream sends a streaming request and returns a channel of events.
func (p *OpenAIProvider) ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error) {
	model := opts.Model
	if model == "" {
		model = p.config.DefaultModel
	}

	reqBody := p.newOpenAIRequest(model, messages, tools, opts)
	reqBody.Stream = true

	body, err := json.Marshal(reqBody)
	if err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to marshal request: %s", err))
	}

	url := p.baseURL + "/chat/completions"
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, NewAiNetworkError(fmt.Sprintf("failed to create request: %s", err))
	}

	p.setHeaders(req)

	resp, err := p.client.Do(req)
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
	go p.readStream(resp, ch)
	return ch, nil
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

// setHeaders sets the common headers for OpenAI-compatible requests.
func (p *OpenAIProvider) setHeaders(req *http.Request) {
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+p.config.APIKey)
}

func (p *OpenAIProvider) convertResponse(r *openAIResponse) *AiResponse {
	result := &AiResponse{
		Model: r.Model,
		Usage: TokenUsage{
			PromptTokens:     r.Usage.PromptTokens,
			CompletionTokens: r.Usage.CompletionTokens,
			TotalTokens:      r.Usage.TotalTokens,
		},
	}

	if len(r.Choices) > 0 {
		choice := r.Choices[0]
		result.Content = choice.Message.Content
		result.FinishReason = choice.FinishReason
		result.ToolCalls = choice.Message.ToolCalls
	}

	return result
}

func (p *OpenAIProvider) readStream(resp *http.Response, ch chan<- StreamEvent) {
	defer close(ch)
	defer resp.Body.Close()

	payloads := ParseSSEStream(resp.Body)

	// Accumulate tool call data across chunks
	type toolCallAcc struct {
		ID        strings.Builder
		Name      strings.Builder
		Arguments strings.Builder
	}
	toolCalls := make(map[int]*toolCallAcc)

	for _, payload := range payloads {
		var chunk openAIStreamChunk
		if err := json.Unmarshal([]byte(payload), &chunk); err != nil {
			ch <- StreamEvent{Type: "done"}
			return
		}

		for _, choice := range chunk.Choices {
			// Content delta
			if choice.Delta.Content != "" {
				ch <- StreamEvent{
					Type:    "content_delta",
					Content: choice.Delta.Content,
				}
			}

			// Tool call deltas
			for _, tc := range choice.Delta.ToolCalls {
				acc, ok := toolCalls[tc.Index]
				if !ok {
					acc = &toolCallAcc{}
					toolCalls[tc.Index] = acc
				}
				if tc.ID != "" {
					acc.ID.WriteString(tc.ID)
				}
				if tc.Type != "" {
					// type is always "function" for OpenAI
				}
				if tc.Function != nil {
					if tc.Function.Name != "" {
						acc.Name.WriteString(tc.Function.Name)
					}
					if tc.Function.Arguments != "" {
						acc.Arguments.WriteString(tc.Function.Arguments)
					}
				}
			}

			// Finish
			if choice.FinishReason != nil {
				// Emit accumulated tool calls
				for i := 0; i < len(toolCalls); i++ {
					if acc, ok := toolCalls[i]; ok {
						ch <- StreamEvent{
							Type:         "tool_call_delta",
							ToolCallID:   acc.ID.String(),
							ToolCallName: acc.Name.String(),
							Arguments:    acc.Arguments.String(),
						}
					}
				}
				ch <- StreamEvent{Type: "done"}
				return
			}
		}
	}

	// Stream ended without explicit finish
	ch <- StreamEvent{Type: "done"}
}
