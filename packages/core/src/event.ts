import type { AgentId, SessionId, TokenUsage } from "./types.js";
import type { ToolCall } from "./message.js";

// ---------------------------------------------------------------------------
// AgentEvent interface
// ---------------------------------------------------------------------------

export interface AgentEvent {
  eventType: string;
  agentId: AgentId;
  sessionId: SessionId;
  timestamp: Date;
}

// ---------------------------------------------------------------------------
// BaseEvent
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// EventHandler
// ---------------------------------------------------------------------------

export interface EventHandler {
  handle(ev: AgentEvent): Promise<void> | void;
  readonly name: string;
}

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

export class EventBus {
  private _subs = new Map<string, EventHandler>();

  subscribe(handler: EventHandler): void {
    this._subs.set(handler.name, handler);
  }

  unsubscribe(id: string): void {
    this._subs.delete(id);
  }

  async publish(ev: AgentEvent): Promise<void> {
    const handlers = [...this._subs.values()];
    const results = await Promise.allSettled(
      handlers.map(async (h) => {
        try {
          await h.handle(ev);
        } catch {
          // Handler threw; swallow to protect the bus.
        }
      }),
    );
    // Silently ignore rejected handlers (mirrors Go's panic recovery).
    void results;
  }

  get handlerCount(): number {
    return this._subs.size;
  }
}
