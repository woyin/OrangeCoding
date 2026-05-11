package workflows

import (
	"context"
	"fmt"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// ExecutionWorkflow uses an executor-style agent to execute plan steps.
type ExecutionWorkflow struct {
	provider ai.AiProvider
	registry *tools.ToolRegistry
	workDir  string
}

// NewExecutionWorkflow creates a new ExecutionWorkflow.
func NewExecutionWorkflow(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *ExecutionWorkflow {
	return &ExecutionWorkflow{
		provider: provider,
		registry: registry,
		workDir:  workDir,
	}
}

// ExecutionResult holds the output of the execution workflow.
type ExecutionResult struct {
	StepsCompleted int
	StepsFailed    int
	Duration       float64 // seconds
}

// Run executes the given plan steps sequentially.
func (ew *ExecutionWorkflow) Run(ctx context.Context, steps []string) (*ExecutionResult, error) {
	result := &ExecutionResult{}

	for i, step := range steps {
		sid := core.NewSessionId()
		agentCtx := agent.NewAgentContext(sid, ew.workDir)
		agentCtx.SetSystemPrompt("You are an executor agent. Execute the given step precisely and report the result.")

		allowedTools := []string{"bash", "read_file", "write_file", "edit_file"}
		filteredRegistry := agent.FilteredRegistry(ew.registry, allowedTools)
		executor := agent.NewToolExecutor(filteredRegistry)
		toolDefs := buildWorkflowToolDefs(filteredRegistry)

		loop := agent.NewAgentLoop(core.NewAgentId(), ew.provider, executor, agentCtx, agent.DefaultLoopConfig(), toolDefs)
		agentCtx.AddUserMessage(fmt.Sprintf("Execute step %d: %s", i+1, step))

		eventCh := make(chan core.AgentEvent, 100)
		go func() {
			for range eventCh {
			}
		}()

		loopResult, err := loop.Run(ctx, ai.ChatOptions{}, eventCh)
		close(eventCh)

		result.Duration += loopResult.Duration.Seconds()

		if err != nil {
			result.StepsFailed++
		} else {
			result.StepsCompleted++
		}

		// Check context cancellation
		if ctx.Err() != nil {
			return result, fmt.Errorf("execution workflow canceled at step %d: %w", i+1, ctx.Err())
		}
	}

	return result, nil
}
