package ai

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

// ---------------------------------------------------------------------------
// TestAiError
// ---------------------------------------------------------------------------

func TestAiErrorKindString(t *testing.T) {
	cases := map[AiErrorKind]string{
		AiErrNetwork:            "network",
		AiErrApi:                "api",
		AiErrAuth:               "auth",
		AiErrParse:              "parse",
		AiErrStream:             "stream",
		AiErrConfig:             "config",
		AiErrUnsupportedProvider: "unsupported-provider",
		AiErrRateLimit:          "rate-limit",
		AiErrTimeout:            "timeout",
	}
	for kind, expected := range cases {
		if got := kind.String(); got != expected {
			t.Errorf("AiErrorKind(%d).String() = %q, want %q", kind, got, expected)
		}
	}
}

func TestAiErrorError(t *testing.T) {
	err := &AiError{Kind: AiErrApi, Message: "bad request"}
	want := "ai: api: bad request"
	if got := err.Error(); got != want {
		t.Errorf("AiError.Error() = %q, want %q", got, want)
	}
}

func TestAiErrorIsRetryable(t *testing.T) {
	tests := []struct {
		kind     AiErrorKind
		expected bool
	}{
		{AiErrNetwork, true},
		{AiErrRateLimit, true},
		{AiErrTimeout, true},
		{AiErrApi, false},
		{AiErrAuth, false},
		{AiErrParse, false},
		{AiErrConfig, false},
		{AiErrUnsupportedProvider, false},
	}
	for _, tt := range tests {
		err := &AiError{Kind: tt.kind}
		if got := err.IsRetryable(); got != tt.expected {
			t.Errorf("AiError{Kind: %d}.IsRetryable() = %v, want %v", tt.kind, got, tt.expected)
		}
	}
}

func TestAiErrorConstructors(t *testing.T) {
	err := NewAiNetworkError("conn refused")
	if err.Kind != AiErrNetwork || err.Message != "conn refused" {
		t.Errorf("NewAiNetworkError: got %+v", err)
	}

	err2 := NewAiApiError("bad", 400)
	if err2.Kind != AiErrApi || err2.StatusCode != 400 {
		t.Errorf("NewAiApiError: got %+v", err2)
	}

	err3 := NewAiRateLimitError("slow down", 60)
	if err3.Kind != AiErrRateLimit || err3.RetryAfter != 60 {
		t.Errorf("NewAiRateLimitError: got %+v", err3)
	}

	err4 := NewAiAuthError("bad key")
	if err4.Kind != AiErrAuth {
		t.Errorf("NewAiAuthError: got %+v", err4)
	}

	err5 := NewAiParseError("json fail")
	if err5.Kind != AiErrParse {
		t.Errorf("NewAiParseError: got %+v", err5)
	}

	err6 := NewAiStreamError("broken")
	if err6.Kind != AiErrStream {
		t.Errorf("NewAiStreamError: got %+v", err6)
	}

	err7 := NewAiConfigError("missing key")
	if err7.Kind != AiErrConfig {
		t.Errorf("NewAiConfigError: got %+v", err7)
	}

	err8 := NewAiUnsupportedProviderError("foo")
	if err8.Kind != AiErrUnsupportedProvider {
		t.Errorf("NewAiUnsupportedProviderError: got %+v", err8)
	}

	err9 := NewAiTimeoutError("timed out")
	if err9.Kind != AiErrTimeout {
		t.Errorf("NewAiTimeoutError: got %+v", err9)
	}
}

// ---------------------------------------------------------------------------
// TestChatMessageConstructors
// ---------------------------------------------------------------------------

func TestSystemMsg(t *testing.T) {
	msg := SystemMsg("you are helpful")
	if msg.Role != "system" || msg.Content != "you are helpful" {
		t.Errorf("SystemMsg: got %+v", msg)
	}
}

func TestUserMsg(t *testing.T) {
	msg := UserMsg("hello")
	if msg.Role != "user" || msg.Content != "hello" {
		t.Errorf("UserMsg: got %+v", msg)
	}
}

