/**
 * AgentLoop is the core event loop that drives agent behavior.
 * Ported from modules/agent/loop.go.
 */

import type { AgentId, SessionId, TokenUsage, ToolCall as CoreToolCall } from "@orangecoding/core";
import {
  TokenUsage as TokenUsageClass,
  Role,
  newAssistantMessage,
  newAssistantMessageWithToolCalls,
  newToolResultMessage,
} from "@orangecoding/core";
import type { AgentEvent } from "@orangecoding/core";
import {
  CompletedEvent,
  StreamChunkEvent,
  TokenUsageUpdatedEvent,
  ToolCallRequestedEvent,
  ToolCallCompletedEvent,
  GuardrailDecisionEvent,
} from "@orangecoding/core";
import type { AiProvider, ChatOptions, ToolDefinition, StreamEvent, ChatMessage, ToolCall as AiToolCall } from "@orangecoding/ai";
import { systemMsg, userMsg, assistantMsg, toolResultMsg, assistantMsgWithTools } from "@orangecoding/ai";
import type { ToolExecutor } from "./executor.js";
import type { AgentContext } from "./context.js";
import {
  HarnessProfile,
  defaultHarnessProfile,
} from "./harness-profile.js";
import type {
  StopReason,
  OutputLanguage,
  ReasoningEffort,
  ProgressSnapshot,
} from "./harness-profile.js";
import { HarnessContextBuilder, type HarnessContextInput } from "./harness-context.js";
import type { HarnessMemoryManager } from "./harness-memory.js";
import type { TieredMemoryManager } from "./tiered-memory.js";
import { GuardrailPipeline, defaultGuardrailPipeline } from "./harness-guardrail.js";
import type { GuardrailLogger, GuardrailPhase, GuardrailDecision } from "./harness-guardrail.js";
import { toolCallKey } from "./harness-guardrail.js";
import { Compactor } from "./compaction.js";
import type { CheckpointStore } from "./harness-state.js";
import { MemoryCheckpointStore, HarnessState as HS } from "./harness-state.js";
import { HarnessEngine } from "./harness-engine.js";
import type { Skill, SkillContext } from "./skills.js";

// ---------------------------------------------------------------------------
// Re-export from harness-profile for convenience
// ---------------------------------------------------------------------------

export type { StopReason, OutputLanguage, ReasoningEffort, ProgressSnapshot };

// ---------------------------------------------------------------------------
// AgentLoopConfig
// ---------------------------------------------------------------------------

export interface AgentLoopConfig {
  maxIterations: number;
  timeoutMs: number;
  autoApproveTools: boolean;
  language: OutputLanguage;
  longTask: {
    enabled: boolean;
    maxToolCalls: number;
    progressEveryNCalls: number;
    compactionMaxTokens: number;
  };
  reasoning: {
    effort: ReasoningEffort;
    budgetTokens: number;
  };
  checkpointStore?: CheckpointStore;
  contextBuilder?: HarnessContextBuilder;
  memoryManager?: HarnessMemoryManager;
  tieredMemory?: TieredMemoryManager;
  guardrails?: GuardrailPipeline;
  guardrailLogger?: GuardrailLogger;
  /** Active skill context — configures system prompt and tool filtering */
  skill?: SkillContext;
}

/** DefaultLoopConfig returns a long-task-friendly config. */
export function defaultLoopConfig(): AgentLoopConfig {
  const profile = defaultHarnessProfile();
  return {
    maxIterations: 60,
    timeoutMs: 300_000,
    autoApproveTools: true,
    language: profile.language,
    longTask: { ...profile.longTask },
    reasoning: { ...profile.reasoning },
  };
}

// ---------------------------------------------------------------------------
// AgentLoopResult
// ---------------------------------------------------------------------------

export interface AgentLoopResult {
  toolCallsMade: number;
  tokensUsed: TokenUsage;
  durationMs: number;
  stopReason: StopReason;
  progress: ProgressSnapshot[];
}

// ---------------------------------------------------------------------------
// AgentLoop
// ---------------------------------------------------------------------------

export class AgentLoop {
  private _id: AgentId;
  private _provider: AiProvider;
  private _executor: ToolExecutor;
  private _context: AgentContext;
  private _config: AgentLoopConfig;
  private _tools: ToolDefinition[];
  private _harnessRunID: string;
  private _cachedToolKeys: string[];

