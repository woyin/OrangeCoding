package agents

import (
	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewSisyphus creates the primary general-purpose coding agent.
// RoleCoder, all tools, general system prompt.
func NewSisyphus(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	sid := core.NewSessionId()
	agentCtx := agent.NewAgentContext(sid, workDir)
	agentCtx.SetSystemPrompt("You are Sisyphus, a general-purpose coding agent. You write, debug, review, and refactor code. You are thorough, methodical, and never give up on a task.")

	executor := agent.NewToolExecutor(registry)
	toolDefs := buildToolDefs(registry)
	loop := agent.NewAgentLoop(core.NewAgentId(), provider, executor, agentCtx, agent.DefaultLoopConfig(), toolDefs)

	return NewBaseAgent(core.RoleCoder, loop)
}

func buildToolDefs(registry *tools.ToolRegistry) []ai.ToolDefinition {
	var defs []ai.ToolDefinition
	for _, t := range registry.List() {
		defs = append(defs, ai.ToolDefinition{
			Type: "function",
			Function: ai.FunctionDefinition{
				Name:        t.Name(),
				Description: t.Description(),
				Parameters:  ai.ToolParameter{Type: "object", Properties: make(map[string]interface{})},
			},
		})
	}
	return defs
}
