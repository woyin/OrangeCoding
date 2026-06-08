/**
 * @module master-worker
 * Master-worker collaboration protocol.
 */

import { AgentRole, TaskStatus } from "@orangecoding/core";
import type { TaskResult } from "@orangecoding/core";
import type { AgentPool } from "./agent-pool.js";
import type { CollaborationProtocol, AssignmentPlan } from "./collaboration.js";

// ---------------------------------------------------------------------------
// MasterWorker
// ---------------------------------------------------------------------------

/**
 * Master-worker collaboration. The master decomposes tasks and workers
 * execute them in parallel from the pool.
 */
export class MasterWorker implements CollaborationProtocol {
  private pool: AgentPool;

  constructor(pool: AgentPool) {
    this.pool = pool;
  }

  /** Execute runs tasks in parallel using workers from the pool. */
  async execute(plan: AssignmentPlan): Promise<TaskResult[]> {
    const results: TaskResult[] = new Array(plan.tasks.length);
    let firstError: Error | undefined;

    const promises = plan.tasks.map(async (task, idx) => {
      try {
        const worker = await this.pool.acquire(undefined, AgentRole.Executor, []);
        try {
          let result = await worker.assignTask(undefined, task);
          if (result.status === TaskStatus.Failed && result.error) {
            result = {
              taskId: task.id,
              status: TaskStatus.Failed,
              output: "",
              error: result.error,
            };
          }
          results[idx] = result;
        } finally {
          this.pool.release(worker.id());
        }
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        if (!firstError) {
          firstError = new Error(`acquire worker for task ${task.id}: ${error.message}`);
        }
      }
    });

    await Promise.all(promises);

    // Fill in any missing results from failures
    for (let i = 0; i < results.length; i++) {
      if (!results[i]) {
        const task = plan.tasks[i]!;
        results[i] = {
          taskId: task.id,
          status: TaskStatus.Failed,
          output: "",
          error: firstError,
        };
      }
    }

    if (firstError) {
      return results;
    }
    return results;
  }
}
