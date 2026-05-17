package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
)

// AgentLoopConfig configures the agent loop behavior.
type AgentLoopConfig struct {
	MaxIterations    uint32
	Timeout          time.Duration
	AutoApproveTools bool
	Language         OutputLanguage
	LongTask         LongTaskPolicy
	Reasoning        ReasoningPolicy
}

// DefaultLoopConfig returns a long-task-friendly config.
func DefaultLoopConfig() AgentLoopConfig {
	profile := DefaultHarnessProfile()
	return AgentLoopConfig{
		MaxIterations:    60,
		Timeout:          300 * time.Second,
		AutoApproveTools: true,
		Language:         profile.Language,
		LongTask:         profile.LongTask,
		Reasoning:        profile.Reasoning,
	}
}

// AgentLoopResult holds the summary of a completed agent loop run.
type AgentLoopResult struct {
	ToolCallsMade uint32
	TokensUsed    core.TokenUsage
	Duration      time.Duration
	StopReason    StopReason
	Progress      []ProgressSnapshot
}

// AgentLoop is the core event loop that drives agent behavior. It repeatedly:
//  1. Sends the conversation to the AI provider
//  2. Collects the response (including tool calls)
//  3. Executes any tool calls
//  4. Feeds tool results back into the conversation
//  5. Repeats until the AI returns a text-only response or max iterations is reached
type AgentLoop struct {
	id       core.AgentId
	provider ai.AiProvider
	executor *ToolExecutor
	context  *AgentContext
	config   AgentLoopConfig
	tools    []ai.ToolDefinition
}

// NewAgentLoop creates a new AgentLoop.
func NewAgentLoop(
	id core.AgentId,
	provider ai.AiProvider,
	executor *ToolExecutor,
	ctx *AgentContext,
	config AgentLoopConfig,
	toolDefs []ai.ToolDefinition,
) *AgentLoop {
	return &AgentLoop{
		id:       id,
		provider: provider,
		executor: executor,
		context:  ctx,
		config:   config,
		tools:    toolDefs,
	}
}

// Context returns the agent context.
func (l *AgentLoop) Context() *AgentContext {
	return l.context
}

// Executor returns the tool executor.
func (l *AgentLoop) Executor() *ToolExecutor {
	return l.executor
}

// Provider returns the AI provider.
func (l *AgentLoop) Provider() ai.AiProvider {
	return l.provider
}

// Config returns the loop configuration.
func (l *AgentLoop) Config() AgentLoopConfig {
	return l.config
}

// ToolDefs returns the tool definitions.
func (l *AgentLoop) ToolDefs() []ai.ToolDefinition {
	return l.tools
}

// AgentID returns the agent ID.
func (l *AgentLoop) AgentID() core.AgentId {
	return l.id
}

