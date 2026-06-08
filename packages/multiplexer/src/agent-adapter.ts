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
export class MultiplexerAgentAdapter {
  private readonly id: AgentId;
  private status: AgentStatus = AgentStatus.Idle;
  private abortController: AbortController | null = null;

  constructor(
    private readonly role: AgentRole,
    private readonly manager: PaneManager,
  ) {
    this.id = AgentId.create();
  }

  getID(): AgentId {
    return this.id;
  }

  getRole(): AgentRole {
    return this.role;
  }

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
