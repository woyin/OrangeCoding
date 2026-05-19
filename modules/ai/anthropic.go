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
// Anthropic / Claude provider
// ---------------------------------------------------------------------------

const defaultAnthropicBaseURL = "https://api.anthropic.com/v1"
const anthropicVersion = "2023-06-01"

// AnthropicProvider implements AiProvider for the Anthropic (Claude) API.
type AnthropicProvider struct {
	config  ProviderConfig
	client  *http.Client
	baseURL string
}

// NewAnthropicProvider creates a new Anthropic provider with the given config.
func NewAnthropicProvider(config ProviderConfig) *AnthropicProvider {
	baseURL := config.BaseURL
	if baseURL == "" {
		baseURL = defaultAnthropicBaseURL
	}
	return &AnthropicProvider{
		config:  config,
		client:  &http.Client{Timeout: config.Timeout()},
		baseURL: baseURL,
	}
}

// Name returns "anthropic".
func (p *AnthropicProvider) Name() string { return "anthropic" }

// ---------------------------------------------------------------------------
// Anthropic wire types
// ---------------------------------------------------------------------------

type anthropicRequest struct {
	Model         string             `json:"model"`
	Messages      []ChatMessage      `json:"messages"`
	System        string             `json:"system,omitempty"`
	MaxTokens     uint32             `json:"max_tokens"`
	Tools         []anthropicTool    `json:"tools,omitempty"`
	Stream        bool               `json:"stream,omitempty"`
	Temperature   *float64           `json:"temperature,omitempty"`
	TopP          *float64           `json:"top_p,omitempty"`
	StopSequences []string           `json:"stop_sequences,omitempty"`
	Thinking      *anthropicThinking `json:"thinking,omitempty"`
}

type anthropicThinking struct {
	Type         string `json:"type"`
	BudgetTokens uint32 `json:"budget_tokens"`
}

type anthropicTool struct {
	Name        string      `json:"name"`
	Description string      `json:"description"`
	InputSchema interface{} `json:"input_schema"`
}

type anthropicResponse struct {
	ID         string             `json:"id"`
	Type       string             `json:"type"`
	Role       string             `json:"role"`
	Content    []anthropicContent `json:"content"`
	Model      string             `json:"model"`
	StopReason string             `json:"stop_reason"`
	Usage      anthropicUsage     `json:"usage"`
}

type anthropicContent struct {
	Type  string          `json:"type"`
	Text  string          `json:"text,omitempty"`
	ID    string          `json:"id,omitempty"`
	Name  string          `json:"name,omitempty"`
	Input json.RawMessage `json:"input,omitempty"`
}

type anthropicUsage struct {
	InputTokens  uint32 `json:"input_tokens"`
	OutputTokens uint32 `json:"output_tokens"`
}

// Streaming wire types
type anthropicStreamEvent struct {
	Type         string                 `json:"type"`
	Index        int                    `json:"index,omitempty"`
	ContentBlock *anthropicContentBlock `json:"content_block,omitempty"`
	Delta        *anthropicDelta        `json:"delta,omitempty"`
	Message      *anthropicMsgDelta     `json:"message,omitempty"`
}

type anthropicContentBlock struct {
	Type  string          `json:"type"`
	ID    string          `json:"id,omitempty"`
	Name  string          `json:"name,omitempty"`
	Input json.RawMessage `json:"input,omitempty"`
	Text  string          `json:"text,omitempty"`
}

type anthropicDelta struct {
	Type        string `json:"type,omitempty"`
	Text        string `json:"text,omitempty"`
	PartialJSON string `json:"partial_json,omitempty"`
	StopReason  string `json:"stop_reason,omitempty"`
}

type anthropicMsgDelta struct {
	StopReason string          `json:"stop_reason,omitempty"`
	Usage      *anthropicUsage `json:"usage,omitempty"`
}

// ---------------------------------------------------------------------------
// ChatCompletion (non-streaming)
// ---------------------------------------------------------------------------

// ChatCompletion sends a non-streaming request to the Anthropic API.
func (p *AnthropicProvider) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
	model := opts.Model
	if model == "" {
		model = p.config.DefaultModel
	}

	// Extract system messages from the message list
	systemPrompt, filtered := p.extractSystemPrompt(messages)

	maxTokens := uint32(4096)
	if opts.MaxTokens != nil {
		maxTokens = *opts.MaxTokens
	}
	maxTokens = ensureAnthropicThinkingRoom(maxTokens, opts.ReasoningBudgetTokens)

	reqBody := anthropicRequest{
		Model:         model,
		Messages:      filtered,
		System:        systemPrompt,
		MaxTokens:     maxTokens,
		Temperature:   opts.Temperature,
		TopP:          opts.TopP,
		StopSequences: opts.StopSequences,
	}
	if opts.ReasoningBudgetTokens != nil && *opts.ReasoningBudgetTokens > 0 {
		reqBody.Thinking = &anthropicThinking{Type: "enabled", BudgetTokens: *opts.ReasoningBudgetTokens}
	}

	// Convert tools to Anthropic format (input_schema instead of parameters)
	if len(tools) > 0 {
		reqBody.Tools = p.convertTools(tools)
	}

	body, err := json.Marshal(reqBody)
	if err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to marshal request: %s", err))
	}

	url := p.baseURL + "/messages"
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

	var result anthropicResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to decode response: %s", err))
	}

	return p.convertResponse(&result), nil
}

// ---------------------------------------------------------------------------
// ChatCompletionStream (streaming)
// ---------------------------------------------------------------------------

