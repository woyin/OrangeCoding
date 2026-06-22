/**
 * @module agent-pool
 *
 * Agent pool management for the mesh network.
 *
 * The AgentPool manages a pool of worker agents, handling:
 * - Agent creation and destruction
 * - Resource tracking (active agents, available capacity)
 * - Task assignment based on agent role and capabilities
 * - Agent lifecycle management (idle -> busy -> idle)
 *
 * The pool enforces concurrency limits and ensures efficient
 * agent reuse to minimize startup overhead.
 */

import type { AgentId, AgentRole, AgentStatus } from "@orangecoding/core";
import type { ManagedAgent } from "./registry.js";

// ---------------------------------------------------------------------------
// AgentPoolConfig
// ---------------------------------------------------------------------------

/** Configures the agent pool. */
export interface AgentPoolConfig {
  maxAgents: number; // 0 means unlimited
  idleTimeoutMs: number;
}

// ---------------------------------------------------------------------------
// PoolStatus
// ---------------------------------------------------------------------------

/** Summarizes agent pool state. */
export interface PoolStatus {
  active: number;
  idle: number;
  total: number;
}

// ---------------------------------------------------------------------------
// AgentFactory
// ---------------------------------------------------------------------------

/** Creates a new ManagedAgent instance. */
export type AgentFactory = (
  signal: AbortSignal | undefined,
  role: AgentRole,
  caps: string[],
) => Promise<ManagedAgent>;

// ---------------------------------------------------------------------------
// Internal pool entry
// ---------------------------------------------------------------------------

interface PoolEntry {
  agent: ManagedAgent;
  status: AgentStatus;
  idleSince: Date | null;
}

// ---------------------------------------------------------------------------
// AgentPool
// ---------------------------------------------------------------------------

/** Manages a set of ManagedAgent instances with acquire/release semantics. */
export class AgentPool {
  private agents = new Map<string, PoolEntry>();
  private config: AgentPoolConfig;
  private factory: AgentFactory;

  constructor(config: AgentPoolConfig, factory: AgentFactory) {
    this.config = config;
    this.factory = factory;
  }

  /** Acquire gets or creates an agent matching the role and capabilities. */
  async acquire(
    signal: AbortSignal | undefined,
    role: AgentRole,
    caps: string[],
  ): Promise<ManagedAgent> {
    // Try to reuse an idle agent with the matching role.
    for (const [key, entry] of this.agents) {
      if (entry.status === "idle" && entry.agent.role() === role) {
        this.agents.set(key, { agent: entry.agent, status: "running", idleSince: null });
        return entry.agent;
      }
    }

    // Check capacity before creating a new agent.
    if (this.config.maxAgents > 0 && this.agents.size >= this.config.maxAgents) {
      throw new Error(`agent pool at capacity: ${this.agents.size}/${this.config.maxAgents}`);
    }

    const agent = await this.factory(signal, role, caps);
    this.agents.set(agent.id().toString(), {
      agent,
      status: "running",
      idleSince: null,
    });
    return agent;
  }

  /** Release returns an agent to the pool, making it available for reuse. */
  release(id: AgentId): void {
    const entry = this.agents.get(id.toString());
    if (!entry) return;
    this.agents.set(id.toString(), {
      agent: entry.agent,
      status: "idle",
      idleSince: new Date(),
    });
  }

  /** Remove permanently removes an agent from the pool. */
  remove(id: AgentId): void {
    this.agents.delete(id.toString());
  }

  /** Status returns the current pool status. */
  status(): PoolStatus {
    let active = 0;
    let idle = 0;
    for (const entry of this.agents.values()) {
      if (entry.status === "running") {
        active++;
      } else {
        idle++;
      }
    }
    return { active, idle, total: this.agents.size };
  }
}
