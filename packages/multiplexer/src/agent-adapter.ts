import { AgentId, AgentStatus, type AgentRole } from "@orangecoding/core";
import type { PaneManager, ManagedPane } from "./manager.js";
import {
  IPCMessageType,
  type ResultPayload,
} from "./transport.js";
import { newAgentError } from "@orangecoding/core";

/**
 * MultiplexerAgentAdapter wraps an agent to run in a separate terminal pane.
 */
/**
 * MultiplexerAgentAdapter runs an agent in a separate terminal pane.
 *
 * Bridges the agent system with the terminal multiplexer (tmux/zellij):
 * - Spawns a new pane running the pane-agent binary
 * - Establishes IPC via Unix domain sockets
 * - Sends task payloads and receives streaming events
 * - Collects the final result when the agent completes
 *
 * This enables parallel agent execution across terminal panes,
 * each with its own visible output for debugging and monitoring.
 */
export class MultiplexerAgentAdapter {
  private readonly id: AgentId;
  private status: AgentStatus = AgentStatus.Idle;
  private abortController: AbortController | null = null;

  /**
   * Constructs an adapter for the given role that drives a managed pane lifecycle
   * (spawn → IPC receive loop → completion) via the PaneManager.
   */
  constructor(
    private readonly role: AgentRole,
    private readonly manager: PaneManager,
  ) {
    this.id = AgentId.create();
  }

  /** getID returns the adapter agent identifier. */
  getID(): AgentId {
    return this.id;
  }

  /** getRole returns the configured agent role. */
  getRole(): AgentRole {
    return this.role;
  }

  /** getStatus returns the current execution status of the agent. */
  getStatus(): AgentStatus {
    return this.status;
  }

  /**
   * Stop cancels the running agent and closes its pane.
   */
  async stop(): Promise<void> {
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
    this.status = AgentStatus.Completed;
  }

  /**
   * Run spawns a pane, sends the task, and waits for the result.
   */
  async run(signal: AbortSignal, task: string): Promise<void> {
    this.status = AgentStatus.Running;
    this.abortController = new AbortController();
    const combinedSignal = AbortSignal.any([signal, this.abortController.signal]);

    try {
      const managed = await this.manager.spawnAgentPane(
        this.id.toString(),
        task,
        combinedSignal,
      );

      // Receive loop: process events and wait for result.
      for (;;) {
        let msg;
        try {
          msg = await managed.transport.receive();
        } catch (err) {
          this.status = AgentStatus.Failed;
          throw newAgentError(this.id.toString(), `receive from pane: ${(err as Error).message}`);
        }

        switch (msg.type) {
          case IPCMessageType.Result: {
            const result = msg.payload as ResultPayload;
            if (result.success) {
              this.status = AgentStatus.Completed;
            } else {
              this.status = AgentStatus.Failed;
            }
            if (!result.success && result.error) {
              throw newAgentError(this.id.toString(), `agent error: ${result.error}`);
            }
            return;
          }

          case IPCMessageType.Event:
          case IPCMessageType.Keepalive:
            // Informational; continue receiving.
            continue;

          default:
            continue;
        }
      }
    } catch (err) {
      this.status = AgentStatus.Failed;
      throw err;
    } finally {
      this.abortController = null;
    }
  }
}
