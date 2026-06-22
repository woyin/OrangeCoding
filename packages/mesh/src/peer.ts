/**
 * @module peer
 *
 * Peer-to-peer agent communication in the mesh network.
 *
 * Each peer represents an agent node in the mesh that can:
 * - Advertise its capabilities and skills
 * - Accept or reject incoming tasks
 * - Communicate with other peers via the bus
 * - Track health and availability of other peers
 *
 * Peers form a decentralized mesh where any agent can request
 * help from any other agent, subject to capability matching.
 */

import { TaskStatus, TaskType, AgentRole, AgentStatus } from "@orangecoding/core";
import type { Task, TaskResult, AgentRole as AgentRoleType, AgentStatus as AgentStatusType } from "@orangecoding/core";
import type { AgentPool } from "./agent-pool.js";
import type { AgentRegistry, AgentInfo } from "./registry.js";
import type { CollaborationProtocol, AssignmentPlan } from "./collaboration.js";

// ---------------------------------------------------------------------------
// Bid
// ---------------------------------------------------------------------------

/** Represents an agent's offer to handle a task. */
export interface Bid {
  agentId: string;
  taskId: string;
  score: number;
  load: number;
}

// ---------------------------------------------------------------------------
// PeerNegotiationConfig
// ---------------------------------------------------------------------------

/** Configures the peer negotiation protocol. */
export interface PeerNegotiationConfig {
  /** Minimum score to accept a bid (default 0.3) */
  minScore: number;
  /** Bid collection timeout in milliseconds (default 5000) */
  timeoutMs: number;
}

// ---------------------------------------------------------------------------
// PeerNegotiation
// ---------------------------------------------------------------------------

/**
 * CollaborationProtocol with a bidding-based approach.
 * Agents claim tasks based on capability matching rather than central assignment.
 */
export class PeerNegotiation implements CollaborationProtocol {
  private pool: AgentPool;
  private registry: AgentRegistry;
  private minScore: number;
  private timeoutMs: number;

  constructor(
    pool: AgentPool,
    registry: AgentRegistry,
    config: PeerNegotiationConfig,
  ) {
    this.pool = pool;
    this.registry = registry;
    this.minScore = config.minScore > 0 ? config.minScore : 0.3;
    this.timeoutMs = config.timeoutMs > 0 ? config.timeoutMs : 5000;
  }

  /** Execute implements CollaborationProtocol. */
  async execute(plan: AssignmentPlan): Promise<TaskResult[]> {
    const results: TaskResult[] = new Array(plan.tasks.length);

    for (let i = 0; i < plan.tasks.length; i++) {
      const task = plan.tasks[i]!;
      try {
        const result = await this.negotiateTask(task);
        results[i] = result;
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        results[i] = {
          taskId: task.id,
          status: TaskStatus.Failed,
          output: "",
          error,
        };
      }
    }

    return results;
  }

  /**
   * Negotiates a single task: collect bids from eligible idle agents, pick a
   * winner, acquire it from the pool, run the task, and release it. Errors
   * surface as a Failed TaskResult so the batch continues.
   */
  private async negotiateTask(task: Task): Promise<TaskResult> {
    const bids = this.collectBids(task);
    if (bids.length === 0) {
      throw new Error(`peer negotiation: no bids for task ${task.id}`);
    }

    const winner = this.selectWinner(bids);
    // Bid.agentId is stored as string (AgentInfo.id.toString()), look up directly
    const info = this.registry.get(winner.agentId);
    if (!info) {
      throw new Error(`peer negotiation: winning agent ${winner.agentId} not found`);
    }

    const caps = capsToNames(info);
    const agent = await this.pool.acquire(undefined, info.role, caps);
    try {
      const result = await agent.assignTask(undefined, task);
      if (result.status === TaskStatus.Failed && result.error) {
        return {
          taskId: task.id,
          status: TaskStatus.Failed,
          output: "",
          error: result.error,
        };
      }
      return result;
    } finally {
      this.pool.release(agent.id());
    }
  }

  /**
   * Gathers bids for a task. Tries the preferred role first, then falls
   * back to Planner (general-purpose) then Executor (generic worker) so a
   * task is never left unassigned when a specialist is unavailable.
   */
  private collectBids(task: Task): Bid[] {
    const role = preferredRole(task.type);
    let bids = this.bidsForRole(role, task);

    // Fallback: try planner as general-purpose agent.
    if (bids.length === 0 && role !== AgentRole.Planner) {
      bids = this.bidsForRole(AgentRole.Planner, task);
    }
    // Fallback: try executor as generic worker.
    if (bids.length === 0 && role !== AgentRole.Executor) {
      bids = this.bidsForRole(AgentRole.Executor, task);
    }
    return bids;
  }

  /**
   * Computes bids from all idle agents of `role`. Each bid scores the
   * role/task match (1.0 for a perfect specialty match, lower otherwise);
   * bids below minScore are dropped to avoid hopeless assignments.
   */
  private bidsForRole(role: AgentRoleType, task: Task): Bid[] {
    const agents = this.registry.findByRole(role);
    const bids: Bid[] = [];

    for (const info of agents) {
      if (info.status !== AgentStatus.Idle) {
        continue;
      }
      const score = roleMatchScore(info.role, task.type);
      if (score < this.minScore) {
        continue;
      }
      bids.push({
        agentId: info.id.toString(),
        taskId: task.id,
        score,
        load: loadFromStatus(info.status),
      });
    }
    return bids;
  }

  /**
   * Picks the winning bid: highest score, ties broken by lowest load.
   *
   * Performance: single O(n) pass instead of the previous O(n log n)
   * sort + slice. We only need the single best bid, not a full ordering,
   * so a linear scan with the same comparison is both faster and allocation-
   * free (no sorted copy).
   */
  private selectWinner(bids: Bid[]): Bid {
    let best = bids[0]!;
    for (let i = 1; i < bids.length; i++) {
      const b = bids[i]!;
      // Higher score wins; on tie, lower load wins (mirrors the old sort key).
      if (b.score > best.score || (b.score === best.score && b.load < best.load)) {
        best = b;
      }
    }
    return best;
  }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

function preferredRole(tt: TaskType): AgentRoleType {
  switch (tt) {
    case TaskType.Coding:
      return AgentRole.Coder;
    case TaskType.Review:
      return AgentRole.Reviewer;
    case TaskType.Exploration:
      return AgentRole.Executor;
    default:
      return AgentRole.Executor;
  }
}

function roleMatchScore(role: AgentRoleType, tt: TaskType): number {
  if (role === AgentRole.Coder && tt === TaskType.Coding) return 1.0;
  if (role === AgentRole.Reviewer && tt === TaskType.Review) return 1.0;
  if (role === AgentRole.Executor && tt === TaskType.Exploration) return 1.0;
  if (role === AgentRole.Planner) return 0.5;
  if (role === AgentRole.Executor) return 0.4;
  return 0.2;
}

function loadFromStatus(s: AgentStatusType): number {
  switch (s) {
    case AgentStatus.Idle:
      return 0.0;
    case AgentStatus.Running:
      return 0.7;
    default:
      return 1.0;
  }
}

function capsToNames(info: AgentInfo): string[] {
  return info.capabilities.map((c) => c.name);
}
