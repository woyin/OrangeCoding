/**
 * @module pipeline
 *
 * Sequential stage collaboration protocol for multi-agent mesh networking.
 *
 * The Pipeline pattern chains multiple agent stages together where each stage's
 * output becomes context for the next stage. This is useful for workflows like:
 *   research → planning → implementation → review
 *
 * Unlike parallel collaboration (Consensus, Debate), Pipeline enforces strict
 * sequential ordering — stage N+1 only starts after stage N completes.
 */

import { AgentRole, TaskStatus } from "@orangecoding/core";
import type { Task, TaskResult } from "@orangecoding/core";
import type { AgentPool } from "./agent-pool.js";
import type { CollaborationProtocol, AssignmentPlan } from "./collaboration.js";

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

/**
 * Pipeline implements sequential stage collaboration where tasks are executed
 * one after another, with each stage's output appended as context for the next.
 *
 * If any stage fails, the pipeline throws immediately and previous results are
 * discarded. Failed stages produce a TaskResult with status=Failed before throwing.
 *
 * Thread safety: acquires and releases workers from the AgentPool for each stage,
 * ensuring safe concurrent use of the pool.
 */
export class Pipeline implements CollaborationProtocol {
  private pool: AgentPool;

  constructor(pool: AgentPool) {
    this.pool = pool;
  }

  /**
   * Execute runs the task plan sequentially through pipeline stages.
   *
   * For each task in the plan:
   * 1. Acquires a worker from the pool
   * 2. Augments the task description with previous stage output (if any)
   * 3. Assigns the task to the worker
   * 4. Collects the result and feeds output to the next stage
   *
   * @param plan - the assignment plan containing ordered tasks
   * @returns array of task results (one per stage)
   * @throws Error if any stage fails (with stage ID in the message)
   */
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
