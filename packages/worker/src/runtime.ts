/**
 * WorkerRuntime manages multiple agent sessions. It tracks running agents
 * and provides lifecycle operations (start, stop, list, status).
 *
 * Ported from modules/worker/runtime.go.
 */

import { AgentExecutor, type ExecutorStatus } from "./executor.js";
import type { AgentLoop } from "@orangecoding/agent";
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

  /**
   * @param eventHandler - Handler for emitting server events. May be null.
   */
  constructor(
    private readonly eventHandler: ((event: ServerEvent) => void) | null,
  ) {}

  // -----------------------------------------------------------------------
  // Session management
  // -----------------------------------------------------------------------

  /**
   * StartSession creates a new agent executor and starts it asynchronously.
   * Returns an error if a session with the same ID already exists.
   * If agentLoop is null, the executor will complete immediately (useful for testing).
   */
  startSession(sessionID: string, agentLoop: AgentLoop | null): void {
    if (this.agents.has(sessionID)) {
      throw new Error(`worker runtime: session "${sessionID}" already exists`);
    }

    const executor = new AgentExecutor(sessionID, agentLoop);
    executor.setEventHandler(this.eventHandler);

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
   * Shutdown cancels all running sessions and waits for them to finish.
   */
  shutdown(): void {
    const agents = Array.from(this.agents.values());
    this.agents.clear();

    for (const executor of agents) {
      executor.cancel();
    }
  }
}