// ChatCompletionStream sends a streaming request and returns a channel of events.
func (p *AnthropicProvider) ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error) {
	model := opts.Model
	if model == "" {
		model = p.config.DefaultModel
	}

	systemPrompt, filtered := p.extractSystemPrompt(messages)

	maxTokens := uint32(4096)
	if opts.MaxTokens != nil {
		maxTokens = *opts.MaxTokens
	}
	maxTokens = ensureAnthropicThinkingRoom(maxTokens, opts.ReasoningBudgetTokens)

	reqBody := anthropicRequest{
		Model:         model,
		Messages:      filtered,
		System:        systemPrompt,
		MaxTokens:     maxTokens,
		Stream:        true,
		Temperature:   opts.Temperature,
		TopP:          opts.TopP,
		StopSequences: opts.StopSequences,
	}
	if opts.ReasoningBudgetTokens != nil && *opts.ReasoningBudgetTokens > 0 {
		reqBody.Thinking = &anthropicThinking{Type: "enabled", BudgetTokens: *opts.ReasoningBudgetTokens}
	}

	if len(tools) > 0 {
		reqBody.Tools = p.convertTools(tools)
	}

	body, err := json.Marshal(reqBody)
	if err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to marshal request: %s", err))
	}

	url := p.baseURL + "/messages"
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

func ensureAnthropicThinkingRoom(maxTokens uint32, thinkingBudget *uint32) uint32 {
	if thinkingBudget == nil || *thinkingBudget == 0 {
		return maxTokens
	}
	if maxTokens > *thinkingBudget {
		return maxTokens
	}
	return *thinkingBudget + 1024
}

func (p *AnthropicProvider) setHeaders(req *http.Request) {
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("x-api-key", p.config.APIKey)
	req.Header.Set("anthropic-version", anthropicVersion)
}

// extractSystemPrompt removes system messages from the list and joins them
// into a single string for the separate "system" field.
func (p *AnthropicProvider) extractSystemPrompt(messages []ChatMessage) (string, []ChatMessage) {
	var systemParts []string
	var filtered []ChatMessage
	for _, msg := range messages {
		if msg.Role == "system" {
			systemParts = append(systemParts, msg.Content)
		} else {
			filtered = append(filtered, msg)
		}
	}
	return strings.Join(systemParts, "\n"), filtered
}

// convertTools converts ToolDefinition slices to Anthropic's tool format.
func (p *AnthropicProvider) convertTools(tools []ToolDefinition) []anthropicTool {
	result := make([]anthropicTool, len(tools))
	for i, t := range tools {
		result[i] = anthropicTool{
			Name:        t.Function.Name,
			Description: t.Function.Description,
			InputSchema: t.Function.Parameters,
		}
	}
	return result
}

func (p *AnthropicProvider) convertResponse(r *anthropicResponse) *AiResponse {
	result := &AiResponse{
		Model: r.Model,
		Usage: TokenUsage{
			PromptTokens:     r.Usage.InputTokens,
			CompletionTokens: r.Usage.OutputTokens,
			TotalTokens:      r.Usage.InputTokens + r.Usage.OutputTokens,
		},
		FinishReason: r.StopReason,
	}

	var contentParts []string
	for _, block := range r.Content {
		switch block.Type {
		case "text":
			contentParts = append(contentParts, block.Text)
		case "tool_use":
			args := "{}"
			if len(block.Input) > 0 {
				args = string(block.Input)
			}
			result.ToolCalls = append(result.ToolCalls, ToolCall{
				ID:   block.ID,
				Type: "function",
				Function: FunctionCall{
					Name:      block.Name,
					Arguments: args,
				},
			})
		}
	}
	result.Content = strings.Join(contentParts, "")

	return result
}

func (p *AnthropicProvider) readStream(resp *http.Response, ch chan<- StreamEvent) {
	defer close(ch)
	defer resp.Body.Close()

	payloads, _ := ParseSSEStream(resp.Body)

	for _, payload := range payloads {
		var evt anthropicStreamEvent
		if err := json.Unmarshal([]byte(payload), &evt); err != nil {
			continue
		}

		switch evt.Type {
		case "content_block_delta":
			if evt.Delta != nil {
				switch evt.Delta.Type {
				case "text_delta":
					ch <- StreamEvent{
						Type:    "content_delta",
						Content: evt.Delta.Text,
					}
				case "input_json_delta":
					// Tool call arguments being streamed
					// We need the content block info from a prior event
					ch <- StreamEvent{
						Type:      "tool_call_delta",
						Arguments: evt.Delta.PartialJSON,
					}
				}
			}

		case "content_block_start":
			if evt.ContentBlock != nil && evt.ContentBlock.Type == "tool_use" {
				ch <- StreamEvent{
					Type:         "tool_call_delta",
					ToolCallID:   evt.ContentBlock.ID,
					ToolCallName: evt.ContentBlock.Name,
				}
			}

		case "message_delta":
			if evt.Delta != nil && evt.Delta.StopReason != "" {
				// Usage info might come with the final delta
				usage := TokenUsage{}
				if evt.Message != nil && evt.Message.Usage != nil {
					usage = TokenUsage{
						PromptTokens:     evt.Message.Usage.InputTokens,
						CompletionTokens: evt.Message.Usage.OutputTokens,
						TotalTokens:      evt.Message.Usage.InputTokens + evt.Message.Usage.OutputTokens,
					}
				}
				ch <- StreamEvent{
					Type:  "usage",
					Usage: &usage,
				}
				ch <- StreamEvent{Type: "done"}
				return
			}

		case "message_stop":
			ch <- StreamEvent{Type: "done"}
			return
		}
	}

	ch <- StreamEvent{Type: "done"}
}