func TestAssistantMsg(t *testing.T) {
	msg := AssistantMsg("hi there")
	if msg.Role != "assistant" || msg.Content != "hi there" {
		t.Errorf("AssistantMsg: got %+v", msg)
	}
}

func TestToolResultMsg(t *testing.T) {
	msg := ToolResultMsg("call-123", "result data")
	if msg.Role != "tool" || msg.ToolCallID != "call-123" || msg.Content != "result data" {
		t.Errorf("ToolResultMsg: got %+v", msg)
	}
}

func TestAssistantMsgWithTools(t *testing.T) {
	toolCalls := []ToolCall{
		{ID: "tc-1", Type: "function", Function: FunctionCall{Name: "read_file", Arguments: `{"path": "/tmp"}`}},
	}
	msg := AssistantMsgWithTools(toolCalls)
	if msg.Role != "assistant" || len(msg.ToolCalls) != 1 || msg.ToolCalls[0].ID != "tc-1" {
		t.Errorf("AssistantMsgWithTools: got %+v", msg)
	}
}

// ---------------------------------------------------------------------------
// TestSSEStreamParsing
// ---------------------------------------------------------------------------

func TestSSEStreamParsing(t *testing.T) {
	input := `data: {"content": "hello"}

data: {"content": " world"}

: this is a comment
data: [DONE]

data: {"content": "after done"}
`
	payloads := ParseSSEStream(strings.NewReader(input))

	if len(payloads) != 3 {
		t.Fatalf("ParseSSEStream: got %d payloads, want 3", len(payloads))
	}
	if payloads[0] != `{"content": "hello"}` {
		t.Errorf("payload[0] = %q", payloads[0])
	}
	if payloads[1] != `{"content": " world"}` {
		t.Errorf("payload[1] = %q", payloads[1])
	}
	if payloads[2] != `{"content": "after done"}` {
		t.Errorf("payload[2] = %q", payloads[2])
	}
}

func TestSSEStreamParsingEmpty(t *testing.T) {
	payloads := ParseSSEStream(strings.NewReader(""))
	if len(payloads) != 0 {
		t.Errorf("ParseSSEStream(empty): got %d payloads, want 0", len(payloads))
	}
}

// ---------------------------------------------------------------------------
// TestProviderFactory
// ---------------------------------------------------------------------------

func TestProviderFactoryKnownNames(t *testing.T) {
	factory := &ProviderFactory{}
	config := ProviderConfig{APIKey: "test-key"}

	names := map[string]string{
		"openai":        "openai",
		"OpenAI":        "openai",
		"zai":           "openai",
		"z.ai":          "openai",
		"zen":           "openai",
		"opencode-zen":  "openai",
		"anthropic":     "anthropic",
		"claude":        "anthropic",
		"CLAUDE":        "anthropic",
		"deepseek":      "deepseek",
		"DeepSeek":      "deepseek",
		"qianwen":       "qianwen",
		"tongyi":        "qianwen",
		"dashscope":     "qianwen",
		"wenxin":        "wenxin",
		"ernie":         "wenxin",
		"baidu":         "wenxin",
	}

	for name, expectedProvider := range names {
		provider, err := factory.CreateProvider(name, config)
		if err != nil {
			t.Errorf("CreateProvider(%q): unexpected error: %v", name, err)
			continue
		}
		if provider.Name() != expectedProvider {
			t.Errorf("CreateProvider(%q).Name() = %q, want %q", name, provider.Name(), expectedProvider)
		}
	}
}

func TestProviderFactoryUnknownName(t *testing.T) {
	factory := &ProviderFactory{}
	config := ProviderConfig{APIKey: "test-key"}

	_, err := factory.CreateProvider("unknown", config)
	if err == nil {
		t.Fatal("CreateProvider(unknown) should return error")
	}

	aiErr, ok := err.(*AiError)
	if !ok {
		t.Fatalf("expected *AiError, got %T", err)
	}
	if aiErr.Kind != AiErrUnsupportedProvider {
		t.Errorf("error kind = %d, want AiErrUnsupportedProvider", aiErr.Kind)
	}
}

// ---------------------------------------------------------------------------
// TestModelRouter
// ---------------------------------------------------------------------------

