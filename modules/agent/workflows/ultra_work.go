// Package workflows provides orchestrated multi-agent workflows.
package workflows

import (
	"context"
	"fmt"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// UltraWork runs an AgentLoop autonomously with a step budget.
type UltraWork struct {
	loop        *agent.AgentLoop
	stepBudget  uint32
}

// NewUltraWork creates a new UltraWork workflow.
func NewUltraWork(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string, stepBudget uint32) *UltraWork {
	sid := core.NewSessionId()
	agentCtx := agent.NewAgentContext(sid, workDir)
	agentCtx.SetSystemPrompt("You are an autonomous agent working within a step budget. Complete the task efficiently.")

	executor := agent.NewToolExecutor(registry)
	toolDefs := buildWorkflowToolDefs(registry)
	config := agent.AgentLoopConfig{
		MaxIterations:    stepBudget,
		Timeout:          600 * 1e9, // 10 minutes
		AutoApproveTools: true,
	}
	loop := agent.NewAgentLoop(core.NewAgentId(), provider, executor, agentCtx, config, toolDefs)

	return &UltraWork{
		loop:       loop,
		stepBudget: stepBudget,
	}
}

// Run executes the workflow with the given task.
func (uw *UltraWork) Run(ctx context.Context, task string) (*agent.AgentLoopResult, error) {
	uw.loop.Context().AddUserMessage(task)

	eventCh := make(chan core.AgentEvent, 100)
	go func() {
		for range eventCh {
			// drain events
		}
	}()

	result, err := uw.loop.Run(ctx, ai.ChatOptions{}, eventCh)
	close(eventCh)
	if err != nil {
		return nil, fmt.Errorf("ultra work failed: %w", err)
	}
	return result, nil
}

func buildWorkflowToolDefs(registry *tools.ToolRegistry) []ai.ToolDefinition {
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
