/**
 * AgentLoop — the core event loop that drives agent behavior.
 *
 * Each iteration of {@link AgentLoop.run} performs the same fixed pipeline:
 *
 *   1. Build context      — assemble system prompt + memory + conversation.
 *   2. Pre-model guardrail— policy check before spending tokens.
 *   3. Model call          — stream the provider response, accumulating text
 *                            and tool-call deltas.
 *   4. Final-output / tool decision — branch on whether the model requested tools.
 *   5. Pre-tool guardrail  — policy check per requested tool call.
 *   6. Tool dispatch       — execute the batch via the ToolExecutor.
 *   7. Observe + memory    — ingest tool results, feed memory, emit events.
 *   8. Post-tool guardrail — policy check on tool outputs.
 *   9. Compact + checkpoint— shrink the conversation if over budget, persist.
 *
 * The loop terminates on completion, a guardrail deny, the tool/iteration
 * budget, cancellation, or a provider error. A {@link HarnessEngine} records
 * state-machine transitions for observability and resumption.
 *
 * Originally ported from modules/agent/loop.go; since refactored for clarity
 * and to remove repeated allocation/error-handling boilerplate.
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
// 常量：近期工具调用键的窗口上限。recentToolKeys() 只保留最近这么多键，
// 把 RepeatedToolGuardrail 的每次检查从 O(累计键数) 压到 O(窗口) —— 即常数时间。
// ---------------------------------------------------------------------------*/
const RECENT_TOOL_KEY_WINDOW = 200;

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

