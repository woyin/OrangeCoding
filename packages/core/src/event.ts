/**
 * @module event
 *
 * Agent event types for observability and streaming.
 *
 * Events are emitted during agent execution to provide
 * real-time visibility into the agent's progress:
 * - ModelRequest/ModelResponse: AI API calls
 * - ToolCall/ToolResult: tool invocations
 * - Thinking: reasoning steps
 * - Error: failures and warnings
 * - Status: lifecycle state changes
 */
import type { AgentId, SessionId, TokenUsage } from "./types.js";
import type { ToolCall } from "./message.js";

// ---------------------------------------------------------------------------
// AgentEvent interface
// ---------------------------------------------------------------------------

/**
 * Structural shape that all concrete event types share. Consumers of the
 * EventBus treat events polymorphically through this interface; type
 * discrimination happens via `eventType`. Keeping it minimal avoids forcing
 * every subscriber to import the concrete classes.
 */

export interface AgentEvent {
  eventType: string;
  agentId: AgentId;
  sessionId: SessionId;
  timestamp: Date;
}

// ---------------------------------------------------------------------------
// BaseEvent
// ---------------------------------------------------------------------------

/**
 * Shared base for all 12 concrete event types. Carries the routing triple
 * (type, agent, session) plus a timestamp, and knows how to serialize
 * itself to the snake_case JSON shape used for persistence and transport.
 * Concrete subclasses override {@link toJSON} when they carry extra fields.
 */

export interface BaseEventJSON {
  type: string;
  agent_id: string;
  session_id: string;
  timestamp: string;
}

export class BaseEvent {
  constructor(
    public readonly type: string,
    public readonly agent: AgentId,
    public readonly session: SessionId,
    public readonly time: Date = new Date(),
  ) {}

  get eventType(): string { return this.type; }
  get agentId(): AgentId { return this.agent; }
  get sessionId(): SessionId { return this.session; }
  get timestamp(): Date { return this.time; }

  toJSON(): BaseEventJSON {
    return {
      type: this.type,
      agent_id: this.agent.toJSON(),
      session_id: this.session.toJSON(),
      timestamp: this.time.toISOString(),
    };
  }
}

// ---------------------------------------------------------------------------
// Concrete event types (11)
// ---------------------------------------------------------------------------
// Each event below models a single stage in the agent lifecycle. They are
// intentionally tiny immutable value objects exposing exactly the fields the
// UI / metrics / persistence layers need. Subscribers discriminate by the
// `eventType` string ("started", "tool_call_requested", ...) returned by the
// getter on BaseEvent.

// 1. StartedEvent
export class StartedEvent extends BaseEvent {
  constructor(agentId: AgentId, sessionId: SessionId) {
    super("started", agentId, sessionId);
  }
}

// 2. CompletedEvent
export class CompletedEvent extends BaseEvent {
  constructor(agentId: AgentId, sessionId: SessionId, public readonly summary: string) {
    super("completed", agentId, sessionId);
  }
}

// 3. MessageReceivedEvent
export class MessageReceivedEvent extends BaseEvent {
  constructor(agentId: AgentId, sessionId: SessionId, public readonly contentPreview: string) {
    super("message_received", agentId, sessionId);
  }
}

// 4. ToolCallRequestedEvent
export class ToolCallRequestedEvent extends BaseEvent {
  constructor(agentId: AgentId, sessionId: SessionId, public readonly toolCall: ToolCall) {
    super("tool_call_requested", agentId, sessionId);
  }
}

// 5. ToolCallCompletedEvent
export class ToolCallCompletedEvent extends BaseEvent {
  constructor(
    agentId: AgentId,
    sessionId: SessionId,
    public readonly toolName: string,
    public readonly success: boolean,
    public readonly durationMs: number,
  ) {
    super("tool_call_completed", agentId, sessionId);
  }

  override toJSON(): BaseEventJSON & {
    tool_name: string;
    success: boolean;
    duration_ms: number;
  } {
    return {
      ...super.toJSON(),
      tool_name: this.toolName,
      success: this.success,
      duration_ms: this.durationMs,
    };
  }
}

// 6. TokenUsageUpdatedEvent
export class TokenUsageUpdatedEvent extends BaseEvent {
  constructor(agentId: AgentId, sessionId: SessionId, public readonly usage: TokenUsage) {
    super("token_usage_updated", agentId, sessionId);
  }
}

