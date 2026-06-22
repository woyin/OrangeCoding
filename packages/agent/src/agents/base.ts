/**
 * @module base-agent
 *
 * Base agent implementation with shared functionality.
 *
 * Provides common agent behavior that specialized agents extend:
 * - Agent lifecycle (init, run, shutdown)
 * - Event emission
 * - Configuration management
 * - Error handling and recovery
 */

import type { AgentId, AgentRole, AgentStatus, Task, TaskResult } from "@orangecoding/core";
import { AgentId as AgentIdClass, AgentStatus as AgentStatusEnum } from "@orangecoding/core";
import type { HealthReport, ManagedAgent } from "@orangecoding/mesh";
import type { AgentLoop } from "../loop.js";

// ---------------------------------------------------------------------------
// Agent interface
// ---------------------------------------------------------------------------

/**
 * Agent is the common contract for all agents: identity (id/role), lifecycle
 * (run/stop), and status query. Implementations include BaseAgent and its
 * specialized subclasses.
 */
export interface Agent {
  id(): AgentId;
  role(): AgentRole;
  run(signal: AbortSignal | undefined, task: string): Promise<void>;
  stop(signal: AbortSignal | undefined, reason: string): Promise<void>;
  status(): AgentStatus;
}

// ---------------------------------------------------------------------------
// BaseAgent
// ---------------------------------------------------------------------------

export class BaseAgent implements Partial<ManagedAgent> {
  private _id: AgentId;
  private _role: AgentRole;
  private _loop: AgentLoop;
  private _status: AgentStatus;
  private _abortController: AbortController | null;

  constructor(role: AgentRole, loop: AgentLoop) {
    this._id = AgentIdClass.create();
    this._role = role;
    this._loop = loop;
    this._status = AgentStatusEnum.Idle;
    this._abortController = null;
  }

  /** id returns the unique agent identifier. */
  id(): AgentId { return this._id; }
  /** role returns the configured agent role. */
  role(): AgentRole { return this._role; }

  /** status returns the current lifecycle status of this agent. */
  status(): AgentStatus {
    return this._status;
  }

  async stop(_signal: AbortSignal | undefined, _reason: string): Promise<void> {
    if (this._abortController) {
      this._abortController.abort();
      this._abortController = null;
    }
    this._status = AgentStatusEnum.Completed;
  }

  /** Run executes the agent loop for the given task. */
  async run(_signal: AbortSignal | undefined, task: string): Promise<void> {
    this._status = AgentStatusEnum.Running;
    this._abortController = new AbortController();

    // Add the task as a user message
    this._loop.context.addUserMessage(task);

    try {
      const result = await this._loop.run({}, null);

      if (this._abortController) {
        this._abortController.abort();
        this._abortController = null;
      }

      this._status = AgentStatusEnum.Completed;
    } catch (err) {
      this._status = AgentStatusEnum.Failed;
      throw new Error(`agent ${this._id} failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  /** Loop returns the underlying AgentLoop. */
  get loop(): AgentLoop {
    return this._loop;
  }

  /** Capabilities returns the tools this agent can use. */
  capabilities(): string[] {
    return ["bash", "read", "write", "edit"];
  }

  /** AssignTask implements mesh.ManagedAgent. */
  async assignTask(_signal: AbortSignal | undefined, task: Task): Promise<TaskResult> {
    try {
      await this.run(undefined, task.description);
      return {
        taskId: task.id,
        status: "completed" as TaskResult["status"],
        output: "",
      };
    } catch (err) {
      return {
        taskId: task.id,
        status: "failed" as TaskResult["status"],
        output: "",
        error: err instanceof Error ? err : new Error(String(err)),
      };
    }
  }

  /** HealthCheck implements mesh.ManagedAgent. */
  healthCheck(_signal: AbortSignal | undefined): HealthReport {
    return {
      healthy: this._status === AgentStatusEnum.Running || this._status === AgentStatusEnum.Waiting || this._status === AgentStatusEnum.Idle,
      lastSeen: new Date(),
      message: "",
    };
  }
}
