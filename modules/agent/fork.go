package agent

import (
	"context"
	"fmt"

	"github.com/woyin/OrangeCoding/modules/core"
)

// ForkAgent creates a child agent that runs with a restricted tool subset
// on a cloned context.
type ForkAgent struct {
	parent       *AgentLoop
	allowedTools []string
}

// NewForkAgent creates a ForkAgent that inherits the parent's provider and
// configuration but restricts available tools to the given subset.
func NewForkAgent(parent *AgentLoop, allowedTools []string) *ForkAgent {
	return &ForkAgent{
		parent:       parent,
		allowedTools: allowedTools,
	}
}

// Run clones the parent context, creates a filtered tool registry, and runs a
// new AgentLoop for the given task.
func (f *ForkAgent) Run(ctx context.Context, task string) (*AgentLoopResult, error) {
	// Clone the parent's context by copying the conversation
	parentCtx := f.parent.Context()
	clonedConv := core.NewConversation()
	for _, m := range parentCtx.Conversation().Messages() {
		clonedConv.AddMessage(m)
	}

	forkCtx := &AgentContext{
		sessionID:    parentCtx.SessionID(),
		conversation: clonedConv,
		workDir:      parentCtx.WorkDir(),
		env:          make(map[string]string),
		metadata:     make(map[string]string),
	}

	// Set task as user message
	forkCtx.AddUserMessage(task)

	// Create filtered tool registry
	filteredRegistry := FilteredRegistry(f.parent.Executor().Registry(), f.allowedTools)
	forkExecutor := NewToolExecutor(filteredRegistry)

	// Filter tool definitions
	var filteredDefs []aiToolDef
	for _, td := range f.parent.ToolDefs() {
		for _, name := range f.allowedTools {
			if td.Function.Name == name {
				filteredDefs = append(filteredDefs, td)
				break
			}
		}
	}

	forkID := core.NewAgentId()
	loop := NewAgentLoop(forkID, f.parent.Provider(), forkExecutor, forkCtx, f.parent.Config(), filteredDefs)

	eventCh := make(chan core.AgentEvent, 100)
	// Drain events to prevent the channel from blocking the agent loop.
	go func() {
		for range eventCh {
		}
	}()

	result, err := loop.Run(ctx, aiChatOpts{}, eventCh)
	close(eventCh)
	if err != nil {
		return nil, fmt.Errorf("fork agent failed: %w", err)
	}
	return result, nil
}
