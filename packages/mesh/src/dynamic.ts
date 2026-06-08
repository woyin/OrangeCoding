/**
 * @module dynamic
 * DynamicCollaboration wires all collaboration protocols and supports runtime switching.
 */

import { TaskType, TaskStatus } from "@orangecoding/core";
import type { Task, TaskResult, TaskType as TaskTypeType } from "@orangecoding/core";
import type { AgentPool } from "./agent-pool.js";
import type { AgentRegistry } from "./registry.js";
import type { TaskClassifier, CollaborationProtocol, AssignmentPlan } from "./collaboration.js";
import { CollaborationRouter } from "./collaboration.js";
import { MasterWorker } from "./master-worker.js";
import { Pipeline } from "./pipeline.js";
import { PeerNegotiation } from "./peer.js";

// ---------------------------------------------------------------------------
// DynamicCollaboration
// ---------------------------------------------------------------------------

/**
 * Wires all collaboration protocols and supports runtime switching.
 * Routes tasks to the appropriate protocol based on task type:
 *   - Coding -> MasterWorker
 *   - Review -> Pipeline
 *   - Exploration -> PeerNegotiation
 */
export class DynamicCollaboration implements CollaborationProtocol {
  private router: CollaborationRouter;
  private pool: AgentPool;
  private registry: AgentRegistry;

  constructor(
    pool: AgentPool,
    registry: AgentRegistry,
    classifier: TaskClassifier,
  ) {
    const protocols = new Map<string, CollaborationProtocol>();
    protocols.set(TaskType.Coding, new MasterWorker(pool));
    protocols.set(TaskType.Review, new Pipeline(pool));
    protocols.set(TaskType.Exploration, new PeerNegotiation(pool, registry, { minScore: 0, timeoutMs: 0 }));

    this.router = new CollaborationRouter(classifier, protocols);
    this.pool = pool;
    this.registry = registry;
  }

  /** Execute implements CollaborationProtocol by routing each task through the router. */
  async execute(plan: AssignmentPlan): Promise<TaskResult[]> {
    const allResults: TaskResult[] = [];

    for (const task of plan.tasks) {
      try {
        const results = await this.router.route(task);
        allResults.push(...results);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        allResults.push({
          taskId: task.id,
          status: TaskStatus.Failed,
          output: "",
          error,
        });
      }
    }

    return allResults;
  }

  /** Route classifies and delegates a task to the appropriate protocol. */
  async route(task: Task): Promise<TaskResult[]> {
    return this.router.route(task);
  }

  /** SwitchProtocol replaces the protocol for a task type at runtime. */
  switchProtocol(taskType: TaskTypeType, protocol: CollaborationProtocol): void {
    this.router.setProtocol(taskType, protocol);
  }
}
