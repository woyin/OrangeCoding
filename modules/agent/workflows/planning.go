package workflows

import (
	"context"
	"fmt"
	"strings"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// PlanningWorkflow uses a planner-style agent to decompose a task into steps.
type PlanningWorkflow struct {
	provider ai.AiProvider
	registry *tools.ToolRegistry
	workDir  string
}

// NewPlanningWorkflow creates a new PlanningWorkflow.
func NewPlanningWorkflow(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *PlanningWorkflow {
	return &PlanningWorkflow{
		provider: provider,
		registry: registry,
		workDir:  workDir,
	}
}

// PlanResult holds the output of the planning workflow.
type PlanResult struct {
	Steps    []string
	RawPlan  string
	Duration float64 // seconds
}

// Run executes the planning workflow and returns a list of steps.
func (pw *PlanningWorkflow) Run(ctx context.Context, task string) (*PlanResult, error) {
	sid := core.NewSessionId()
	agentCtx := agent.NewAgentContext(sid, pw.workDir)
	agentCtx.SetSystemPrompt("You are a planning agent. Decompose the given task into a numbered list of clear, actionable steps. Output only the steps, one per line.")

	allowedTools := []string{"read_file", "find", "grep", "glob"}
	filteredRegistry := agent.FilteredRegistry(pw.registry, allowedTools)
	executor := agent.NewToolExecutor(filteredRegistry)
	toolDefs := buildWorkflowToolDefs(filteredRegistry)

	loop := agent.NewAgentLoop(core.NewAgentId(), pw.provider, executor, agentCtx, agent.DefaultLoopConfig(), toolDefs)
	agentCtx.AddUserMessage(task)

	eventCh := make(chan core.AgentEvent, 100)
	go func() {
		for range eventCh {
		}
	}()

	result, err := loop.Run(ctx, ai.ChatOptions{}, eventCh)
	close(eventCh)
	if err != nil {
		return nil, fmt.Errorf("planning workflow failed: %w", err)
	}

	// Extract steps from the last assistant message
	conv := agentCtx.Conversation()
	lastAssistant := conv.LastAssistantMessage()
	if lastAssistant == nil {
		return nil, fmt.Errorf("planning workflow: no assistant response")
	}

	steps := parseSteps(lastAssistant.Content)
	return &PlanResult{
		Steps:    steps,
		RawPlan:  lastAssistant.Content,
		Duration: result.Duration.Seconds(),
	}, nil
}

// parseSteps extracts numbered steps from plan text.
func parseSteps(text string) []string {
	var steps []string
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		// Match lines like "1. step" or "- step"
		if len(line) > 2 && (line[0] == '-' || (line[0] >= '0' && line[0] <= '9' && strings.Contains(line[:5], "."))) {
			// Strip leading number/bullet and whitespace
			idx := strings.IndexFunc(line, func(r rune) bool {
				return r >= 'A' && r <= 'z'
			})
			if idx > 0 {
				steps = append(steps, line[idx:])
			} else {
				steps = append(steps, line)
			}
		}
	}
	return steps
}
