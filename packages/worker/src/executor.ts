/**
 * AgentExecutor manages the lifecycle of a single agent session.
 * It wraps an AgentLoop and tracks its execution status.
 *
 * Ported from modules/worker/executor.go.
 */

import type { AgentLoop } from "@orangecoding/agent";
import type { ChatOptions } from "@orangecoding/ai";
import {
  ErrorEvent,
  TaskUpdateEvent,
  type ServerEvent,
} from "@orangecoding/control-protocol";

// ---------------------------------------------------------------------------
// Executor status type
// ---------------------------------------------------------------------------

/** The possible statuses of an agent executor. */
export type ExecutorStatus = "pending" | "running" | "completed" | "failed";

// ---------------------------------------------------------------------------
// AgentExecutor
// ---------------------------------------------------------------------------

/**
 * AgentExecutor manages the lifecycle of a single agent session.
 * It wraps an AgentLoop and tracks its execution status.
 */
export class AgentExecutor {
  private _status: ExecutorStatus = "pending";
  private _abortController: AbortController | null = null;
  private _eventHandler: ((event: ServerEvent) => void) | null = null;
  private _runPromise: Promise<void> | null = null;

  /**
   * @param sessionID - The session identifier.
   * @param loop - The agent loop to execute. May be null for testing/stub scenarios.
   */
  constructor(
    private readonly sessionID: string,
    private readonly loop: AgentLoop | null,
  ) {}

  // -----------------------------------------------------------------------
  // Accessors
  // -----------------------------------------------------------------------

  /** Returns the session identifier. */
  get id(): string {
    return this.sessionID;
  }

  /** Returns the current execution status. */
  get status(): ExecutorStatus {
    return this._status;
  }

  // -----------------------------------------------------------------------
  // Configuration
  // -----------------------------------------------------------------------

  /** Sets the handler for emitting server events. */
  setEventHandler(handler: ((event: ServerEvent) => void) | null): void {
    this._eventHandler = handler;
  }

  /** Sets the AbortController for cancelling this executor. */
  setAbortController(controller: AbortController): void {
    this._abortController = controller;
  }

  // -----------------------------------------------------------------------
  // Lifecycle
  // -----------------------------------------------------------------------

  /**
   * Cancel stops the agent execution by aborting its controller.
   */
  cancel(): void {
    if (this._abortController != null) {
      this._abortController.abort();
    }
  }

  /**
   * Run executes the agent loop. It updates status to "running" during execution
   * and sets "completed" or "failed" when done.
   *
   * If the loop is null, it immediately completes successfully (useful for testing).
   * Returns the run promise so callers can await completion.
   */
  run(signal: AbortSignal): Promise<void> {
    this._runPromise = this._execute(signal);
    return this._runPromise;
  }

  // -----------------------------------------------------------------------
  // Internal
  // -----------------------------------------------------------------------

  private async _execute(signal: AbortSignal): Promise<void> {
    this._status = "running";

    // Check signal first
    if (signal.aborted) {
      this._status = "failed";
      this.emitEvent(
        new ErrorEvent(this.sessionID, `agent executor: context already canceled`),
      );
      throw new Error(
        `agent executor: context already canceled: ${signal.reason ?? "aborted"}`,
      );
    }

    // Emit task_update: running
    this.emitEvent(
      new TaskUpdateEvent(this.sessionID, "running", "agent started"),
    );

    // Handle null loop (for testing or stub scenarios)
    if (this.loop == null) {
      this._status = "completed";
      this.emitEvent(
        new TaskUpdateEvent(this.sessionID, "completed", "agent completed (no loop)"),
      );
      return;
    }

    // Run the agent loop
    try {
      await this.loop.run({} as ChatOptions, null);
    } catch (err) {
      this._status = "failed";
      const message = err instanceof Error ? err.message : String(err);
      this.emitEvent(new ErrorEvent(this.sessionID, message));
      throw new Error(`agent executor: run failed: ${message}`);
    }

    this._status = "completed";
    this.emitEvent(
      new TaskUpdateEvent(this.sessionID, "completed", "agent completed"),
    );
  }

  private emitEvent(event: ServerEvent): void {
    if (this._eventHandler != null) {
      this._eventHandler(event);
    }
  }
}
