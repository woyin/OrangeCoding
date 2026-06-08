/**
 * @module pipeline
 * Sequential stage collaboration protocol.
 */

import { AgentRole, TaskStatus } from "@orangecoding/core";
import type { Task, TaskResult } from "@orangecoding/core";
import type { AgentPool } from "./agent-pool.js";
import type { CollaborationProtocol, AssignmentPlan } from "./collaboration.js";

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

/** Sequential stage collaboration. Each stage's output feeds into the next. */
export class Pipeline implements CollaborationProtocol {
  private pool: AgentPool;

  constructor(pool: AgentPool) {
    this.pool = pool;
  }

  /** Execute runs tasks sequentially, feeding each output to the next stage. */
  async execute(plan: AssignmentPlan): Promise<TaskResult[]> {
    const results: TaskResult[] = [];
    let previousOutput = "";

    for (const task of plan.tasks) {
      const augmentedTask: Task = { ...task };
      if (previousOutput) {
        augmentedTask.description =
          task.description + "\n\nPrevious stage output:\n" + previousOutput;
      }

      const worker = await this.pool.acquire(undefined, AgentRole.Executor, []);
      let result: TaskResult;
      try {
        result = await worker.assignTask(undefined, augmentedTask);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        result = {
          taskId: task.id,
          status: TaskStatus.Failed,
          output: "",
          error,
        };
        results.push(result);
        throw new Error(`stage ${task.id} failed: ${error.message}`);
      } finally {
        this.pool.release(worker.id());
      }

      results.push(result);
      previousOutput = result.output;
    }

    return results;
  }
}