func TestModelRouterRouting(t *testing.T) {
	rules := []RoutingRule{
		{Category: CategoryCoding, Provider: "openai", Model: "gpt-4"},
		{Category: CategoryPlanning, Provider: "anthropic", Model: "claude-3"},
		{Category: CategoryReview, Provider: "deepseek", Model: "deepseek-coder"},
	}
	router := NewModelRouter(rules)

	tests := []struct {
		category       ModelCategory
		wantProvider   string
		wantModel      string
	}{
		{CategoryCoding, "openai", "gpt-4"},
		{CategoryPlanning, "anthropic", "claude-3"},
		{CategoryReview, "deepseek", "deepseek-coder"},
	}

	for _, tt := range tests {
		provider, model := router.Route(tt.category)
		if provider != tt.wantProvider || model != tt.wantModel {
			t.Errorf("Route(%v) = (%q, %q), want (%q, %q)",
				tt.category, provider, model, tt.wantProvider, tt.wantModel)
		}
	}
}

func TestModelRouterDefaultFallback(t *testing.T) {
	rules := []RoutingRule{
		{Category: CategoryCoding, Provider: "openai", Model: "gpt-4"},
	}
	router := NewModelRouter(rules)

	// Category not in rules should fall back to first rule
	provider, model := router.Route(CategoryCreative)
	if provider != "openai" || model != "gpt-4" {
		t.Errorf("Route(CategoryCreative) = (%q, %q), want default (openai, gpt-4)",
			provider, model)
	}
}

func TestModelRouterEmpty(t *testing.T) {
	router := NewModelRouter(nil)
	provider, model := router.Route(CategoryGeneral)
	if provider != "openai" || model != "gpt-4" {
		t.Errorf("Route(General) with empty rules = (%q, %q), want (openai, gpt-4)",
			provider, model)
	}
}

func TestModelCategoryString(t *testing.T) {
	cases := map[ModelCategory]string{
		CategoryCoding:   "coding",
		CategoryPlanning: "planning",
		CategoryReview:   "review",
		CategoryAnswer:   "answer",
		CategoryExplore:  "explore",
		CategoryCreative: "creative",
		CategoryAnalysis: "analysis",
		CategoryGeneral:  "general",
	}
	for cat, expected := range cases {
		if got := cat.String(); got != expected {
			t.Errorf("ModelCategory(%d).String() = %q, want %q", cat, got, expected)
		}
	}
}

// ---------------------------------------------------------------------------
// TestFallbackChain
// ---------------------------------------------------------------------------

type mockProvider struct {
	name    string
	respond func(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error)
}

func (m *mockProvider) Name() string { return m.name }

func (m *mockProvider) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
	return m.respond(ctx, messages, tools, opts)
}

func (m *mockProvider) ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error) {
	return nil, fmt.Errorf("mock stream not implemented")
}

func TestFallbackChainSuccessOnFirst(t *testing.T) {
	p1 := &mockProvider{
		name: "provider-1",
		respond: func(_ context.Context, _ []ChatMessage, _ []ToolDefinition, _ ChatOptions) (*AiResponse, error) {
			return &AiResponse{Content: "ok", Model: "test"}, nil
		},
	}
	p2 := &mockProvider{name: "provider-2"}

	chain := NewFallbackChain([]AiProvider{p1, p2}, 5*time.Second)
	resp, err := chain.ChatCompletion(context.Background(), nil, nil, ChatOptions{})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}
	if resp.Content != "ok" {
		t.Errorf("Content = %q, want %q", resp.Content, "ok")
	}
}

func TestFallbackChainFallsBackOnError(t *testing.T) {
	callCount := 0
	p1 := &mockProvider{
		name: "provider-1",
		respond: func(_ context.Context, _ []ChatMessage, _ []ToolDefinition, _ ChatOptions) (*AiResponse, error) {
			callCount++
			return nil, NewAiApiError("server error", 500)
		},
	}
	p2 := &mockProvider{
		name: "provider-2",
		respond: func(_ context.Context, _ []ChatMessage, _ []ToolDefinition, _ ChatOptions) (*AiResponse, error) {
			callCount++
			return &AiResponse{Content: "fallback", Model: "test"}, nil
		},
	}

	chain := NewFallbackChain([]AiProvider{p1, p2}, 5*time.Second)
	resp, err := chain.ChatCompletion(context.Background(), nil, nil, ChatOptions{})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}
	if resp.Content != "fallback" {
		t.Errorf("Content = %q, want %q", resp.Content, "fallback")
	}
	if callCount != 2 {
		t.Errorf("callCount = %d, want 2", callCount)
	}
}