  constructor(
    id: AgentId,
    provider: AiProvider,
    executor: ToolExecutor,
    ctx: AgentContext,
    config: AgentLoopConfig,
    toolDefs: ToolDefinition[],
  ) {
    this._id = id;
    this._provider = provider;
    this._executor = executor;
    this._context = ctx;
    this._config = config;
    this._tools = toolDefs;
    this._harnessRunID = "";
    this._cachedToolKeys = [];
  }

  get context(): AgentContext { return this._context; }
  get executor(): ToolExecutor { return this._executor; }
  get provider(): AiProvider { return this._provider; }
  get config(): AgentLoopConfig { return this._config; }
  get toolDefs(): ToolDefinition[] { return this._tools; }
  get harnessRunID(): string { return this._harnessRunID; }
  get agentID(): AgentId { return this._id; }

  /** Run executes the agent loop, streaming events via the callback. */
  async run(
    chatOpts: Partial<ChatOptions>,
    eventCb: ((event: AgentEvent) => void) | null,
  ): Promise<AgentLoopResult> {
    const start = Date.now();
    const result: AgentLoopResult = {
      toolCallsMade: 0,
      tokensUsed: TokenUsageClass.create(0, 0),
      durationMs: 0,
      stopReason: "completed" as StopReason,
      progress: [],
    };

    const sid = this._context.sessionID;

    // Apply skill context if configured
    if (this._config.skill) {
      this._context.setSystemPrompt(this._config.skill.systemPrompt);
      this._tools = this._tools.filter((t) =>
        this._config.skill!.allowedTools.includes(t.function.name),
      );
    }

    const profile = new HarnessProfile({
      language: this._config.language,
      longTask: this._config.longTask,
      reasoning: this._config.reasoning,
    }).normalized();
    this._context.applyHarnessProfile(profile);
    chatOpts = profile.applyToChatOptions(chatOpts as ChatOptions);

    let checkpointStore = this._config.checkpointStore;
    if (!checkpointStore) checkpointStore = new MemoryCheckpointStore();

    let contextBuilder = this._config.contextBuilder;
    if (!contextBuilder) {
      contextBuilder = new HarnessContextBuilder({
        maxTokens: this._config.longTask.compactionMaxTokens,
        recentMessages: 8,
      });
    }

    let guardrails = this._config.guardrails;
    if (!guardrails) {
      guardrails = defaultGuardrailPipeline({
        maxTokens: this._config.reasoning.budgetTokens,
      });
    }

    const task = this.currentTask();
    this._harnessRunID = `${sid.toString()}-${this._id.toString()}`;
    const harness = new HarnessEngine({
      runID: this._harnessRunID,
      sessionID: sid,
      checkpointStore,
    });

    try {
      await harness.start(undefined, task);
    } catch (err) {
      result.durationMs = Date.now() - start;
      return result;
    }

    // Apply overall timeout
    const loopController = new AbortController();
    const timeoutId = setTimeout(() => loopController.abort(), this._config.timeoutMs);

    try {
      for (let iteration = 0; iteration < this._config.maxIterations; iteration++) {
        if (iteration > 0) {
          try {
            await harness.transition(loopController.signal, HS.BuildContext, "next iteration");
          } catch (err) {
            result.durationMs = Date.now() - start;
            return result;
          }
        }

        // Check context
        if (loopController.signal.aborted) {
          result.durationMs = Date.now() - start;
          result.stopReason = "canceled";
          await harness.transition(undefined, HS.Stopped, "context canceled").catch(() => {});
          return result;
        }

        // Build harness context blocks
        const harnessInput: HarnessContextInput = {
          systemPrompt: systemPromptFromContext(this._context),
          task,
          conversation: this._context.conversation,
          memoryManager: this._config.memoryManager,
          tieredMemory: this._config.tieredMemory,
        };
        let contextBlocks;
        try {
          contextBlocks = await contextBuilder!.build(loopController.signal, harnessInput);
        } catch (err) {
          result.durationMs = Date.now() - start;
          return result;
        }

        // Build messages for the AI provider
        let aiMessages = conversationToAIMessages(this._context.conversation);
        aiMessages = prependHarnessContextMessages(aiMessages, contextBlocks);
        const preModelTokenEstimate = this._context.conversation.tokenEstimate();

        // Pre-model guardrail
        const preModelResult = await guardrails.check(loopController.signal, {
          phase: "pre_model",
          output: "",
          recentToolKeys: [],
          tokenEstimate: preModelTokenEstimate,
          maxTokens: 0,
        });
        this.recordGuardrail(preModelResult.name, preModelResult.decision, preModelResult.reason, "pre_model", eventCb, sid);
        if (preModelResult.decision === "deny") {
          result.durationMs = Date.now() - start;
          result.stopReason = "guardrail";
          await harness.update(undefined, (cp) => { cp.lastErrorMessage = preModelResult.reason; }).catch(() => {});
          await harness.transition(undefined, HS.Stopped, "pre-model guardrail denied").catch(() => {});
          return result;
        }

        // Transition to model call state
        try {
          await harness.transition(loopController.signal, HS.ModelCall, "call model");
        } catch (err) {
          result.durationMs = Date.now() - start;
          return result;
        }

        // Call the AI provider
        let streamIter: AsyncIterable<StreamEvent>;
        try {
          streamIter = await this._provider.chatCompletionStream(aiMessages, this._tools, chatOpts as ChatOptions);
        } catch (err) {
          result.durationMs = Date.now() - start;
          result.stopReason = "provider_error";
          await harness.update(undefined, (cp) => { cp.lastErrorMessage = (err as Error).message; }).catch(() => {});
          await harness.transition(undefined, HS.Failed, "provider error").catch(() => {});
          return result;
        }

        // Accumulate streaming response
        let content = "";
        const toolCalls: AiToolCallAccumulator[] = [];
        let usage: { promptTokens: number; completionTokens: number } | null = null;

        for await (const event of streamIter) {
          switch (event.type) {
            case "content_delta":
              content += event.content;
              if (eventCb) {
                eventCb(new StreamChunkEvent(this._id, sid, event.content));
              }
              break;
            case "tool_call_delta": {
              let found = false;
              for (const tc of toolCalls) {
                if (tc.id === event.tool_call_id) {
                  tc.arguments += event.arguments;
                  if (event.tool_call_name) tc.name = event.tool_call_name;
                  found = true;
                  break;
                }
              }
              if (!found) {
                toolCalls.push({
                  id: event.tool_call_id,
                  name: event.tool_call_name,
                  arguments: event.arguments,
                });
              }
              break;
            }
            case "usage":
              if (event.usage) {
                usage = {
                  promptTokens: event.usage.prompt_tokens,
                  completionTokens: event.usage.completion_tokens,
                };
              }
              break;
          }
        }

        // Convert tool calls
        const coreToolCalls: CoreToolCall[] = toolCalls.map((tc) => ({
          id: tc.id,
          function_name: tc.name,
          arguments: tc.arguments,
        }));

        // Add assistant message to context
        if (coreToolCalls.length > 0) {
          this._context.conversation.addMessage(newAssistantMessageWithToolCalls(content, coreToolCalls));
        } else {
          this._context.conversation.addMessage(newAssistantMessage(content));
        }

        if (this._config.tieredMemory) {
          await this._config.tieredMemory.learn(content).catch(() => {});
        } else if (this._config.memoryManager) {
          await this._config.memoryManager.learnObservation(undefined, content).catch(() => {});
        }

        // Update token usage
        if (usage) {
          const tu = TokenUsageClass.create(usage.promptTokens, usage.completionTokens);
          result.tokensUsed.accumulate(tu);
          if (eventCb) {
            eventCb(new TokenUsageUpdatedEvent(this._id, sid, tu));
          }
        }

        // If no tool calls, we're done
        if (coreToolCalls.length === 0) {
          // Final output guardrail
          const finalResult = await guardrails.check(loopController.signal, {
            phase: "final_output",
            output: content,
            recentToolKeys: [],
            tokenEstimate: this._context.conversation.tokenEstimate(),
            maxTokens: 0,
          });
          this.recordGuardrail(finalResult.name, finalResult.decision, finalResult.reason, "final_output", eventCb, sid);
          if (finalResult.decision === "deny") {
            result.durationMs = Date.now() - start;
            result.stopReason = "guardrail";
            await harness.update(undefined, (cp) => { cp.lastErrorMessage = finalResult.reason; }).catch(() => {});
            await harness.transition(undefined, HS.Stopped, "final-output guardrail denied").catch(() => {});
            return result;
          }

          result.durationMs = Date.now() - start;
          result.stopReason = "completed";
          this.recordProgress(result, iteration, "completed");
          await harness.transition(loopController.signal, HS.GuardrailCheck, "no tool calls").catch(() => {});
          await harness.transition(loopController.signal, HS.Completed, "completed").catch(() => {});
          if (eventCb) {
            eventCb(new CompletedEvent(this._id, sid, content));
          }
          return result;
        }

        // Track tool calls
        result.toolCallsMade += coreToolCalls.length;

        // Check tool budget
        if (this._config.longTask.enabled && this._config.longTask.maxToolCalls > 0 && result.toolCallsMade >= this._config.longTask.maxToolCalls) {
          result.durationMs = Date.now() - start;
          result.stopReason = "tool_budget";
          this.recordProgress(result, iteration, "tool budget exceeded");
          await harness.transition(undefined, HS.Stopped, "tool budget exceeded").catch(() => {});
          return result;
        }

        // Transition to guardrail check state
        try {
          await harness.transition(loopController.signal, HS.GuardrailCheck, "check guardrails");
        } catch (err) {
          result.durationMs = Date.now() - start;
          return result;
        }

        // Pre-tool guardrail
        const recentKeys = this.recentToolKeys();
        for (const tc of coreToolCalls) {
          const guardrailResult = await guardrails.check(loopController.signal, {
            phase: "pre_tool",
            toolCall: tc,
            output: "",
            recentToolKeys: recentKeys,
            tokenEstimate: 0,
            maxTokens: 0,
          });
          this.recordGuardrail(guardrailResult.name, guardrailResult.decision, guardrailResult.reason, "pre_tool", eventCb, sid);
          if (guardrailResult.decision === "deny") {
            result.durationMs = Date.now() - start;
            result.stopReason = "guardrail";
            this.recordProgress(result, iteration, guardrailResult.reason);
            await harness.update(undefined, (cp) => {
              cp.lastErrorMessage = guardrailResult.reason;
              cp.toolCallsMade = result.toolCallsMade;
              cp.tokenUsage = result.tokensUsed;
            }).catch(() => {});
            await harness.transition(undefined, HS.Stopped, "guardrail denied tool call").catch(() => {});
            return result;
          }
        }

        // Emit tool call requested events
        for (const tc of coreToolCalls) {
          if (eventCb) {
            eventCb(new ToolCallRequestedEvent(this._id, sid, tc));
          }
        }

        try {
          await harness.transition(loopController.signal, HS.ToolDispatch, "execute tools");
        } catch (err) {
          result.durationMs = Date.now() - start;
          return result;
        }

        const execResults = await this._executor.executeBatch(loopController.signal, coreToolCalls);

        try {
          await harness.transition(loopController.signal, HS.Observe, "observe tool results");
        } catch (err) {
          result.durationMs = Date.now() - start;
          return result;
        }

        // Add tool results to conversation and emit completion events
        for (const er of execResults) {
          this._context.conversation.addMessage(newToolResultMessage(er.toolCallID, er.content, er.isError));
          if (this._config.tieredMemory) {
            await this._config.tieredMemory.learn(er.content).catch(() => {});
          } else if (this._config.memoryManager) {
            await this._config.memoryManager.learnObservation(undefined, er.content).catch(() => {});
          }
          if (eventCb) {
            let toolName = "";
            for (const tc of coreToolCalls) {
              if (tc.id === er.toolCallID) {
                toolName = tc.function_name;
                break;
              }
            }
            eventCb(new ToolCallCompletedEvent(this._id, sid, toolName, !er.isError, er.durationMs));
          }
        }

        // Post-tool guardrail
        for (const er of execResults) {
          const postToolResult = await guardrails.check(loopController.signal, {
            phase: "post_tool",
            output: er.content,
            recentToolKeys: [],
            tokenEstimate: 0,
            maxTokens: 0,
          });
          this.recordGuardrail(postToolResult.name, postToolResult.decision, postToolResult.reason, "post_tool", eventCb, sid);
          if (postToolResult.decision === "deny") {
            result.durationMs = Date.now() - start;
            result.stopReason = "guardrail";
            await harness.update(undefined, (cp) => { cp.lastErrorMessage = postToolResult.reason; }).catch(() => {});
            await harness.transition(undefined, HS.Stopped, "post-tool guardrail denied").catch(() => {});
            return result;
          }
        }

        if (profile.shouldRecordProgress(result.toolCallsMade)) {
          this.recordProgress(result, iteration, "tool batch completed");
        }

        await harness.transition(loopController.signal, HS.MemoryUpdate, "memory updated").catch(() => {});

        // Advance working memory turn counter
        if (this._config.tieredMemory) {
          this._config.tieredMemory.advanceTurn();
        }

        // Compact conversation if it exceeds the token budget
        if (this._config.longTask.enabled && this._config.longTask.compactionMaxTokens > 0) {
          const compactor = new Compactor(this._config.longTask.compactionMaxTokens);
          compactor.compact(this._context.conversation);
        }
        await harness.update(loopController.signal, (cp) => {
          cp.iteration = iteration;
          cp.toolCallsMade = result.toolCallsMade;
          cp.tokenUsage = result.tokensUsed;
          cp.recentToolKeys = this.recentToolKeys();
        }).catch(() => {});
        await harness.transition(loopController.signal, HS.Checkpoint, "checkpoint saved").catch(() => {});
        await harness.transition(loopController.signal, HS.DecideNext, "continue").catch(() => {});
      }

      result.durationMs = Date.now() - start;
      result.stopReason = "max_iterations";
      await harness.transition(undefined, HS.Stopped, "max iterations").catch(() => {});
      return result;
    } finally {
      clearTimeout(timeoutId);
    }
  }

