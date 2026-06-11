/**
 * WorkerRuntime manages multiple agent sessions. It tracks running agents
 * and provides lifecycle operations (start, stop, list, status).
 *
 * Ported from modules/worker/runtime.go.
 */

import { AgentExecutor, type ExecutorStatus } from "./executor.js";
import type { AgentLoop } from "@orangecoding/agent";
import type { AgentEvent } from "@orangecoding/core";
import type { ServerEvent } from "@orangecoding/control-protocol";

// ---------------------------------------------------------------------------
// WorkerRuntime
// ---------------------------------------------------------------------------

/**
 * WorkerRuntime manages multiple agent sessions. It tracks running agents
 * and provides lifecycle operations (start, stop, list, status, shutdown).
 */
export class WorkerRuntime {
  private agents = new Map<string, AgentExecutor>();
  private pendingTasks = new Map<string, string[]>();

  /**
   * @param eventHandler - Handler for emitting server events. May be null.
   */
  constructor(
    private readonly eventHandler: ((event: ServerEvent) => void) | null,
    private readonly agentEventHandler: ((event: AgentEvent) => void) | null = null,
  ) {}

  // -----------------------------------------------------------------------
  // Session management
  // -----------------------------------------------------------------------

  /**
   * StartSession creates a new agent executor and starts it asynchronously.
   * The executor enters a task-processing loop, waiting for tasks to be
   * submitted via submitTask().
   *
   * Returns an error if a session with the same ID already exists.
   * If agentLoop is null, the executor records tasks but does not process them.
   */
  startSession(sessionID: string, agentLoop: AgentLoop | null): void {
    if (this.agents.has(sessionID)) {
      throw new Error(`worker runtime: session "${sessionID}" already exists`);
    }

    const executor = new AgentExecutor(sessionID, agentLoop);
    executor.setEventHandler(this.eventHandler);
    executor.setAgentEventHandler(this.agentEventHandler);

    const controller = new AbortController();
    executor.setAbortController(controller);

    this.agents.set(sessionID, executor);

    // Run asynchronously (fire-and-forget, matching Go goroutine behavior)
    void executor.run(controller.signal).catch(() => {
      // Error is already handled inside the executor (status set to "failed").
      // Swallow here to avoid unhandled promise rejection.
    });
  }

  /**
   * StopSession cancels and removes a running agent session.
   * Throws if the session does not exist.
   */
  stopSession(sessionID: string): void {
    const executor = this.agents.get(sessionID);
    if (executor == null) {
      throw new Error(`worker runtime: session "${sessionID}" not found`);
    }

    executor.cancel();
    this.agents.delete(sessionID);
    this.pendingTasks.delete(sessionID);
  }

  /**
   * ListSessions returns the IDs of all active sessions.
   */
  listSessions(): string[] {
    return Array.from(this.agents.keys());
  }

  /**
   * GetStatus returns the execution status of a session.
   * The second return value is false if the session does not exist.
   */
  getStatus(sessionID: string): [ExecutorStatus, true] | [undefined, false] {
    const executor = this.agents.get(sessionID);
    if (executor == null) {
      return [undefined, false];
    }
    return [executor.status, true];
  }

  /**
   * SubmitTask enqueues a task for an active session and forwards it to
   * the executor for processing through the agent loop.
   */
  submitTask(sessionID: string, task: string): void {
    const executor = this.agents.get(sessionID);
    if (executor == null) {
      throw new Error(`worker runtime: session "${sessionID}" not found`);
    }

    // Record the task for query purposes
    const tasks = this.pendingTasks.get(sessionID) ?? [];
    tasks.push(task);
    this.pendingTasks.set(sessionID, tasks);

    // Forward to executor for actual processing
    executor.submitTask(task);
  }

  /** PendingTasks returns queued task strings for an active session. */
  pendingTasksFor(sessionID: string): string[] {
    return [...(this.pendingTasks.get(sessionID) ?? [])];
  }

  /**
   * Shutdown cancels all running sessions and waits for them to finish.
   */
  shutdown(): void {
    const agents = Array.from(this.agents.values());
    this.agents.clear();
    this.pendingTasks.clear();

    for (const executor of agents) {
      executor.cancel();
    }
  }
}
