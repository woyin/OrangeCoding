// ---------------------------------------------------------------------------
// ClientCommand -- commands from web UI to server
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

  commandType(): string {
    return "approve";
  }
}

/** CancelCommand requests cancellation of a session. */
export class CancelCommand implements ClientCommand {
  constructor(
    public readonly sessionId: string,
  ) {}

  commandType(): string {
    return "cancel";
  }
}

// ---------------------------------------------------------------------------
// ServerEvent -- events from server to web UI
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

  eventType(): string {
    return "error";
  }
}
