/**
 * AgentExecutor manages the lifecycle of a single agent session.
 * It wraps an AgentLoop and tracks its execution status.
 *
 * The executor sits in a long-lived task-processing loop. Each submitted
 * task is added as a user message to the agent context and triggers one
 * full agent-loop run.  The executor returns to "idle" between tasks and
 * only reaches "completed" when explicitly cancelled.
 *
 * Ported from modules/worker/executor.go.
 */

import type { AgentLoop } from "@orangecoding/agent";
import type { ChatOptions } from "@orangecoding/ai";
import type { AgentEvent } from "@orangecoding/core";
import {
  ErrorEvent,
  TaskUpdateEvent,
  type ServerEvent,
} from "@orangecoding/control-protocol";

// ---------------------------------------------------------------------------
// Executor status type
// ---------------------------------------------------------------------------

/** The possible statuses of an agent executor. */
export type ExecutorStatus = "pending" | "idle" | "running" | "completed" | "failed";

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
  private _agentEventHandler: ((event: AgentEvent) => void) | null = null;
  private _runPromise: Promise<void> | null = null;

  // Task queue with promise-based signaling
  private _taskQueue: string[] = [];
  private _taskNotify: (() => void) | null = null;

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

  /** Sets the handler for raw AgentLoop events, such as guardrail and tool events. */
  setAgentEventHandler(handler: ((event: AgentEvent) => void) | null): void {
    this._agentEventHandler = handler;
  }

  /** Sets the AbortController for cancelling this executor. */
  setAbortController(controller: AbortController): void {
    this._abortController = controller;
  }

  // -----------------------------------------------------------------------
  // Task submission
  // -----------------------------------------------------------------------

  /**
   * SubmitTask enqueues a task for processing.  If the executor is idle
   * (waiting for a task), it is woken immediately.
   */
  submitTask(task: string): void {
    this._taskQueue.push(task);
    if (this._taskNotify !== null) {
      const notify = this._taskNotify;
      this._taskNotify = null;
      notify();
    }
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
   * Run enters the task-processing loop.  It transitions to "idle" while
   * waiting for tasks and to "running" while processing one.
   *
   * If the loop is null the executor still enters the wait loop (useful
   * for testing the plumbing) but never actually invokes an agent loop.
   *
   * Returns the run promise so callers can await completion.
   */
  run(signal: AbortSignal): Promise<void> {
    this._runPromise = this._execute(signal);
    return this._runPromise;
  }

  // -----------------------------------------------------------------------
  // Internal
  // -----------------------------------------------------------------------

  /** Wait for the next task from the queue, or null if aborted. */
  private waitForTask(signal: AbortSignal): Promise<string | null> {
    // Drain synchronously if a task is already queued
    if (this._taskQueue.length > 0) {
      return Promise.resolve(this._taskQueue.shift()!);
    }

    return new Promise<string | null>((resolve) => {
      const onAbort = (): void => {
        this._taskNotify = null;
        resolve(null);
      };

      signal.addEventListener("abort", onAbort, { once: true });

      this._taskNotify = (): void => {
        signal.removeEventListener("abort", onAbort);
        resolve(this._taskQueue.shift() ?? null);
      };
    });
  }

  private async _execute(signal: AbortSignal): Promise<void> {
    // Check signal first
    if (signal.aborted) {
      this._status = "failed";
      this.emitEvent(
        new ErrorEvent(this.sessionID, `agent executor: context already canceled`),
      );
      return;
    }

    // Emit task_update: running
    this.emitEvent(
      new TaskUpdateEvent(this.sessionID, "running", "agent started"),
    );

    // ---- task-processing loop ------------------------------------------
    while (!signal.aborted) {
      this._status = "idle";

      const task = await this.waitForTask(signal);
      if (task === null || signal.aborted) {
        break;
      }

      this._status = "running";

      // Null-loop mode: record the task but don't process it
      if (this.loop === null) {
        this.emitEvent(
          new TaskUpdateEvent(this.sessionID, "task_received", task),
        );
        continue;
      }

      // Add the task as a user message in the agent conversation
      this.loop.context.addUserMessage(task);

      this.emitEvent(
        new TaskUpdateEvent(this.sessionID, "task_processing", task),
      );

      // Run the agent loop for this task
      try {
        await this.loop.run({} as ChatOptions, this._agentEventHandler);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        this.emitEvent(new ErrorEvent(this.sessionID, message));
        // Continue processing the next task — don't crash the executor
        continue;
      }

      this.emitEvent(
        new TaskUpdateEvent(this.sessionID, "task_completed", task),
      );
    }

    // ---- shutdown -------------------------------------------------------
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