func TestFallbackChainCooldown(t *testing.T) {
	p1 := &mockProvider{
		name: "provider-1",
		respond: func(_ context.Context, _ []ChatMessage, _ []ToolDefinition, _ ChatOptions) (*AiResponse, error) {
			return nil, NewAiApiError("server error", 500)
		},
	}

	chain := NewFallbackChain([]AiProvider{p1}, 5*time.Second)

	// First call fails, sets cooldown
	_, err := chain.ChatCompletion(context.Background(), nil, nil, ChatOptions{})
	if err == nil {
		t.Fatal("expected error from first call")
	}

	// Provider should be on cooldown
	if !chain.IsCoolingDown(0) {
		t.Error("provider should be on cooldown after failure")
	}

	// Second call should also fail (provider on cooldown, no other providers)
	_, err = chain.ChatCompletion(context.Background(), nil, nil, ChatOptions{})
	if err == nil {
		t.Fatal("expected error from second call (cooldown)")
	}
}

func TestFallbackChainAllFail(t *testing.T) {
	p1 := &mockProvider{
		name: "provider-1",
		respond: func(_ context.Context, _ []ChatMessage, _ []ToolDefinition, _ ChatOptions) (*AiResponse, error) {
			return nil, NewAiNetworkError("conn refused")
		},
	}
	p2 := &mockProvider{
		name: "provider-2",
		respond: func(_ context.Context, _ []ChatMessage, _ []ToolDefinition, _ ChatOptions) (*AiResponse, error) {
			return nil, NewAiApiError("bad request", 400)
		},
	}

	chain := NewFallbackChain([]AiProvider{p1, p2}, 5*time.Second)
	_, err := chain.ChatCompletion(context.Background(), nil, nil, ChatOptions{})
	if err == nil {
		t.Fatal("expected error when all providers fail")
	}
	// Should return the last error
	aiErr, ok := err.(*AiError)
	if !ok {
		t.Fatalf("expected *AiError, got %T", err)
	}
	if aiErr.Kind != AiErrApi {
		t.Errorf("error kind = %d, want AiErrApi", aiErr.Kind)
	}
}

// ---------------------------------------------------------------------------
// TestOpenAIProviderWithMockServer
// ---------------------------------------------------------------------------

func TestOpenAIProviderWithMockServer(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Verify auth header
		auth := r.Header.Get("Authorization")
		if auth != "Bearer test-key" {
			t.Errorf("Authorization header = %q, want %q", auth, "Bearer test-key")
		}

		// Verify request body
		var reqBody openAIRequest
		if err := json.NewDecoder(r.Body).Decode(&reqBody); err != nil {
			t.Errorf("failed to decode request: %v", err)
			http.Error(w, "bad request", 400)
			return
		}

		if reqBody.Model != "gpt-4" {
			t.Errorf("model = %q, want %q", reqBody.Model, "gpt-4")
		}

		// Return a mock response
		resp := openAIResponse{
			ID:    "chatcmpl-test",
			Model: "gpt-4",
			Choices: []openAIChoice{
				{
					Message: openAIMsg{
						Role:    "assistant",
						Content: "Hello! How can I help you?",
					},
					FinishReason: "stop",
				},
			},
			Usage: openAIUsage{
				PromptTokens:     10,
				CompletionTokens: 8,
				TotalTokens:      18,
			},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewOpenAIProvider(ProviderConfig{
		APIKey:       "test-key",
		BaseURL:      server.URL,
		DefaultModel: "gpt-4",
	})

	messages := []ChatMessage{
		SystemMsg("You are helpful."),
		UserMsg("Hello!"),
	}

	resp, err := provider.ChatCompletion(context.Background(), messages, nil, ChatOptions{Model: "gpt-4"})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}

	if resp.Content != "Hello! How can I help you?" {
		t.Errorf("Content = %q, want %q", resp.Content, "Hello! How can I help you?")
	}
	if resp.Model != "gpt-4" {
		t.Errorf("Model = %q, want %q", resp.Model, "gpt-4")
	}
	if resp.Usage.PromptTokens != 10 || resp.Usage.CompletionTokens != 8 || resp.Usage.TotalTokens != 18 {
		t.Errorf("Usage = %+v", resp.Usage)
	}
	if resp.FinishReason != "stop" {
		t.Errorf("FinishReason = %q, want %q", resp.FinishReason, "stop")
	}
}

