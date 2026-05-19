package ai

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync"
	"time"
)

// ---------------------------------------------------------------------------
// Wenxin / Baidu ERNIE provider
// ---------------------------------------------------------------------------

const defaultWenxinBaseURL = "https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop"

// WenxinProvider implements AiProvider for the Baidu ERNIE (Wenxin) API.
// It uses OAuth token-based authentication.
type WenxinProvider struct {
	config     ProviderConfig
	client     *http.Client
	baseURL    string

	mu          sync.Mutex
	accessToken string
	tokenExpiry time.Time
}

// NewWenxinProvider creates a new Wenxin provider with the given config.
func NewWenxinProvider(config ProviderConfig) *WenxinProvider {
	baseURL := config.BaseURL
	if baseURL == "" {
		baseURL = defaultWenxinBaseURL
	}
	return &WenxinProvider{
		config:  config,
		client:  &http.Client{Timeout: config.Timeout()},
		baseURL: baseURL,
	}
}

// Name returns "wenxin".
func (p *WenxinProvider) Name() string { return "wenxin" }

// ---------------------------------------------------------------------------
// Wenxin response wire types
// ---------------------------------------------------------------------------

type wenxinResponse struct {
	ID      string `json:"id"`
	Object  string `json:"object"`
	Result  string `json:"result"`  // Wenxin uses "result" instead of "content"
	Model   string `json:"model"`
	Usage   wenxinUsage `json:"usage"`
}

type wenxinUsage struct {
	PromptTokens     uint32 `json:"prompt_tokens"`
	CompletionTokens uint32 `json:"completion_tokens"`
	TotalTokens      uint32 `json:"total_tokens"`
}

// ---------------------------------------------------------------------------
// ChatCompletion (non-streaming)
// ---------------------------------------------------------------------------

// ChatCompletion sends a non-streaming request to the Wenxin API.
func (p *WenxinProvider) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
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

	body, err := json.Marshal(reqBody)
	if err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to marshal request: %s", err))
	}

	accessToken, err := p.getAccessToken(ctx)
	if err != nil {
		return nil, err
	}

	// Wenxin uses model-specific endpoints, e.g., /completions_pro?access_token=xxx
	url := p.buildURL(accessToken, model)
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, NewAiNetworkError(fmt.Sprintf("failed to create request: %s", err))
	}

	req.Header.Set("Content-Type", "application/json")

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

	var result wenxinResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to decode response: %s", err))
	}

	return &AiResponse{
		Content: result.Result,
		Model:   result.Model,
		Usage: TokenUsage{
			PromptTokens:     result.Usage.PromptTokens,
			CompletionTokens: result.Usage.CompletionTokens,
			TotalTokens:      result.Usage.TotalTokens,
		},
		FinishReason: "stop",
	}, nil
}

// ---------------------------------------------------------------------------
// ChatCompletionStream (streaming)
// ---------------------------------------------------------------------------

// ChatCompletionStream sends a streaming request to the Wenxin API.
func (p *WenxinProvider) ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error) {
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

	body, err := json.Marshal(reqBody)
	if err != nil {
		return nil, NewAiParseError(fmt.Sprintf("failed to marshal request: %s", err))
	}

	accessToken, err := p.getAccessToken(ctx)
	if err != nil {
		return nil, err
	}

	url := p.buildURL(accessToken, model)
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, NewAiNetworkError(fmt.Sprintf("failed to create request: %s", err))
	}

	req.Header.Set("Content-Type", "application/json")

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

// buildURL constructs the Wenxin API URL with the access token.
// The model name maps to a specific endpoint path.
func (p *WenxinProvider) buildURL(accessToken, model string) string {
	endpoint := p.modelToEndpoint(model)
	return p.baseURL + "/" + endpoint + "?access_token=" + accessToken
}

// modelToEndpoint maps a model name to its Wenxin API endpoint path.
func (p *WenxinProvider) modelToEndpoint(model string) string {
	// Common model-to-endpoint mappings for Baidu ERNIE
	switch strings.ToLower(model) {
	case "ernie-4.0", "ernie-4.0-8k", "completions_pro":
		return "completions_pro"
	case "ernie-3.5", "ernie-3.5-8k", "completions":
		return "completions"
	case "ernie-speed", "ernie-speed-8k":
		return "ernie_speed"
	case "ernie-lite", "ernie-lite-8k":
		return "ernie_lite"
	case "ernie-bot-4":
		return "completions_pro"
	default:
		// Default to completions_pro for unknown models
		return "completions_pro"
	}
}

// getAccessToken obtains an access token using the API key and secret.
// It caches the token and refreshes it when expired.
func (p *WenxinProvider) getAccessToken(ctx context.Context) (string, error) {
	p.mu.Lock()
	defer p.mu.Unlock()

	if p.accessToken != "" && time.Now().Before(p.tokenExpiry) {
		return p.accessToken, nil
	}

	tokenURL := fmt.Sprintf(
		"https://aip.baidubce.com/oauth/2.0/token?grant_type=client_credentials&client_id=%s&client_secret=%s",
		p.config.APIKey,
		p.config.APISecret,
	)

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, tokenURL, nil)
	if err != nil {
		return "", NewAiNetworkError(fmt.Sprintf("failed to create token request: %s", err))
	}

	resp, err := p.client.Do(req)
	if err != nil {
		return "", NewAiAuthError(fmt.Sprintf("token request failed: %s", err))
	}
	defer resp.Body.Close()

	var tokenResp struct {
		AccessToken string `json:"access_token"`
		ExpiresIn   int64  `json:"expires_in"`
		Error       string `json:"error,omitempty"`
	}

	if err := json.NewDecoder(resp.Body).Decode(&tokenResp); err != nil {
		return "", NewAiParseError(fmt.Sprintf("failed to decode token response: %s", err))
	}

	if tokenResp.Error != "" {
		return "", NewAiAuthError(fmt.Sprintf("token error: %s", tokenResp.Error))
	}

	if tokenResp.AccessToken == "" {
		return "", NewAiAuthError("empty access token received")
	}

	p.accessToken = tokenResp.AccessToken
	// Set expiry to 30 days minus 5 minutes buffer
	expirySeconds := tokenResp.ExpiresIn
	if expirySeconds <= 0 {
		expirySeconds = 2592000 // 30 days default
	}
	p.tokenExpiry = time.Now().Add(time.Duration(expirySeconds)*time.Second - 5*time.Minute)

	return p.accessToken, nil
}

func (p *WenxinProvider) readStream(resp *http.Response, ch chan<- StreamEvent) {
	defer close(ch)
	defer resp.Body.Close()

	payloads, _ := ParseSSEStream(resp.Body)

	for _, payload := range payloads {
		// Wenxin streaming returns incremental results
		var chunk struct {
			Result string `json:"result"`
			IsEnd  bool   `json:"is_end"`
		}
		if err := json.Unmarshal([]byte(payload), &chunk); err != nil {
			continue
		}

		if chunk.Result != "" {
			ch <- StreamEvent{
				Type:    "content_delta",
				Content: chunk.Result,
			}
		}

		if chunk.IsEnd {
			ch <- StreamEvent{Type: "done"}
			return
		}
	}

	ch <- StreamEvent{Type: "done"}
}
