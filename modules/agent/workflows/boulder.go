package workflows

import (
	"context"
	"fmt"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// BoulderRecovery detects a stuck agent, resets the context, and retries.
type BoulderRecovery struct {
	provider   ai.AiProvider
	registry   *tools.ToolRegistry
	workDir    string
	maxRetries int
}

// NewBoulderRecovery creates a new BoulderRecovery workflow.
func NewBoulderRecovery(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string, maxRetries int) *BoulderRecovery {
	return &BoulderRecovery{
		provider:   provider,
		registry:   registry,
		workDir:    workDir,
		maxRetries: maxRetries,
	}
}

// BoulderResult holds the output of a boulder recovery attempt.
type BoulderResult struct {
	Attempts   int
	Success    bool
	Duration   float64 // seconds
	FinalError string
}

// Run attempts the task, detecting stuck states and retrying with a fresh context.
func (br *BoulderRecovery) BoulderRecovery(ctx context.Context, task string) (*BoulderResult, error) {
	result := &BoulderResult{}

	for attempt := 0; attempt < br.maxRetries; attempt++ {
		result.Attempts = attempt + 1

		// Fresh context for each attempt
		sid := core.NewSessionId()
		agentCtx := agent.NewAgentContext(sid, br.workDir)
		agentCtx.SetSystemPrompt("You are a resilient agent. Complete the task without getting stuck in loops.")
		agentCtx.AddUserMessage(task)

		executor := agent.NewToolExecutor(br.registry)
		toolDefs := buildWorkflowToolDefs(br.registry)

		loop := agent.NewAgentLoop(core.NewAgentId(), br.provider, executor, agentCtx, agent.DefaultLoopConfig(), toolDefs)

		eventCh := make(chan core.AgentEvent, 100)
		go func() {
			for range eventCh {
			}
		}()

		loopResult, err := loop.Run(ctx, ai.ChatOptions{}, eventCh)
		close(eventCh)

		result.Duration += loopResult.Duration.Seconds()

		if err == nil {
			result.Success = true
			return result, nil
		}

		result.FinalError = err.Error()

		// Check context cancellation
		if ctx.Err() != nil {
			return result, fmt.Errorf("boulder recovery canceled on attempt %d: %w", attempt+1, ctx.Err())
		}
	}

	return result, fmt.Errorf("boulder recovery: all %d attempts exhausted, last error: %s", br.maxRetries, result.FinalError)
}
