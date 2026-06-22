/**
 * @module control-messages
 *
 * Control protocol message types for the HTTP control server.
 *
 * Defines the request/response types for the REST API:
 * - Session management (create, list, get, delete)
 * - Task submission and status queries
 * - Agent health checks
 * - Configuration updates
 *
 * All messages are serialized as JSON for HTTP transport.
 */
// ---------------------------------------------------------------------------
// ClientCommand -- commands sent from the web UI / control client to the
// control server. Each command carries a discriminant string via commandType().
// ---------------------------------------------------------------------------

/** ClientCommand is the interface for commands from web UI to server. */
export interface ClientCommand {
  commandType(): string;
}

/** SendTaskCommand instructs the server to send a task to an agent session. */
export class SendTaskCommand implements ClientCommand {
  constructor(
    public readonly sessionId: string,
    public readonly task: string,
  ) {}

  /** commandType returns the wire discriminant "send_task". */
  commandType(): string {
    return "send_task";
  }
}

/** ApproveCommand responds to an approval request. */
export class ApproveCommand implements ClientCommand {
  constructor(
    public readonly requestId: string,
    public readonly approved: boolean,
  ) {}

  /** commandType returns the wire discriminant "approve". */
  commandType(): string {
    return "approve";
  }
}

/** CancelCommand requests cancellation of a session. */
export class CancelCommand implements ClientCommand {
  constructor(
    public readonly sessionId: string,
  ) {}

  /** commandType returns the wire discriminant "cancel". */
  commandType(): string {
    return "cancel";
  }
}

// ---------------------------------------------------------------------------
// ServerEvent -- events streamed from the control server to connected web UI
// clients over the WebSocket channel. Each event carries a discriminant via
// eventType() so the client can switch on a single field.
// ---------------------------------------------------------------------------

/** ServerEvent is the interface for events from server to web UI. */
export interface ServerEvent {
  eventType(): string;
}

/** TaskUpdateEvent reports a status change for a task. */
export class TaskUpdateEvent implements ServerEvent {
  constructor(
    public readonly sessionId: string,
    public readonly status: string,
    public readonly message: string,
  ) {}

  /** eventType returns the wire discriminant "task_update". */
  eventType(): string {
    return "task_update";
  }
}

/** ToolCallEvent reports a tool invocation and its result. */
export class ToolCallEvent implements ServerEvent {
  constructor(
    public readonly sessionId: string,
    public readonly toolName: string,
    public readonly input: string,
    public readonly output: string,
    public readonly isError: boolean,
  ) {}

  /** eventType returns the wire discriminant "tool_call". */
  eventType(): string {
    return "tool_call";
  }
}

/** ApprovalRequestEvent asks the user to approve a tool call. */
export class ApprovalRequestEvent implements ServerEvent {
  constructor(
    public readonly requestId: string,
    public readonly toolName: string,
    public readonly input: string,
    public readonly message: string,
  ) {}

  /** eventType returns the wire discriminant "approval_request". */
  eventType(): string {
    return "approval_request";
  }
}

/** ErrorEvent reports an error for a session. */
export class ErrorEvent implements ServerEvent {
  constructor(
    public readonly sessionId: string,
    public readonly error: string,
  ) {}

  /** eventType returns the wire discriminant "error". */
  eventType(): string {
    return "error";
  }
}

/** AgentStreamEvent reports a streaming text chunk from the agent. */
export class AgentStreamEvent implements ServerEvent {
  constructor(
    public readonly sessionId: string,
    public readonly content: string,
  ) {}

  eventType(): string {
    return "agent_stream";
  }
}

/** AgentCompletedEvent reports the agent finished with a final answer. */
export class AgentCompletedEvent implements ServerEvent {
  constructor(
    public readonly sessionId: string,
    public readonly content: string,
    public readonly toolCallsMade: number,
    public readonly tokensUsed: number,
    public readonly durationMs: number,
    public readonly stopReason: string,
  ) {}

  eventType(): string {
    return "agent_completed";
  }
}

/** GuardrailEvent reports a guardrail decision. */
export class GuardrailEvent implements ServerEvent {
  constructor(
    public readonly sessionId: string,
    public readonly phase: string,
    public readonly decision: string,
    public readonly reason: string,
    public readonly guardrailName: string,
  ) {}

  eventType(): string {
    return "guardrail";
  }
}
