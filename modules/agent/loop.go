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
	CheckpointStore  CheckpointStore
	ContextBuilder   *HarnessContextBuilder
	MemoryManager    *HarnessMemoryManager
	Guardrails       *GuardrailPipeline
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
	id           core.AgentId
	provider     ai.AiProvider
	executor     *ToolExecutor
	context      *AgentContext
	config       AgentLoopConfig
	tools        []ai.ToolDefinition
	harnessRunID string
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

// HarnessRunID returns the checkpoint run ID used by the last Run call.
func (l *AgentLoop) HarnessRunID() string {
	return l.harnessRunID
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
	checkpointStore := l.config.CheckpointStore
	if checkpointStore == nil {
		checkpointStore = NewMemoryCheckpointStore()
	}
	contextBuilder := l.config.ContextBuilder
	if contextBuilder == nil {
		contextBuilder = NewHarnessContextBuilder(HarnessContextConfig{
			MaxTokens:      profile.LongTask.CompactionMaxTokens,
			RecentMessages: 8,
		})
	}
	guardrails := l.config.Guardrails
	if guardrails == nil {
		guardrails = NewGuardrailPipeline(NewDangerousToolGuardrail(), NewRepeatedToolGuardrail(3))
	}
	task := l.currentTask()
	l.harnessRunID = fmt.Sprintf("%s-%s", sid.String(), l.id.String())
	harness := NewHarnessEngine(HarnessEngineConfig{
		RunID:           l.harnessRunID,
		SessionID:       sid,
		CheckpointStore: checkpointStore,
	})
	if _, err := harness.Start(ctx, task); err != nil {
		return result, err
	}

	// Apply overall timeout
	loopCtx, cancel := context.WithTimeout(ctx, l.config.Timeout)
	defer cancel()

	for iteration := uint32(0); iteration < l.config.MaxIterations; iteration++ {
		if iteration > 0 {
			if _, err := harness.Transition(loopCtx, HarnessStateBuildContext, "next iteration"); err != nil {
				result.Duration = time.Since(start)
				return result, err
			}
		}
		// Check context
		if loopCtx.Err() != nil {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonCanceled
			_, _ = harness.Transition(context.Background(), HarnessStateStopped, "context canceled")
			return result, fmt.Errorf("agent loop canceled: %w", loopCtx.Err())
		}

		if profile.LongTask.Enabled && profile.LongTask.CompactionMaxTokens > 0 {
			compactor := NewCompactor(profile.LongTask.CompactionMaxTokens)
			if err := compactor.Compact(l.context.Conversation()); err != nil {
				result.Duration = time.Since(start)
				return result, fmt.Errorf("compact conversation: %w", err)
			}
		}

		blocks, err := contextBuilder.Build(loopCtx, HarnessContextInput{
			SystemPrompt:  l.context.Conversation().Messages()[0].Content,
			Task:          task,
			Conversation:  l.context.Conversation(),
			MemoryManager: l.config.MemoryManager,
		})
		if err != nil {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonProviderError
			_, _ = harness.Transition(context.Background(), HarnessStateFailed, err.Error())
			return result, fmt.Errorf("build harness context: %w", err)
		}
		_, _ = harness.Update(loopCtx, func(cp *HarnessCheckpoint) {
			cp.ContextBlocks = blocks
			cp.Iteration = iteration
			cp.ToolCallsMade = result.ToolCallsMade
			cp.TokenUsage = result.TokensUsed
		})

		// Emit started event on first iteration
		if iteration == 0 && eventCh != nil {
			eventCh <- core.NewStartedEvent(l.id, sid)
		}

		// Convert conversation to AI messages
		aiMessages := conversationToAIMessages(l.context.Conversation())
		aiMessages = prependHarnessContextMessages(aiMessages, blocks)

		// Call provider with streaming
		if _, err := harness.Transition(loopCtx, HarnessStateModelCall, "call provider"); err != nil {
			result.Duration = time.Since(start)
			return result, err
		}
		streamCh, err := l.provider.ChatCompletionStream(loopCtx, aiMessages, l.tools, chatOpts)
		if err != nil {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonProviderError
			_, _ = harness.Transition(context.Background(), HarnessStateFailed, err.Error())
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
		if l.config.MemoryManager != nil {
			_ = l.config.MemoryManager.LearnObservation(loopCtx, content)
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
			_, _ = harness.Transition(loopCtx, HarnessStateGuardrailCheck, "no tool calls")
			_, _ = harness.Transition(loopCtx, HarnessStateCompleted, "completed")
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
			_, _ = harness.Transition(context.Background(), HarnessStateGuardrailCheck, "tool budget exceeded")
			_, _ = harness.Transition(context.Background(), HarnessStateStopped, "tool budget exceeded")
			return result, fmt.Errorf("agent loop: tool budget exceeded (%d > %d)", nextToolCount, profile.LongTask.MaxToolCalls)
		}
		result.ToolCallsMade = nextToolCount

		if _, err := harness.Transition(loopCtx, HarnessStateGuardrailCheck, "check tool calls"); err != nil {
			result.Duration = time.Since(start)
			return result, err
		}
		for _, tc := range coreToolCalls {
			guardrailResult := guardrails.Check(loopCtx, GuardrailContext{
				Phase:          GuardrailPhasePreTool,
				ToolCall:       &tc,
				RecentToolKeys: l.recentToolKeys(),
			})
			if guardrailResult.Decision == GuardrailDeny {
				result.Duration = time.Since(start)
				result.StopReason = StopReasonGuardrail
				l.recordProgress(result, iteration, guardrailResult.Reason)
				_, _ = harness.Update(context.Background(), func(cp *HarnessCheckpoint) {
					cp.StopReason = StopReasonGuardrail
					cp.LastErrorMessage = guardrailResult.Reason
					cp.ToolCallsMade = result.ToolCallsMade
					cp.TokenUsage = result.TokensUsed
				})
				_, _ = harness.Transition(context.Background(), HarnessStateStopped, "guardrail denied tool call")
				return result, fmt.Errorf("agent loop: guardrail %s denied tool call %s: %s", guardrailResult.Name, tc.FunctionName, guardrailResult.Reason)
			}
		}

		// Emit tool call requested events
		for _, tc := range coreToolCalls {
			if eventCh != nil {
				eventCh <- core.NewToolCallRequestedEvent(l.id, sid, tc)
			}
		}

		if _, err := harness.Transition(loopCtx, HarnessStateToolDispatch, "execute tools"); err != nil {
			result.Duration = time.Since(start)
			return result, err
		}
		execResults := l.executor.ExecuteBatch(loopCtx, coreToolCalls)
		if _, err := harness.Transition(loopCtx, HarnessStateObserve, "observe tool results"); err != nil {
			result.Duration = time.Since(start)
			return result, err
		}

		// Add tool results to conversation and emit completion events
		for _, er := range execResults {
			l.context.Conversation().AddMessage(core.NewToolResultMessage(er.ToolCallID, er.Content, er.IsError))
			if l.config.MemoryManager != nil {
				_ = l.config.MemoryManager.LearnObservation(loopCtx, er.Content)
			}
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
		_, _ = harness.Transition(loopCtx, HarnessStateMemoryUpdate, "memory updated")
		_, _ = harness.Update(loopCtx, func(cp *HarnessCheckpoint) {
			cp.Iteration = iteration
			cp.ToolCallsMade = result.ToolCallsMade
			cp.TokenUsage = result.TokensUsed
			cp.RecentToolKeys = l.recentToolKeys()
		})
		_, _ = harness.Transition(loopCtx, HarnessStateCheckpoint, "checkpoint saved")
		_, _ = harness.Transition(loopCtx, HarnessStateDecideNext, "continue")
	}

	result.Duration = time.Since(start)
	result.StopReason = StopReasonMaxIterations
	_, _ = harness.Transition(context.Background(), HarnessStateStopped, "max iterations")
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

func (l *AgentLoop) currentTask() string {
	msgs := l.context.Conversation().Messages()
	for i := len(msgs) - 1; i >= 0; i-- {
		if msgs[i].Role == core.RoleUser {
			return msgs[i].Content
		}
	}
	return ""
}

func (l *AgentLoop) recentToolKeys() []string {
	var keys []string
	msgs := l.context.Conversation().Messages()
	for _, msg := range msgs {
		for _, call := range msg.ToolCalls {
			keys = append(keys, ToolCallKey(call))
		}
	}
	return keys
}

func prependHarnessContextMessages(messages []ai.ChatMessage, blocks []ContextBlock) []ai.ChatMessage {
	var prefix []ai.ChatMessage
	for _, block := range blocks {
		if block.Kind == ContextBlockConversation {
			continue
		}
		if block.Content == "" {
			continue
		}
		prefix = append(prefix, ai.SystemMsg(block.Content))
	}
	return append(prefix, messages...)
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