func TestOpenAIProviderWithToolCalls(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		resp := openAIResponse{
			ID:    "chatcmpl-tools",
			Model: "gpt-4",
			Choices: []openAIChoice{
				{
					Message: openAIMsg{
						Role: "assistant",
						ToolCalls: []ToolCall{
							{
								ID:   "call-1",
								Type: "function",
								Function: FunctionCall{
									Name:      "read_file",
									Arguments: `{"path":"/tmp/test.txt"}`,
								},
							},
						},
					},
					FinishReason: "tool_calls",
				},
			},
			Usage: openAIUsage{PromptTokens: 20, CompletionTokens: 15, TotalTokens: 35},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewOpenAIProvider(ProviderConfig{
		APIKey:  "test-key",
		BaseURL: server.URL,
	})

	resp, err := provider.ChatCompletion(context.Background(), nil, nil, ChatOptions{Model: "gpt-4"})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}

	if len(resp.ToolCalls) != 1 {
		t.Fatalf("ToolCalls length = %d, want 1", len(resp.ToolCalls))
	}
	tc := resp.ToolCalls[0]
	if tc.ID != "call-1" || tc.Function.Name != "read_file" {
		t.Errorf("ToolCall = %+v", tc)
	}
	if tc.Function.Arguments != `{"path":"/tmp/test.txt"}` {
		t.Errorf("Arguments = %q", tc.Function.Arguments)
	}
}

func TestOpenAIProviderStreaming(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")
		w.Header().Set("Cache-Control", "no-cache")

		flusher, ok := w.(http.Flusher)
		if !ok {
			t.Error("response writer does not support flushing")
			return
		}

		// Send content deltas
		chunks := []string{
			`{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}`,
			`{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}`,
			`{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"!"},"finish_reason":"stop"}]}`,
		}
		for _, chunk := range chunks {
			fmt.Fprintf(w, "data: %s\n\n", chunk)
			flusher.Flush()
		}
		fmt.Fprintf(w, "data: [DONE]\n\n")
		flusher.Flush()
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewOpenAIProvider(ProviderConfig{
		APIKey:  "test-key",
		BaseURL: server.URL,
	})

	ch, err := provider.ChatCompletionStream(context.Background(), nil, nil, ChatOptions{Model: "gpt-4"})
	if err != nil {
		t.Fatalf("ChatCompletionStream: unexpected error: %v", err)
	}

	var contentParts []string
	var gotDone bool
	for evt := range ch {
		switch evt.Type {
		case "content_delta":
			contentParts = append(contentParts, evt.Content)
		case "done":
			gotDone = true
		}
	}

	if !gotDone {
		t.Error("stream did not send done event")
	}
	fullContent := strings.Join(contentParts, "")
	if fullContent != "Hello world!" {
		t.Errorf("streamed content = %q, want %q", fullContent, "Hello world!")
	}
}

func TestOpenAIProviderAPIError(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusTooManyRequests)
		io.WriteString(w, `{"error": {"message": "rate limit exceeded"}}`)
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewOpenAIProvider(ProviderConfig{
		APIKey:  "test-key",
		BaseURL: server.URL,
	})

	_, err := provider.ChatCompletion(context.Background(), nil, nil, ChatOptions{Model: "gpt-4"})
	if err == nil {
		t.Fatal("expected error for 429 response")
	}

	aiErr, ok := err.(*AiError)
	if !ok {
		t.Fatalf("expected *AiError, got %T", err)
	}
	if aiErr.Kind != AiErrApi {
		t.Errorf("error kind = %d, want AiErrApi", aiErr.Kind)
	}
	if aiErr.StatusCode != 429 {
		t.Errorf("StatusCode = %d, want 429", aiErr.StatusCode)
	}
}