  private logGuardrail(name: string, decision: string, reason: string, phase: string): void {
    if (this._config.guardrailLogger) {
      this._config.guardrailLogger.log({
        name,
        decision: decision as GuardrailDecision,
        reason,
        phase: phase as GuardrailPhase,
        timestamp: new Date(),
      });
    }
  }

  private recordGuardrail(
    name: string,
    decision: string,
    reason: string,
    phase: string,
    eventCb: ((event: AgentEvent) => void) | null,
    sid: SessionId,
  ): void {
    this.logGuardrail(name, decision, reason, phase);
    if (eventCb) {
      eventCb(new GuardrailDecisionEvent(this._id, sid, phase, decision, reason, name));
    }
  }

  private recordProgress(result: AgentLoopResult, iteration: number, reason: string): void {
    result.progress.push({
      iteration,
      toolCallsMade: result.toolCallsMade,
      tokensUsed: result.tokensUsed,
      reason,
      createdAt: new Date(),
    });
  }

  private currentTask(): string {
    const msgs = this._context.conversation.messagesUnsafe();
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i]!.role === Role.User) return msgs[i]!.content;
    }
    return "";
  }

  private recentToolKeys(): string[] {
    const msgs = this._context.conversation.messagesUnsafe();
    const startIdx = this._cachedToolKeys.length > 0
      ? Math.max(0, msgs.length - 10)
      : 0;

    const keys: string[] = [];
    for (let i = startIdx; i < msgs.length; i++) {
      const msg = msgs[i]!;
      if (msg.toolCalls) {
        for (const call of msg.toolCalls) {
          keys.push(toolCallKey(call));
        }
      }
    }

    if (keys.length > 0) {
      this._cachedToolKeys = [...this._cachedToolKeys, ...keys];
    }
    return this._cachedToolKeys;
  }
}

