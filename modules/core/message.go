package core

import (
	"encoding/json"
	"time"
)

// ---------------------------------------------------------------------------
// ToolCall
// ---------------------------------------------------------------------------

// ToolCall represents a single tool invocation requested by the assistant.
type ToolCall struct {
	ID           string          `json:"id"`
	FunctionName string          `json:"function_name"`
	Arguments    json.RawMessage `json:"arguments"`
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

// Message represents a single message in a conversation.
type Message struct {
	Role       Role       `json:"role"`
	Content    string     `json:"content,omitempty"`
	Name       string     `json:"name,omitempty"`
	ToolCalls  []ToolCall `json:"tool_calls,omitempty"`
	ToolCallID string     `json:"tool_call_id,omitempty"`
	CreatedAt  time.Time  `json:"created_at"`
}

// HasToolCalls returns true when the message contains at least one tool call.
func (m Message) HasToolCalls() bool {
	return len(m.ToolCalls) > 0
}

// NewSystemMessage creates a message with RoleSystem and the given content.
func NewSystemMessage(content string) Message {
	return Message{
		Role:      RoleSystem,
		Content:   content,
		CreatedAt: time.Now().UTC(),
	}
}

// NewUserMessage creates a message with RoleUser and the given content.
func NewUserMessage(content string) Message {
	return Message{
		Role:      RoleUser,
		Content:   content,
		CreatedAt: time.Now().UTC(),
	}
}

// NewAssistantMessage creates a message with RoleAssistant and the given content.
func NewAssistantMessage(content string) Message {
	return Message{
		Role:      RoleAssistant,
		Content:   content,
		CreatedAt: time.Now().UTC(),
	}
}

// NewAssistantMessageWithToolCalls creates a RoleAssistant message with tool calls.
func NewAssistantMessageWithToolCalls(content string, toolCalls []ToolCall) Message {
	return Message{
		Role:      RoleAssistant,
		Content:   content,
		ToolCalls: toolCalls,
		CreatedAt: time.Now().UTC(),
	}
}

// NewToolResultMessage creates a RoleTool message for a tool call result.
func NewToolResultMessage(toolCallID, content string, isError bool) Message {
	return Message{
		Role:       RoleTool,
		Content:    content,
		ToolCallID: toolCallID,
		CreatedAt:  time.Now().UTC(),
	}
}

// ---------------------------------------------------------------------------
// ToolResult
// ---------------------------------------------------------------------------

// ToolResult holds the result of executing a tool call.
type ToolResult struct {
	ToolCallID string `json:"tool_call_id"`
	Content    string `json:"content"`
	IsError    bool   `json:"is_error"`
}

// NewToolResultSuccess creates a ToolResult indicating successful execution.
func NewToolResultSuccess(toolCallID, content string) ToolResult {
	return ToolResult{
		ToolCallID: toolCallID,
		Content:    content,
		IsError:    false,
	}
}

// NewToolResultError creates a ToolResult indicating a failure.
func NewToolResultError(toolCallID, content string) ToolResult {
	return ToolResult{
		ToolCallID: toolCallID,
		Content:    content,
		IsError:    true,
	}
}

// ToMessage converts the ToolResult into a Message suitable for appending to a conversation.
func (tr ToolResult) ToMessage() Message {
	return NewToolResultMessage(tr.ToolCallID, tr.Content, tr.IsError)
}

// ---------------------------------------------------------------------------
// Conversation
// ---------------------------------------------------------------------------

// Conversation manages an ordered sequence of messages.
type Conversation struct {
	messages []Message
}

// NewConversation creates an empty conversation.
func NewConversation() *Conversation {
	return &Conversation{messages: []Message{}}
}

// NewConversationWithSystemPrompt creates a conversation with an initial system message.
func NewConversationWithSystemPrompt(prompt string) *Conversation {
	conv := NewConversation()
	conv.AddMessage(NewSystemMessage(prompt))
	return conv
}

// AddMessage appends a message to the conversation.
func (c *Conversation) AddMessage(msg Message) {
	c.messages = append(c.messages, msg)
}

// Messages returns a copy of the message slice.
func (c *Conversation) Messages() []Message {
	out := make([]Message, len(c.messages))
	copy(out, c.messages)
	return out
}

// Len returns the number of messages in the conversation.
func (c *Conversation) Len() int {
	return len(c.messages)
}

// IsEmpty returns true when the conversation contains no messages.
func (c *Conversation) IsEmpty() bool {
	return len(c.messages) == 0
}

// SystemPrompt returns the content of the first message if it has RoleSystem,
// or nil otherwise.
func (c *Conversation) SystemPrompt() *string {
	if len(c.messages) == 0 || c.messages[0].Role != RoleSystem {
		return nil
	}
	return &c.messages[0].Content
}

// LastMessage returns the last message in the conversation, or nil if empty.
func (c *Conversation) LastMessage() *Message {
	if len(c.messages) == 0 {
		return nil
	}
	return &c.messages[len(c.messages)-1]
}

// LastAssistantMessage searches backwards for the most recent RoleAssistant message.
// Returns nil if none is found.
func (c *Conversation) LastAssistantMessage() *Message {
	for i := len(c.messages) - 1; i >= 0; i-- {
		if c.messages[i].Role == RoleAssistant {
			return &c.messages[i]
		}
	}
	return nil
}

// PendingToolCalls returns the tool calls from the last assistant message.
// Returns nil if there is no last assistant message or it has no tool calls.
func (c *Conversation) PendingToolCalls() []ToolCall {
	last := c.LastAssistantMessage()
	if last == nil {
		return nil
	}
	return last.ToolCalls
}

// Clear removes all messages from the conversation and releases memory.
func (c *Conversation) Clear() {
	c.messages = nil
}

// TokenEstimate returns a rough token estimate.
// CJK characters are counted as ~2 tokens each; other characters as ~0.25 tokens each.
func (c *Conversation) TokenEstimate() int {
	cjkCount := 0
	nonCJKCount := 0
	for _, m := range c.messages {
		for _, r := range m.Content {
			if isCJK(r) {
				cjkCount++
			} else {
				nonCJKCount++
			}
		}
	}
	return cjkCount*2 + nonCJKCount/4
}

// isCJK returns true if the rune is in common CJK Unicode ranges.
func isCJK(r rune) bool {
	return (r >= 0x4E00 && r <= 0x9FFF) || // CJK Unified Ideographs
		(r >= 0x3400 && r <= 0x4DBF) || // CJK Extension A
		(r >= 0x3000 && r <= 0x303F) || // CJK Symbols and Punctuation
		(r >= 0xFF00 && r <= 0xFFEF) || // Fullwidth Forms
		(r >= 0x3040 && r <= 0x309F) || // Hiragana
		(r >= 0x30A0 && r <= 0x30FF) // Katakana
}
