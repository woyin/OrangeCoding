package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
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
	GuardrailLogger  *GuardrailLogger
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
func (l *AgentLoop) Context() *AgentContext { return l.context }

// Executor returns the tool executor.
func (l *AgentLoop) Executor() *ToolExecutor { return l.executor }

// Provider returns the AI provider.
func (l *AgentLoop) Provider() ai.AiProvider { return l.provider }

// Config returns the loop configuration.
func (l *AgentLoop) Config() AgentLoopConfig { return l.config }

// ToolDefs returns the tool definitions.
func (l *AgentLoop) ToolDefs() []ai.ToolDefinition { return l.tools }

// HarnessRunID returns the checkpoint run ID used by the last Run call.
func (l *AgentLoop) HarnessRunID() string { return l.harnessRunID }

// AgentID returns the agent ID.
func (l *AgentLoop) AgentID() core.AgentId { return l.id }

// logGuardrail logs a guardrail result if a logger is configured.
func (l *AgentLoop) logGuardrail(name string, decision GuardrailDecision, reason string, phase GuardrailPhase) {
	if l.config.GuardrailLogger != nil {
		l.config.GuardrailLogger.Log(GuardrailLogEntry{
			Name:      name,
			Decision:  decision,
			Reason:    reason,
			Phase:     phase,
			Timestamp: time.Now().UTC(),
		})
	}
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
			if _, err := harness.Transition(context.Background(), HarnessStateStopped, "context canceled"); err != nil { logHarnessErr(err, "harness transition failed") }
			return result, fmt.Errorf("agent loop canceled: %w", loopCtx.Err())
		}

		// Build harness context blocks
		memoryManager := l.config.MemoryManager
		harnessInput := HarnessContextInput{
			SystemPrompt:  systemPromptFromContext(l.context),
			Task:          task,
			Conversation:  l.context.Conversation(),
			MemoryManager: memoryManager,
		}
		contextBlocks, err := contextBuilder.Build(loopCtx, harnessInput)
		if err != nil {
			result.Duration = time.Since(start)
			return result, fmt.Errorf("agent loop: context build failed: %w", err)
		}

		// Build messages for the AI provider
		aiMessages := conversationToAIMessages(l.context.Conversation())
		aiMessages = prependHarnessContextMessages(aiMessages, contextBlocks)

		// Phase 13: Pre-model guardrail
		preModelResult := guardrails.Check(loopCtx, GuardrailContext{
			Phase: GuardrailPhasePreModel,
		})
		l.logGuardrail(preModelResult.Name, preModelResult.Decision, preModelResult.Reason, GuardrailPhasePreModel)
		if preModelResult.Decision == GuardrailDeny {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonGuardrail
			_, _ = harness.Update(context.Background(), func(cp *HarnessCheckpoint) {
				cp.LastErrorMessage = preModelResult.Reason
			})
			if _, err := harness.Transition(context.Background(), HarnessStateStopped, "pre-model guardrail denied"); err != nil { logHarnessErr(err, "harness transition failed") }
			return result, fmt.Errorf("agent loop: pre-model guardrail %s denied: %s", preModelResult.Name, preModelResult.Reason)
		}

		// Transition to model call state
		if _, err := harness.Transition(loopCtx, HarnessStateModelCall, "call model"); err != nil {
			result.Duration = time.Since(start)
			return result, err
		}

		// Call the AI provider
		streamCh, err := l.provider.ChatCompletionStream(loopCtx, aiMessages, l.tools, chatOpts)
		if err != nil {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonProviderError
			_, _ = harness.Update(context.Background(), func(cp *HarnessCheckpoint) {
				cp.LastErrorMessage = err.Error()
			})
			if _, err := harness.Transition(context.Background(), HarnessStateFailed, "provider error"); err != nil { logHarnessErr(err, "harness transition failed") }
			return result, fmt.Errorf("agent loop: provider error: %w", err)
		}

		// Accumulate streaming response
		var content string
		var toolCalls []aiToolCallAccumulator
		var usage *ai.TokenUsage
		for event := range streamCh {
			switch event.Type {
			case "content_delta":
				content += event.Content
				if eventCh != nil {
					eventCh <- core.NewStreamChunkEvent(l.id, sid, event.Content)
				}
			case "tool_call_delta":
				found := false
				for i := range toolCalls {
					if toolCalls[i].id == event.ToolCallID {
						toolCalls[i].arguments += event.Arguments
						if event.ToolCallName != "" {
							toolCalls[i].name = event.ToolCallName
						}
						found = true
						break
					}
				}
				if !found {
					tc := aiToolCallAccumulator{id: event.ToolCallID, name: event.ToolCallName, arguments: event.Arguments}
					toolCalls = append(toolCalls, tc)
				}
			case "usage":
				usage = event.Usage
			}
		}

		// Convert tool calls
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
			// Phase 13: Final output guardrail
			finalResult := guardrails.Check(loopCtx, GuardrailContext{
				Phase:  GuardrailPhaseFinalOutput,
				Output: content,
			})
			l.logGuardrail(finalResult.Name, finalResult.Decision, finalResult.Reason, GuardrailPhaseFinalOutput)
			if finalResult.Decision == GuardrailDeny {
				result.Duration = time.Since(start)
				result.StopReason = StopReasonGuardrail
				_, _ = harness.Update(context.Background(), func(cp *HarnessCheckpoint) {
					cp.LastErrorMessage = finalResult.Reason
				})
				if _, err := harness.Transition(context.Background(), HarnessStateStopped, "final-output guardrail denied"); err != nil { logHarnessErr(err, "harness transition failed") }
				return result, fmt.Errorf("agent loop: final-output guardrail %s denied: %s", finalResult.Name, finalResult.Reason)
			}

			result.Duration = time.Since(start)
			result.StopReason = StopReasonCompleted
			l.recordProgress(result, iteration, "completed")
			if _, err := harness.Transition(loopCtx, HarnessStateGuardrailCheck, "no tool calls"); err != nil { logHarnessErr(err, "harness transition failed") }
			if _, err := harness.Transition(loopCtx, HarnessStateCompleted, "completed"); err != nil { logHarnessErr(err, "harness transition failed") }
			if eventCh != nil {
				eventCh <- core.NewCompletedEvent(l.id, sid, content)
			}
			return result, nil
		}

		// Track tool calls
		result.ToolCallsMade += uint32(len(coreToolCalls))

		// Check tool budget
		if profile.LongTask.Enabled && profile.LongTask.MaxToolCalls > 0 && result.ToolCallsMade >= profile.LongTask.MaxToolCalls {
			result.Duration = time.Since(start)
			result.StopReason = StopReasonToolBudget
			l.recordProgress(result, iteration, "tool budget exceeded")
			if _, err := harness.Transition(context.Background(), HarnessStateStopped, "tool budget exceeded"); err != nil { logHarnessErr(err, "harness transition failed") }
			return result, fmt.Errorf("agent loop: tool budget (%d) exceeded", profile.LongTask.MaxToolCalls)
		}

		// Transition to guardrail check state
		if _, err := harness.Transition(loopCtx, HarnessStateGuardrailCheck, "check guardrails"); err != nil {
			result.Duration = time.Since(start)
			return result, err
		}

		// Phase 13: Pre-tool guardrail (original + logging)
		for _, tc := range coreToolCalls {
			guardrailResult := guardrails.Check(loopCtx, GuardrailContext{
				Phase:          GuardrailPhasePreTool,
				ToolCall:       &tc,
				RecentToolKeys: l.recentToolKeys(),
			})
			l.logGuardrail(guardrailResult.Name, guardrailResult.Decision, guardrailResult.Reason, GuardrailPhasePreTool)
			if guardrailResult.Decision == GuardrailDeny {
				result.Duration = time.Since(start)
				result.StopReason = StopReasonGuardrail
				l.recordProgress(result, iteration, guardrailResult.Reason)
				_, _ = harness.Update(context.Background(), func(cp *HarnessCheckpoint) {
					cp.LastErrorMessage = guardrailResult.Reason
					cp.ToolCallsMade = result.ToolCallsMade
					cp.TokenUsage = result.TokensUsed
				})
				if _, err := harness.Transition(context.Background(), HarnessStateStopped, "guardrail denied tool call"); err != nil { logHarnessErr(err, "harness transition failed") }
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

		// Phase 13: Post-tool guardrail
		for _, er := range execResults {
			postToolResult := guardrails.Check(loopCtx, GuardrailContext{
				Phase:  GuardrailPhasePostTool,
				Output: er.Content,
			})
			l.logGuardrail(postToolResult.Name, postToolResult.Decision, postToolResult.Reason, GuardrailPhasePostTool)
			if postToolResult.Decision == GuardrailDeny {
				result.Duration = time.Since(start)
				result.StopReason = StopReasonGuardrail
				_, _ = harness.Update(context.Background(), func(cp *HarnessCheckpoint) {
					cp.LastErrorMessage = postToolResult.Reason
				})
				if _, err := harness.Transition(context.Background(), HarnessStateStopped, "post-tool guardrail denied"); err != nil { logHarnessErr(err, "harness transition failed") }
				return result, fmt.Errorf("agent loop: post-tool guardrail %s denied: %s", postToolResult.Name, postToolResult.Reason)
			}
		}

		if profile.ShouldRecordProgress(result.ToolCallsMade) {
			l.recordProgress(result, iteration, "tool batch completed")
		}
		if _, err := harness.Transition(loopCtx, HarnessStateMemoryUpdate, "memory updated"); err != nil { logHarnessErr(err, "harness transition failed") }
		_, _ = harness.Update(loopCtx, func(cp *HarnessCheckpoint) {
			cp.Iteration = iteration
			cp.ToolCallsMade = result.ToolCallsMade
			cp.TokenUsage = result.TokensUsed
			cp.RecentToolKeys = l.recentToolKeys()
		})
		if _, err := harness.Transition(loopCtx, HarnessStateCheckpoint, "checkpoint saved"); err != nil { logHarnessErr(err, "harness transition failed") }
		if _, err := harness.Transition(loopCtx, HarnessStateDecideNext, "continue"); err != nil { logHarnessErr(err, "harness transition failed") }
	}

	result.Duration = time.Since(start)
	result.StopReason = StopReasonMaxIterations
	if _, err := harness.Transition(context.Background(), HarnessStateStopped, "max iterations"); err != nil { logHarnessErr(err, "harness transition failed") }
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

// logHarnessErr logs a non-critical harness error without aborting.
func logHarnessErr(err error, msg string, args ...any) {
	if err != nil {
		slog.Warn(msg, append(args, "error", err)...)
	}
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

// systemPromptFromContext extracts the system prompt from the agent context.
func systemPromptFromContext(ctx *AgentContext) string {
	sp := ctx.Conversation().SystemPrompt()
	if sp != nil {
		return *sp
	}
	return ""
}

// Type aliases used by fork.go and other files within this package.
type (
	aiToolDef  = ai.ToolDefinition
	aiChatOpts = ai.ChatOptions
)