// ---------------------------------------------------------------------------
// AiToolCallAccumulator
// ---------------------------------------------------------------------------

interface AiToolCallAccumulator {
  id: string;
  name: string;
  arguments: string;
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

function systemPromptFromContext(ctx: AgentContext): string {
  return ctx.conversation.systemPrompt() ?? "";
}

function prependHarnessContextMessages(messages: ChatMessage[], blocks: { kind: string; content: string }[]): ChatMessage[] {
  const prefix: ChatMessage[] = [];
  for (const block of blocks) {
    if (block.kind === "conversation") continue;
    if (!block.content) continue;
    prefix.push(systemMsg(block.content));
  }
  return [...prefix, ...messages];
}

function conversationToAIMessages(conv: { messages(): { role: string; content: string; toolCalls?: { id: string; function_name: string; arguments: unknown; toolCallID?: string }[]; toolCallID?: string }[] }): ChatMessage[] {
  const msgs = conv.messages();
  const aiMsgs: ChatMessage[] = [];

  for (const m of msgs) {
    switch (m.role) {
      case "system":
        aiMsgs.push(systemMsg(m.content));
        break;
      case "user":
        aiMsgs.push(userMsg(m.content));
        break;
      case "assistant":
        if (m.toolCalls && m.toolCalls.length > 0) {
          const aiToolCalls = m.toolCalls.map((tc) => ({
            id: tc.id,
            type: "function",
            function: {
              name: tc.function_name,
              arguments: typeof tc.arguments === "string" ? tc.arguments : JSON.stringify(tc.arguments),
            },
          }));
          const msg = assistantMsgWithTools(aiToolCalls);
          msg.content = m.content;
          aiMsgs.push(msg);
        } else {
          aiMsgs.push(assistantMsg(m.content));
        }
        break;
      case "tool":
        aiMsgs.push(toolResultMsg(m.toolCallID ?? "", m.content));
        break;
    }
  }

  return aiMsgs;
}

export type { GuardrailPhase, GuardrailDecision };