// Run executes the agent loop. It streams events to eventCh for each step
// (streaming content, tool calls, token usage updates, etc.).
func (l *AgentLoop) Run(ctx context.Context, chatOpts ai.ChatOptions, eventCh chan<- core.AgentEvent) (*AgentLoopResult, error) {
	start := time.Now()
	result := &AgentLoopResult{}

	sid := l.context.SessionID()
	profile := HarnessProfile{
		Language:  l.config.Language,
		LongTask:  l.config.LongTask,
		Reasoning: l.config.Reasoning,
	}.normalized()
	l.context.ApplyHarnessProfile(profile)
	chatOpts = profile.ApplyToChatOptions(chatOpts)

	// Apply overall timeout
	loopCtx, cancel := context.WithTimeout(ctx, l.config.Timeout)
	defer cancel()

	for iteration := uint32(0); iteration < l.config.MaxIterations; iteration++ {
		// Check context
		if loopCtx.Err() != nil {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonCanceled
			return result, fmt.Errorf("agent loop canceled: %w", loopCtx.Err())
		}

		if profile.LongTask.Enabled && profile.LongTask.CompactionMaxTokens > 0 {
			compactor := NewCompactor(profile.LongTask.CompactionMaxTokens)
			if err := compactor.Compact(l.context.Conversation()); err != nil {
				result.Duration = time.Since(start)
				return result, fmt.Errorf("compact conversation: %w", err)
			}
		}

		// Emit started event on first iteration
		if iteration == 0 && eventCh != nil {
			eventCh <- core.NewStartedEvent(l.id, sid)
		}

		// Convert conversation to AI messages
		aiMessages := conversationToAIMessages(l.context.Conversation())

		// Call provider with streaming
		streamCh, err := l.provider.ChatCompletionStream(loopCtx, aiMessages, l.tools, chatOpts)
		if err != nil {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonProviderError
			if eventCh != nil {
				eventCh <- core.NewErrorEvent(l.id, sid, err.Error())
			}
			return result, fmt.Errorf("provider stream failed: %w", err)
		}

		// Collect stream events
		var contentBuilder strings.Builder
		var toolCalls []aiToolCallAccumulator
		var usage *ai.TokenUsage

		for ev := range streamCh {
			switch ev.Type {
			case "content_delta":
				contentBuilder.WriteString(ev.Content)
				if eventCh != nil {
					eventCh <- core.NewStreamChunkEvent(l.id, sid, ev.Content)
				}

			case "tool_call_delta":
				tcID := ev.ToolCallID
				tcName := ev.ToolCallName
				tcArgs := ev.Arguments

				// Find or create accumulator for this tool call
				found := false
				for i := range toolCalls {
					if toolCalls[i].id == tcID {
						toolCalls[i].arguments += tcArgs
						if tcName != "" {
							toolCalls[i].name = tcName
						}
						found = true
						break
					}
				}
				if !found {
					toolCalls = append(toolCalls, aiToolCallAccumulator{
						id:        tcID,
						name:      tcName,
						arguments: tcArgs,
					})
				}

			case "usage":
				usage = ev.Usage

			case "done":
				// Stream complete
			}
		}

		content := contentBuilder.String()

		// Convert accumulated tool calls to core.ToolCall slice
		var coreToolCalls []core.ToolCall
		for _, tc := range toolCalls {
			coreToolCalls = append(coreToolCalls, core.ToolCall{
				ID:           tc.id,
				FunctionName: tc.name,
				Arguments:    json.RawMessage(tc.arguments),
			})
		}

		// Add assistant message to context
		if len(coreToolCalls) > 0 {
			l.context.Conversation().AddMessage(core.NewAssistantMessageWithToolCalls(content, coreToolCalls))
		} else {
			l.context.Conversation().AddMessage(core.NewAssistantMessage(content))
		}

		// Update token usage
		if usage != nil {
			tu := core.NewTokenUsage(uint64(usage.PromptTokens), uint64(usage.CompletionTokens))
			result.TokensUsed.Accumulate(tu)
			if eventCh != nil {
				eventCh <- core.NewTokenUsageUpdatedEvent(l.id, sid, tu)
			}
		}

		// If no tool calls, we're done
		if len(coreToolCalls) == 0 {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonCompleted
			l.recordProgress(result, iteration, "completed")
			if eventCh != nil {
				eventCh <- core.NewCompletedEvent(l.id, sid, content)
			}
			return result, nil
		}

		// Execute tool calls
		nextToolCount := result.ToolCallsMade + uint32(len(coreToolCalls))
		if profile.LongTask.Enabled && profile.LongTask.MaxToolCalls > 0 && nextToolCount > profile.LongTask.MaxToolCalls {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonToolBudget
			l.recordProgress(result, iteration, "tool budget exceeded")
			return result, fmt.Errorf("agent loop: tool budget exceeded (%d > %d)", nextToolCount, profile.LongTask.MaxToolCalls)
		}
		result.ToolCallsMade = nextToolCount

		// Emit tool call requested events
		for _, tc := range coreToolCalls {
			if eventCh != nil {
				eventCh <- core.NewToolCallRequestedEvent(l.id, sid, tc)
			}
		}

		execResults := l.executor.ExecuteBatch(loopCtx, coreToolCalls)

		// Add tool results to conversation and emit completion events
		for _, er := range execResults {
			l.context.Conversation().AddMessage(core.NewToolResultMessage(er.ToolCallID, er.Content, er.IsError))
			if eventCh != nil {
				// Find tool name for the event
				toolName := ""
				for _, tc := range coreToolCalls {
					if tc.ID == er.ToolCallID {
						toolName = tc.FunctionName
						break
					}
				}
				eventCh <- core.NewToolCallCompletedEvent(
					l.id, sid, toolName, !er.IsError, uint64(er.Duration.Milliseconds()),
				)
			}
		}
		if profile.ShouldRecordProgress(result.ToolCallsMade) {
			l.recordProgress(result, iteration, "tool batch completed")
		}
	}

	result.Duration = time.Since(start)
	result.StopReason = StopReasonMaxIterations
	return result, fmt.Errorf("agent loop: max iterations (%d) exceeded", l.config.MaxIterations)
}

func (l *AgentLoop) recordProgress(result *AgentLoopResult, iteration uint32, reason string) {
	result.Progress = append(result.Progress, ProgressSnapshot{
		Iteration:     iteration,
		ToolCallsMade: result.ToolCallsMade,
		TokensUsed:    result.TokensUsed,
		Reason:        reason,
		CreatedAt:     time.Now().UTC(),
	})
}

// conversationToAIMessages converts a core.Conversation to a slice of ai.ChatMessage.
func conversationToAIMessages(conv *core.Conversation) []ai.ChatMessage {
	msgs := conv.Messages()
	aiMsgs := make([]ai.ChatMessage, 0, len(msgs))

	for _, m := range msgs {
		switch m.Role {
		case core.RoleSystem:
			aiMsgs = append(aiMsgs, ai.SystemMsg(m.Content))
		case core.RoleUser:
			aiMsgs = append(aiMsgs, ai.UserMsg(m.Content))
		case core.RoleAssistant:
			if len(m.ToolCalls) > 0 {
				// Convert core.ToolCall to ai.ToolCall
				aiToolCalls := make([]ai.ToolCall, len(m.ToolCalls))
				for i, tc := range m.ToolCalls {
					aiToolCalls[i] = ai.ToolCall{
						ID:   tc.ID,
						Type: "function",
						Function: ai.FunctionCall{
							Name:      tc.FunctionName,
							Arguments: string(tc.Arguments),
						},
					}
				}
				aiMsgs = append(aiMsgs, ai.AssistantMsgWithTools(aiToolCalls))
				// Also set content if present
				aiMsgs[len(aiMsgs)-1].Content = m.Content
			} else {
				aiMsgs = append(aiMsgs, ai.AssistantMsg(m.Content))
			}
		case core.RoleTool:
			aiMsgs = append(aiMsgs, ai.ToolResultMsg(m.ToolCallID, m.Content))
		}
	}

	return aiMsgs
}

// aiToolCallAccumulator accumulates streaming tool call data.
type aiToolCallAccumulator struct {
	id        string
	name      string
	arguments string
}

// Type aliases used by fork.go and other files within this package.
type (
	aiToolDef  = ai.ToolDefinition
	aiChatOpts = ai.ChatOptions
)
