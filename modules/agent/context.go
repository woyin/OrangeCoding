package agent

import (
	"github.com/woyin/OrangeCoding/modules/core"
)

// AgentContext holds the conversation state, environment, and metadata for an agent session.
type AgentContext struct {
	sessionID    core.SessionId
	conversation *core.Conversation
	workDir      string
	env          map[string]string
	metadata     map[string]string
}

// NewAgentContext creates a new AgentContext with the given session ID and working directory.
func NewAgentContext(sessionID core.SessionId, workDir string) *AgentContext {
	return &AgentContext{
		sessionID:    sessionID,
		conversation: core.NewConversation(),
		workDir:      workDir,
		env:          make(map[string]string),
		metadata:     make(map[string]string),
	}
}

// SetSystemPrompt sets the system prompt as the first message in the conversation.
// If the conversation already has a system message, it is replaced.
func (c *AgentContext) SetSystemPrompt(prompt string) {
	msgs := c.conversation.Messages()
	if len(msgs) > 0 && msgs[0].Role == core.RoleSystem {
		// Rebuild conversation with new system prompt
		newConv := core.NewConversation()
		newConv.AddMessage(core.NewSystemMessage(prompt))
		for _, m := range msgs[1:] {
			newConv.AddMessage(m)
		}
		c.conversation = newConv
	} else {
		// Prepend system message by rebuilding
		newConv := core.NewConversation()
		newConv.AddMessage(core.NewSystemMessage(prompt))
		for _, m := range msgs {
			newConv.AddMessage(m)
		}
		c.conversation = newConv
	}
}

// AddUserMessage appends a user message to the conversation.
func (c *AgentContext) AddUserMessage(content string) {
	c.conversation.AddMessage(core.NewUserMessage(content))
}

// AddAssistantMessage appends an assistant message to the conversation.
func (c *AgentContext) AddAssistantMessage(content string) {
	c.conversation.AddMessage(core.NewAssistantMessage(content))
}

// AddToolResult appends a tool result message to the conversation.
func (c *AgentContext) AddToolResult(result core.ToolResult) {
	c.conversation.AddMessage(result.ToMessage())
}

// Conversation returns the underlying conversation.
func (c *AgentContext) Conversation() *core.Conversation {
	return c.conversation
}

// SessionID returns the session ID.
func (c *AgentContext) SessionID() core.SessionId {
	return c.sessionID
}

// WorkDir returns the working directory.
func (c *AgentContext) WorkDir() string {
	return c.workDir
}
