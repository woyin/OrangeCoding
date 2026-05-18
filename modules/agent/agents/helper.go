package agents

import (
	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// newFilteredAgent is a shared constructor for agents with restricted tool sets.
// It creates a filtered registry, sets the system prompt, and returns a BaseAgent.
func newFilteredAgent(
	provider ai.AiProvider,
	registry *tools.ToolRegistry,
	workDir string,
	role core.AgentRole,
	allowedTools []string,
	systemPrompt string,
) *BaseAgent {
	sid := core.NewSessionId()
	agentCtx := agent.NewAgentContext(sid, workDir)
	agentCtx.SetSystemPrompt(systemPrompt)

	filteredRegistry := agent.FilteredRegistry(registry, allowedTools)
	executor := agent.NewToolExecutor(filteredRegistry)
	toolDefs := agent.BuildToolDefinitions(filteredRegistry)
	loop := agent.NewAgentLoop(core.NewAgentId(), provider, executor, agentCtx, agent.DefaultLoopConfig(), toolDefs)

	return NewBaseAgent(role, loop)
}
