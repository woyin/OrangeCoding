/**
 * @module registry
 * Agent registry and ManagedAgent interface.
 */

import type {
  AgentId,
  AgentRole,
  AgentStatus,
  AgentCapability,
  Task,
  TaskResult,
} from "@orangecoding/core";

// ---------------------------------------------------------------------------
// AgentInfo
// ---------------------------------------------------------------------------

/** Holds metadata about a registered agent. */
export interface AgentInfo {
  id: AgentId;
  role: AgentRole;
  capabilities: AgentCapability[];
  status: AgentStatus;
}

// ---------------------------------------------------------------------------
// AgentRegistry
// ---------------------------------------------------------------------------

/** Maintains a mapping of agent IDs to their info. */
export class AgentRegistry {
  private agents = new Map<string, AgentInfo>();

  /** Register adds or replaces the entry for the given agent. */
  register(info: AgentInfo): void {
    this.agents.set(info.id.toString(), info);
  }

  /** Unregister removes the agent with the given ID. No-op if not found. */
  unregister(id: AgentId): void {
    this.agents.delete(id.toString());
  }

  /** Get returns the AgentInfo for the given ID, or undefined if not found. */
  get(id: AgentId | string): AgentInfo | undefined {
    return this.agents.get(typeof id === "string" ? id : id.toString());
  }

  /** FindByRole returns all agents whose Role matches the given role. */
  findByRole(role: AgentRole): AgentInfo[] {
    const result: AgentInfo[] = [];
    for (const info of this.agents.values()) {
      if (info.role === role) {
        result.push(info);
      }
    }
    return result;
  }

  /** FindByCapability returns all agents that have a capability with the given name. */
  findByCapability(cap: string): AgentInfo[] {
    const result: AgentInfo[] = [];
    for (const info of this.agents.values()) {
      for (const c of info.capabilities) {
        if (c.name === cap) {
          result.push(info);
          break;
        }
      }
    }
    return result;
  }
}

// ---------------------------------------------------------------------------
// HealthReport
// ---------------------------------------------------------------------------

/** Captures the health status of a managed agent. */
export interface HealthReport {
  healthy: boolean;
  lastSeen: Date;
  message: string;
}

// ---------------------------------------------------------------------------
// ManagedAgent
// ---------------------------------------------------------------------------

/** Unified interface for agent instances within the mesh. */
export interface ManagedAgent {
  id(): AgentId;
  role(): AgentRole;
  capabilities(): string[];
  status(): AgentStatus;
  assignTask(ctx: AbortSignal | undefined, task: Task): Promise<TaskResult>;
  healthCheck(ctx: AbortSignal | undefined): HealthReport;
  stop(ctx: AbortSignal | undefined, reason: string): Promise<void>;
}
