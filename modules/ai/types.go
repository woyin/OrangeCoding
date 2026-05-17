package ai

// ---------------------------------------------------------------------------
// Wire types for AI provider communication
// ---------------------------------------------------------------------------

// ChatMessage represents a single message in a conversation.
type ChatMessage struct {
	Role       string     `json:"role"`
	Content    string     `json:"content,omitempty"`
	Name       string     `json:"name,omitempty"`
	ToolCallID string     `json:"tool_call_id,omitempty"`
	ToolCalls  []ToolCall `json:"tool_calls,omitempty"`
}

// SystemMsg creates a system message.
func SystemMsg(content string) ChatMessage {
	return ChatMessage{Role: "system", Content: content}
}

// UserMsg creates a user message.
func UserMsg(content string) ChatMessage {
	return ChatMessage{Role: "user", Content: content}
}

// AssistantMsg creates an assistant message.
func AssistantMsg(content string) ChatMessage {
	return ChatMessage{Role: "assistant", Content: content}
}

// ToolResultMsg creates a tool result message.
func ToolResultMsg(toolCallID, content string) ChatMessage {
	return ChatMessage{Role: "tool", ToolCallID: toolCallID, Content: content}
}

// AssistantMsgWithTools creates an assistant message with tool calls.
func AssistantMsgWithTools(toolCalls []ToolCall) ChatMessage {
	return ChatMessage{Role: "assistant", ToolCalls: toolCalls}
}

// ---------------------------------------------------------------------------
// Tool types
// ---------------------------------------------------------------------------

// ToolCall represents a tool call from the AI model.
type ToolCall struct {
	ID       string       `json:"id"`
	Type     string       `json:"type"`
	Function FunctionCall `json:"function"`
}

// FunctionCall represents the function name and arguments within a tool call.
type FunctionCall struct {
	Name      string `json:"name"`
	Arguments string `json:"arguments"` // raw JSON string
}

// ToolDefinition defines a tool that can be offered to the AI model.
type ToolDefinition struct {
	Type     string             `json:"type"`
	Function FunctionDefinition `json:"function"`
}

// FunctionDefinition describes a function's signature for tool use.
type FunctionDefinition struct {
	Name        string        `json:"name"`
	Description string        `json:"description"`
	Parameters  ToolParameter `json:"parameters"`
}

// ToolParameter describes the JSON schema parameters for a tool.
type ToolParameter struct {
	Type       string                 `json:"type"`
	Properties map[string]interface{} `json:"properties"`
	Required   []string               `json:"required,omitempty"`
}

// ---------------------------------------------------------------------------
// Request/Response types
// ---------------------------------------------------------------------------

// ChatOptions configures a chat completion request.
type ChatOptions struct {
	Model                 string   `json:"model"`
	Temperature           *float64 `json:"temperature,omitempty"`
	MaxTokens             *uint32  `json:"max_tokens,omitempty"`
	TopP                  *float64 `json:"top_p,omitempty"`
	StopSequences         []string `json:"stop_sequences,omitempty"`
	ReasoningEffort       string   `json:"reasoning_effort,omitempty"`
	ReasoningBudgetTokens *uint32  `json:"reasoning_budget_tokens,omitempty"`
}

// TokenUsage tracks token consumption for a single AI call.
type TokenUsage struct {
	PromptTokens     uint32 `json:"prompt_tokens"`
	CompletionTokens uint32 `json:"completion_tokens"`
	TotalTokens      uint32 `json:"total_tokens"`
}

// AiResponse represents the full response from an AI provider.
type AiResponse struct {
	Content      string     `json:"content"`
	ToolCalls    []ToolCall `json:"tool_calls"`
	Usage        TokenUsage `json:"usage"`
	Model        string     `json:"model"`
	FinishReason string     `json:"finish_reason"`
}

// ---------------------------------------------------------------------------
// Streaming types
// ---------------------------------------------------------------------------

// StreamEvent represents a single event in an SSE stream.
type StreamEvent struct {
	Type         string      // "content_delta", "tool_call_delta", "usage", "done"
	Content      string      // text content delta
	ToolCallID   string      // tool call ID (for tool_call_delta)
	ToolCallName string      // tool call function name (for tool_call_delta)
	Arguments    string      // tool call arguments delta (for tool_call_delta)
	Usage        *TokenUsage // token usage (for usage event)
}