// ---------------------------------------------------------------------------
// TestAnthropicProviderWithMockServer
// ---------------------------------------------------------------------------

func TestAnthropicProviderWithMockServer(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Verify auth headers
		apiKey := r.Header.Get("x-api-key")
		if apiKey != "test-key" {
			t.Errorf("x-api-key = %q, want %q", apiKey, "test-key")
		}
		version := r.Header.Get("anthropic-version")
		if version != "2023-06-01" {
			t.Errorf("anthropic-version = %q, want %q", version, "2023-06-01")
		}

		// Verify request
		var reqBody anthropicRequest
		if err := json.NewDecoder(r.Body).Decode(&reqBody); err != nil {
			t.Errorf("failed to decode request: %v", err)
			http.Error(w, "bad request", 400)
			return
		}

		// System prompt should be in the separate field, not in messages
		if reqBody.System != "You are helpful." {
			t.Errorf("system = %q, want %q", reqBody.System, "You are helpful.")
		}

		// Messages should not contain system messages
		for _, msg := range reqBody.Messages {
			if msg.Role == "system" {
				t.Error("system message should be extracted from messages list")
			}
		}

		// Return mock response
		resp := anthropicResponse{
			ID:   "msg-test",
			Type: "message",
			Role: "assistant",
			Content: []anthropicContent{
				{Type: "text", Text: "I can help with that!"},
			},
			Model:      "claude-3",
			StopReason: "end_turn",
			Usage:      anthropicUsage{InputTokens: 15, OutputTokens: 10},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewAnthropicProvider(ProviderConfig{
		APIKey:       "test-key",
		BaseURL:      server.URL,
		DefaultModel: "claude-3",
	})

	messages := []ChatMessage{
		SystemMsg("You are helpful."),
		UserMsg("Hello!"),
	}

	resp, err := provider.ChatCompletion(context.Background(), messages, nil, ChatOptions{Model: "claude-3"})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}

	if resp.Content != "I can help with that!" {
		t.Errorf("Content = %q, want %q", resp.Content, "I can help with that!")
	}
	if resp.Usage.PromptTokens != 15 || resp.Usage.CompletionTokens != 10 || resp.Usage.TotalTokens != 25 {
		t.Errorf("Usage = %+v", resp.Usage)
	}
}

func TestAnthropicProviderWithToolUse(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		resp := anthropicResponse{
			ID:   "msg-tools",
			Type: "message",
			Role: "assistant",
			Content: []anthropicContent{
				{Type: "text", Text: "Let me read that file."},
				{
					Type:  "tool_use",
					ID:    "toolu-1",
					Name:  "read_file",
					Input: json.RawMessage(`{"path":"/tmp/test.txt"}`),
				},
			},
			Model:      "claude-3",
			StopReason: "tool_use",
			Usage:      anthropicUsage{InputTokens: 25, OutputTokens: 20},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewAnthropicProvider(ProviderConfig{
		APIKey:  "test-key",
		BaseURL: server.URL,
	})

	resp, err := provider.ChatCompletion(context.Background(), nil, nil, ChatOptions{Model: "claude-3"})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}

	if resp.Content != "Let me read that file." {
		t.Errorf("Content = %q, want %q", resp.Content, "Let me read that file.")
	}
	if len(resp.ToolCalls) != 1 {
		t.Fatalf("ToolCalls length = %d, want 1", len(resp.ToolCalls))
	}
	tc := resp.ToolCalls[0]
	if tc.ID != "toolu-1" || tc.Function.Name != "read_file" {
		t.Errorf("ToolCall = %+v", tc)
	}
	if tc.Function.Arguments != `{"path":"/tmp/test.txt"}` {
		t.Errorf("Arguments = %q", tc.Function.Arguments)
	}
}