// 7. StreamChunkEvent
export class StreamChunkEvent extends BaseEvent {
  constructor(agentId: AgentId, sessionId: SessionId, public readonly content: string) {
    super("stream_chunk", agentId, sessionId);
  }
}

// 8. ErrorEvent
export class ErrorEvent extends BaseEvent {
  constructor(agentId: AgentId, sessionId: SessionId, public readonly errorMessage: string) {
    super("error", agentId, sessionId);
  }
}

// 9. GoalPhaseChangedEvent
export class GoalPhaseChangedEvent extends BaseEvent {
  constructor(agentId: AgentId, sessionId: SessionId, public readonly phase: string, public readonly cycle: number) {
    super("goal_phase_changed", agentId, sessionId);
  }
}

// 10. GoalTaskCompletedEvent
export class GoalTaskCompletedEvent extends BaseEvent {
  constructor(
    agentId: AgentId,
    sessionId: SessionId,
    public readonly taskId: string,
    public readonly taskDescription: string,
    public readonly success: boolean,
  ) {
    super("goal_task_completed", agentId, sessionId);
  }
}

// 11. GoalCycleCompleteEvent
export class GoalCycleCompleteEvent extends BaseEvent {
  constructor(
    agentId: AgentId,
    sessionId: SessionId,
    public readonly cycle: number,
    public readonly tasksCompleted: number,
    public readonly tasksFailed: number,
    public readonly verificationPassed: boolean,
  ) {
    super("goal_cycle_complete", agentId, sessionId);
  }
}

// 12. GuardrailDecisionEvent
export class GuardrailDecisionEvent extends BaseEvent {
  constructor(
    agentId: AgentId,
    sessionId: SessionId,
    public readonly phase: string,
    public readonly decision: string,
    public readonly reason: string,
    public readonly guardrailName: string,
  ) {
    super("guardrail_decision", agentId, sessionId);
  }

  override toJSON(): BaseEventJSON & {
    phase: string;
    decision: string;
    reason: string;
    guardrail_name: string;
  } {
    return {
      ...super.toJSON(),
      phase: this.phase,
      decision: this.decision,
      reason: this.reason,
      guardrail_name: this.guardrailName,
    };
  }
}

// ---------------------------------------------------------------------------
// EventHandler
// ---------------------------------------------------------------------------

/**
 * Subscriber contract for the EventBus. Implementations may return a Promise
 * (async handlers run concurrently per publish) or void (synchronous).
 * Exceptions are isolated per handler so one failure does not break others.
 */

export interface EventHandler {
  handle(ev: AgentEvent): Promise<void> | void;
  readonly name: string;
}

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

/**
 * Minimal in-process pub/sub used to fan agent lifecycle events out to
 * subscribers. Subscriptions are keyed by {@link EventHandler.name} so a
 * later registration with the same name overwrites the earlier one.
 * publish() awaits all handlers concurrently; per-handler failures are
 * routed to the installed ErrorHandler (default: console.error) and never
 * reject the publish() promise.
 */

export type ErrorHandler = (handler: string, event: AgentEvent, error: Error) => void;

export class EventBus {
  private _subs = new Map<string, EventHandler>();
  private _errorHandler: ErrorHandler | null = null;

  subscribe(handler: EventHandler): void {
    this._subs.set(handler.name, handler);
  }

  unsubscribe(id: string): void {
    this._subs.delete(id);
  }

  /** Set a custom error handler for handler failures. */
  setErrorHandler(handler: ErrorHandler): void {
    this._errorHandler = handler;
  }

  /**
   * Fan `ev` out to every subscriber. Handlers run concurrently via
   * Promise.all; a throwing handler is caught and reported to the error
   * handler rather than rejecting this promise, so a single misbehaving
   * subscriber cannot break the event stream.
   */
  async publish(ev: AgentEvent): Promise<void> {
    const handlers = [...this._subs.values()];
    const promises = handlers.map(async (h) => {
      try {
        await h.handle(ev);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        if (this._errorHandler) {
          this._errorHandler(h.name, ev, error);
        } else {
          // Default: log to console for debugging
          console.error(`[EventBus] handler "${h.name}" failed for event "${ev.eventType}":`, error.message);
        }
      }
    });
    await Promise.all(promises);
  }

  get handlerCount(): number {
    return this._subs.size;
  }
}