/** 返回适合长任务的默认循环配置（迭代上限、超时、推理强度等）。 */
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
// 代理主循环：驱动“构建上下文 → 预模型 guardrail → 模型流式调用 → 工具调度
// → 后置 guardrail → 记忆/压缩/checkpoint”的迭代管线，直到完成、guardrail 拒绝、
// 达到工具/迭代预算、取消或 provider 错误为止。
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
  /**
   * recentToolKeys() 上次扫描到的消息下标。只追加“新增”消息的工具调用键，
   * 避免重复计数。-1 表示尚未扫描过。
   */
  private _lastScannedMsgIdx = -1;

  /**
   * Creates a new AgentLoop instance.
   *
   * @param id - Unique agent identifier
   * @param provider - AI provider for model calls
   * @param executor - Tool executor for dispatching tool calls
   * @param ctx - Agent context (session, conversation, working directory)
   * @param config - Loop configuration (limits, guardrails, etc.)
   * @param toolDefs - Tool definitions to send to the model
   */
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
    this._lastScannedMsgIdx = -1;
  }

  /** The agent's conversation context and working directory. */
  get context(): AgentContext { return this._context; }
  /** The tool executor responsible for running tool calls. */
  get executor(): ToolExecutor { return this._executor; }
  /** The AI provider used for model completions. */
  get provider(): AiProvider { return this._provider; }
  /** Current loop configuration (limits, guardrails, etc.). */
  get config(): AgentLoopConfig { return this._config; }
  /** Tool definitions available to the model. */
  get toolDefs(): ToolDefinition[] { return this._tools; }
  /** The current harness run ID for observability tracking. */
  get harnessRunID(): string { return this._harnessRunID; }
  /** The unique identifier for this agent. */
  get agentID(): AgentId { return this._id; }

  /**
   * 运行代理主循环至完成，通过 `eventCb` 以事件流的形式回报进度。
   *
   * `chatOpts` 可部分指定；首次模型调用前会应用 harness profile 默认值
   * （推理强度、输出语言等）。返回 {@link AgentLoopResult}，包含 token 用量、
   * 工具调用数、耗时与终止原因。
   *
   * ---
   * Runs the agent loop to completion, streaming progress via `eventCb`.
   *
   * `chatOpts` may be partially specified; harness profile defaults are
   * applied (reasoning effort, language, etc.) before the first model call.
   *
   * Returns an {@link AgentLoopResult} describing token usage, tool calls,
   * elapsed time, and the terminal stop reason.
   */
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

    // ---- Main iteration loop -------------------------------------------------
    // Each pass executes one full pipeline (build → guardrail → model → tools).
    // `start` is captured once so every early-return can compute durationMs in O(1).
    try {
      for (let iteration = 0; iteration < this._config.maxIterations; iteration++) {
        // Re-enter the BuildContext state for iterations after the first.
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

        // Convert conversation + harness context blocks into provider wire format.
        // `let` is retained because prependHarnessContextMessages may return the
        // same array by reference when there is no prefix (zero-copy fast path).
        let aiMessages = conversationToAIMessages(this._context.conversation);
        aiMessages = prependHarnessContextMessages(aiMessages, contextBlocks);
        // Token estimate for the pre-model guardrail budget check.
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

        // Accumulators for the streamed model response.
        let content = "";
        const toolCalls: AiToolCallAccumulator[] = [];
        // id -> accumulator, for O(1) lookup while streaming tool-call deltas.
        const toolCallIndex = new Map<string, AiToolCallAccumulator>();
        let usage: { promptTokens: number; completionTokens: number } | null = null;

        // Stream the provider response to completion. Tool-call deltas are
        // accumulated in arrival order; an index map (toolCallIndex) gives O(1)
        // appends for the common single-call case and avoids the previous O(n)
        // linear scan on every delta — a measurable win for models that emit
        // tool-call arguments token-by-token across many deltas.
        for await (const event of streamIter) {
          switch (event.type) {
            case "content_delta":
              // Append text delta and forward to the listener (e.g. TUI).
              content += event.content;
              eventCb?.(new StreamChunkEvent(this._id, sid, event.content));
              break;
            case "tool_call_delta": {
              const existing = toolCallIndex.get(event.tool_call_id);
              if (existing !== undefined) {
                existing.arguments += event.arguments;
                if (event.tool_call_name) existing.name = event.tool_call_name;
              } else {
                const acc: AiToolCallAccumulator = {
                  id: event.tool_call_id,
                  name: event.tool_call_name,
                  arguments: event.arguments,
                };
                toolCallIndex.set(event.tool_call_id, acc);
                toolCalls.push(acc);
              }
              break;
            }
            case "usage":
              // Capture token usage once the provider reports it.
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

        // Feed assistant output into long-term memory (best-effort; errors swallowed).
        await this._learn(content);

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

        // Execute the requested tool calls. toolNameById is built once so the
        // completion-event loop below is O(results) instead of O(results*calls).
        const execResults = await this._executor.executeBatch(loopController.signal, coreToolCalls);
        const toolNameById = new Map<string, string>();
        for (const tc of coreToolCalls) toolNameById.set(tc.id, tc.function_name);

        try {
          await harness.transition(loopController.signal, HS.Observe, "observe tool results");
        } catch (err) {
          result.durationMs = Date.now() - start;
          return result;
        }

        // Add tool results to conversation and emit completion events
        for (const er of execResults) {
          this._context.conversation.addMessage(newToolResultMessage(er.toolCallID, er.content, er.isError));
          await this._learn(er.content);
          if (eventCb) {
            // toolNameById gives O(1) lookup instead of an inner linear scan
            // per tool result (was O(n*m) across the result batch).
            eventCb(new ToolCallCompletedEvent(
              this._id, sid,
              toolNameById.get(er.toolCallID) ?? "",
              !er.isError, er.durationMs,
            ));
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

  /**
   * Logs a guardrail decision to the configured guardrail logger.
   * No-op if no logger is configured.
   */
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

  /**
   * Records a guardrail decision by logging it and emitting an event.
   * Combines logGuardrail with event emission for the event callback.
   */
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

  /**
   * Records a progress snapshot at the current iteration.
   * Captures iteration number, tool calls made, token usage, and reason.
   */
  private recordProgress(result: AgentLoopResult, iteration: number, reason: string): void {
    result.progress.push({
      iteration,
      toolCallsMade: result.toolCallsMade,
      tokensUsed: result.tokensUsed,
      reason,
      createdAt: new Date(),
    });
  }

  /**
   * Returns the most recent user message as the current task description.
   * Scans backwards through the conversation to find the last user input.
   */
  private currentTask(): string {
    const msgs = this._context.conversation.messagesUnsafe();
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i]!.role === Role.User) return msgs[i]!.content;
    }
    return "";
  }

  /**
   * 返回近期工具调用键（name + 参数指纹），供 pre-tool guardrail 做重复检测。
   *
   * 性能优化：原实现把每次新扫描到的键整体追加到 `_cachedToolKeys`，
   * 导致该数组随迭代次数无限增长，RepeatedToolGuardrail.check 每次都要做
   * O(缓存长度) 的线性扫描——整个循环呈 O(迭代²×调用数) 复杂度。
   * 现改为“滑动窗口”：只保留最近 {@link RECENT_TOOL_KEY_WINDOW} 个键，
   * 把每次 guardrail 检查压到常数时间，且仍保留重复计数语义
   * （RepeatedToolGuardrail 依赖同一键多次出现来触发拒绝）。
   */
  private recentToolKeys(): string[] {
    const msgs = this._context.conversation.messagesUnsafe();
    // 只扫描自上次以来的新增消息（_lastScannedMsgIdx），避免对同一消息重复计数，
    // 否则会让 RepeatedToolGuardrail 的重复统计虚高、误触发拒绝。
    const startIdx = this._lastScannedMsgIdx >= 0 ? this._lastScannedMsgIdx : 0;

    for (let i = startIdx; i < msgs.length; i++) {
      const msg = msgs[i]!;
      if (msg.toolCalls) {
        for (const call of msg.toolCalls) {
          this._cachedToolKeys.push(toolCallKey(call));
        }
      }
    }
    this._lastScannedMsgIdx = msgs.length;

    // 修剪到窗口上限：保证 _cachedToolKeys 长度有界，guardrail 检查恒为 O(窗口)。
    if (this._cachedToolKeys.length > RECENT_TOOL_KEY_WINDOW) {
      this._cachedToolKeys = this._cachedToolKeys.slice(-RECENT_TOOL_KEY_WINDOW);
    }
    return this._cachedToolKeys;
  }

  /**
   * Feeds `content` into whichever long-term memory backend is configured.
   * Prefers the tiered memory manager when present, otherwise falls back to the
   * flat memory manager. All errors are swallowed (`.catch(() => {})`) because
   * memory learning is best-effort and must never abort the agent loop.
   */
  private async _learn(content: string): Promise<void> {
    if (this._config.tieredMemory) {
      await this._config.tieredMemory.learn(content).catch(() => {});
    } else if (this._config.memoryManager) {
      await this._config.memoryManager.learnObservation(undefined, content).catch(() => {});
    }
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

/**
 * Prepends non-conversation harness context blocks (as system messages) in
 * front of the conversation messages. "conversation"-kind blocks and empty
 * blocks are skipped — they are already represented in `messages`.
 *
 * Performance note: builds the result in a single pass by pre-sizing the
 * output array and pushing directly, rather than allocating an intermediate
 * `prefix` array and then spreading twice (`[...prefix, ...messages]`), which
 * previously copied every element twice.
 */
function prependHarnessContextMessages(messages: ChatMessage[], blocks: { kind: string; content: string }[]): ChatMessage[] {
  // Pre-count usable blocks so we can size the prefix slice exactly, avoiding
  // a reallocation when we splice it ahead of the conversation messages.
  let prefixCount = 0;
  for (const block of blocks) {
    if (block.kind !== "conversation" && block.content) prefixCount++;
  }
  if (prefixCount === 0) return messages; // no prefix → return messages as-is, zero-copy

  const result: ChatMessage[] = new Array(prefixCount + messages.length);
  let i = 0;
  for (const block of blocks) {
    if (block.kind === "conversation") continue;
    if (!block.content) continue;
    result[i++] = systemMsg(block.content);
  }
  // Copy the conversation messages into the tail of the same array.
  for (let j = 0; j < messages.length; j++) {
    result[i++] = messages[j]!;
  }
  return result;
}

/**
 * Converts the internal Conversation message list into the wire format
 * expected by AI providers ({@link ChatMessage}).
 *
 * Maps: system→system, user→user, assistant(+toolCalls)→assistant w/ tool_calls,
 * tool→tool result. Assistant tool-call arguments are JSON-stringified when
 * they are not already strings, since the wire format requires string args.
 *
 * Performance note: pre-sizes the output array to the exact message count so
 * push() never triggers a backing-array reallocation (V8 otherwise grows
 * dynamically with amortized copies). Uses a plain for-loop index, the
 * fastest iteration shape in V8.
 */
function conversationToAIMessages(conv: { messages(): { role: string; content: string; toolCalls?: { id: string; function_name: string; arguments: unknown; toolCallID?: string }[]; toolCallID?: string }[]; messagesUnsafe(): readonly { role: string; content: string; toolCalls?: { id: string; function_name: string; arguments: unknown; toolCallID?: string }[]; toolCallID?: string }[] }): ChatMessage[] {
  // Read the backing array directly (messagesUnsafe) to skip the defensive
  // copy made by messages() — we never mutate here, so the copy was pure waste
  // and allocated a full message array every agent-loop iteration.
  const msgs = conv.messagesUnsafe();
  const aiMsgs: ChatMessage[] = new Array(msgs.length);

  for (let i = 0; i < msgs.length; i++) {
    const m = msgs[i]!;
    switch (m.role) {
      case "system":
        aiMsgs[i] = systemMsg(m.content);
        break;
      case "user":
        aiMsgs[i] = userMsg(m.content);
        break;
      case "assistant":
        if (m.toolCalls && m.toolCalls.length > 0) {
          // Map internal tool calls to the provider wire shape in one pass.
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
          aiMsgs[i] = msg;
        } else {
          aiMsgs[i] = assistantMsg(m.content);
        }
        break;
      case "tool":
        aiMsgs[i] = toolResultMsg(m.toolCallID ?? "", m.content);
        break;
      default:
        // Unknown roles are preserved as user-style messages so the provider
        // still receives the content (defensive; should not happen in practice).
        aiMsgs[i] = userMsg(m.content);
        break;
    }
  }

  return aiMsgs;
}

export type { GuardrailPhase, GuardrailDecision };