func TestAnthropicProviderStreaming(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")

		flusher, ok := w.(http.Flusher)
		if !ok {
			t.Error("response writer does not support flushing")
			return
		}

		events := []string{
			`{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}`,
			`{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" there"}}`,
			`{"type":"message_delta","delta":{"stop_reason":"end_turn"},"message":{"usage":{"input_tokens":10,"output_tokens":5}}}`,
		}
		for _, evt := range events {
			fmt.Fprintf(w, "data: %s\n\n", evt)
			flusher.Flush()
		}
		fmt.Fprintf(w, "data: [DONE]\n\n")
		flusher.Flush()
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewAnthropicProvider(ProviderConfig{
		APIKey:  "test-key",
		BaseURL: server.URL,
	})

	ch, err := provider.ChatCompletionStream(context.Background(), nil, nil, ChatOptions{Model: "claude-3"})
	if err != nil {
		t.Fatalf("ChatCompletionStream: unexpected error: %v", err)
	}

	var contentParts []string
	var gotDone bool
	for evt := range ch {
		switch evt.Type {
		case "content_delta":
			contentParts = append(contentParts, evt.Content)
		case "done":
			gotDone = true
		}
	}

	if !gotDone {
		t.Error("stream did not send done event")
	}
	fullContent := strings.Join(contentParts, "")
	if fullContent != "Hi there" {
		t.Errorf("streamed content = %q, want %q", fullContent, "Hi there")
	}
}

// ---------------------------------------------------------------------------
// TestDeepSeekProvider
// ---------------------------------------------------------------------------

func TestDeepSeekProviderName(t *testing.T) {
	p := NewDeepSeekProvider(ProviderConfig{APIKey: "test"})
	if p.Name() != "deepseek" {
		t.Errorf("Name() = %q, want %q", p.Name(), "deepseek")
	}
}

func TestDeepSeekProviderWithMockServer(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		auth := r.Header.Get("Authorization")
		if auth != "Bearer ds-key" {
			t.Errorf("Authorization = %q", auth)
		}

		resp := openAIResponse{
			Model: "deepseek-coder",
			Choices: []openAIChoice{
				{
					Message:      openAIMsg{Role: "assistant", Content: "deepseek response"},
					FinishReason: "stop",
				},
			},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewDeepSeekProvider(ProviderConfig{
		APIKey:  "ds-key",
		BaseURL: server.URL,
	})

	resp, err := provider.ChatCompletion(context.Background(), nil, nil, ChatOptions{Model: "deepseek-coder"})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}
	if resp.Content != "deepseek response" {
		t.Errorf("Content = %q, want %q", resp.Content, "deepseek response")
	}
}

// ---------------------------------------------------------------------------
// TestQianwenProvider
// ---------------------------------------------------------------------------

func TestQianwenProviderName(t *testing.T) {
	p := NewQianwenProvider(ProviderConfig{APIKey: "test"})
	if p.Name() != "qianwen" {
		t.Errorf("Name() = %q, want %q", p.Name(), "qianwen")
	}
}

func TestQianwenProviderWithMockServer(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		auth := r.Header.Get("Authorization")
		if auth != "Bearer qw-key" {
			t.Errorf("Authorization = %q", auth)
		}

		resp := openAIResponse{
			Model: "qwen-turbo",
			Choices: []openAIChoice{
				{
					Message:      openAIMsg{Role: "assistant", Content: "qianwen response"},
					FinishReason: "stop",
				},
			},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	server := httptest.NewServer(handler)
	defer server.Close()

	provider := NewQianwenProvider(ProviderConfig{
		APIKey:  "qw-key",
		BaseURL: server.URL,
	})

	resp, err := provider.ChatCompletion(context.Background(), nil, nil, ChatOptions{Model: "qwen-turbo"})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}
	if resp.Content != "qianwen response" {
		t.Errorf("Content = %q, want %q", resp.Content, "qianwen response")
	}
}

// ---------------------------------------------------------------------------
// TestWenxinProvider
// ---------------------------------------------------------------------------

func TestWenxinProviderName(t *testing.T) {
	p := NewWenxinProvider(ProviderConfig{APIKey: "test", APISecret: "secret"})
	if p.Name() != "wenxin" {
		t.Errorf("Name() = %q, want %q", p.Name(), "wenxin")
	}
}

func TestWenxinProviderModelToEndpoint(t *testing.T) {
	p := NewWenxinProvider(ProviderConfig{APIKey: "test", APISecret: "secret"})

	tests := []struct {
		model    string
		expected string
	}{
		{"ernie-4.0", "completions_pro"},
		{"ernie-4.0-8k", "completions_pro"},
		{"completions_pro", "completions_pro"},
		{"ernie-3.5", "completions"},
		{"ernie-speed", "ernie_speed"},
		{"ernie-lite", "ernie_lite"},
		{"ernie-bot-4", "completions_pro"},
		{"unknown-model", "completions_pro"},
	}

	for _, tt := range tests {
		got := p.modelToEndpoint(tt.model)
		if got != tt.expected {
			t.Errorf("modelToEndpoint(%q) = %q, want %q", tt.model, got, tt.expected)
		}
	}
}

func TestWenxinProviderWithMockServer(t *testing.T) {
	tokenServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]interface{}{
			"access_token": "mock-token-123",
			"expires_in":   86400,
		})
	}))
	defer tokenServer.Close()

	apiServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Verify access_token is in URL
		if !strings.Contains(r.URL.String(), "access_token=mock-token-123") {
			t.Errorf("missing access_token in URL: %s", r.URL.String())
		}

		resp := wenxinResponse{
			ID:     "resp-1",
			Result: "wenxin response",
			Model:  "ernie-4.0",
			Usage:  wenxinUsage{PromptTokens: 5, CompletionTokens: 3, TotalTokens: 8},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	}))
	defer apiServer.Close()

	provider := NewWenxinProvider(ProviderConfig{
		APIKey:    "test-key",
		APISecret: "test-secret",
		BaseURL:   apiServer.URL,
	})

	// Override token URL for testing
	originalClient := provider.client
	_ = originalClient
	provider.baseURL = apiServer.URL

	// Note: In a real test we'd need to mock the token URL too.
	// For this test we manually set the access token.
	provider.mu.Lock()
	provider.accessToken = "mock-token-123"
	provider.tokenExpiry = time.Now().Add(1 * time.Hour)
	provider.mu.Unlock()

	resp, err := provider.ChatCompletion(context.Background(), nil, nil, ChatOptions{Model: "ernie-4.0"})
	if err != nil {
		t.Fatalf("ChatCompletion: unexpected error: %v", err)
	}
	if resp.Content != "wenxin response" {
		t.Errorf("Content = %q, want %q", resp.Content, "wenxin response")
	}
}

// ---------------------------------------------------------------------------
// TestProviderConfigTimeout
// ---------------------------------------------------------------------------

func TestProviderConfigTimeout(t *testing.T) {
	config := ProviderConfig{TimeoutSecs: 30}
	if config.Timeout() != 30*time.Second {
		t.Errorf("Timeout() = %v, want 30s", config.Timeout())
	}

	config = ProviderConfig{}
	if config.Timeout() != 120*time.Second {
		t.Errorf("default Timeout() = %v, want 120s", config.Timeout())
	}
}

// ---------------------------------------------------------------------------
// TestFallbackChainProviders
// ---------------------------------------------------------------------------

func TestFallbackChainProviders(t *testing.T) {
	p1 := &mockProvider{name: "a"}
	p2 := &mockProvider{name: "b"}
	chain := NewFallbackChain([]AiProvider{p1, p2}, 5*time.Second)

	providers := chain.Providers()
	if len(providers) != 2 {
		t.Fatalf("Providers() length = %d, want 2", len(providers))
	}
	if providers[0].Name() != "a" || providers[1].Name() != "b" {
		t.Errorf("Providers() names = %q, %q", providers[0].Name(), providers[1].Name())
	}
}

func TestFallbackChainIsCoolingDown(t *testing.T) {
	chain := NewFallbackChain(nil, 5*time.Second)
	if chain.IsCoolingDown(0) {
		t.Error("empty chain should not have cooling down providers")
	}
	if chain.IsCoolingDown(-1) {
		t.Error("negative index should return false")
	}
}
