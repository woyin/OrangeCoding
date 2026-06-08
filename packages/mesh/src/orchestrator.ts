/**
 * @module orchestrator
 * Task orchestrator with topological sort and concurrent execution.
 */

// ---------------------------------------------------------------------------
// TaskFunc
// ---------------------------------------------------------------------------

/** Signature for a task's executable function. */
export type TaskFunc = () => Promise<void>;

// ---------------------------------------------------------------------------
// OrchestratorTask
// ---------------------------------------------------------------------------

/** A unit of work with an identifier, dependency list, and executable function. */
export interface OrchestratorTask {
  id: string;
  deps: string[];
  fn: TaskFunc;
}

// ---------------------------------------------------------------------------
// TaskOrchestrator
// ---------------------------------------------------------------------------

/**
 * Stores a set of tasks and executes them respecting declared dependency ordering.
 * Tasks that share no dependency chain run concurrently (Kahn's algorithm).
 */
export class TaskOrchestrator {
  private tasks = new Map<string, OrchestratorTask>();

  /**
   * AddTask registers a task with the given id, dependency list, and function.
   * If a task with the same id already exists it is replaced.
   */
  addTask(id: string, deps: string[], fn: TaskFunc): void {
    this.tasks.set(id, { id, deps, fn });
  }

  /**
   * Run performs a topological sort of all registered tasks and executes them
   * in dependency order. Tasks whose dependencies have all completed are
   * dispatched concurrently.
   *
   * @param signal - Optional AbortSignal to cancel execution.
   * @returns The first error encountered by any task, or undefined if all succeed.
   */
  async run(signal?: AbortSignal): Promise<Error | undefined> {
    // Snapshot tasks.
    const tasks = new Map<string, OrchestratorTask>();
    for (const [k, v] of this.tasks) {
      tasks.set(k, v);
    }

    // Build in-degree map and adjacency list (dep -> dependents).
    const inDegree = new Map<string, number>();
    const dependents = new Map<string, string[]>(); // dep -> list of task IDs that depend on it

    for (const [id, t] of tasks) {
      if (!inDegree.has(id)) {
        inDegree.set(id, 0);
      }
      for (const dep of t.deps) {
        inDegree.set(id, (inDegree.get(id) ?? 0) + 1);
        const list = dependents.get(dep) ?? [];
        list.push(id);
        dependents.set(dep, list);
      }
    }

    // Collect tasks with no dependencies (in-degree 0).
    let ready: string[] = [];
    for (const [id, deg] of inDegree) {
      if (deg === 0) {
        ready.push(id);
      }
    }

    let firstError: Error | undefined;

    const execOne = async (id: string): Promise<void> => {
      const t = tasks.get(id)!;
      try {
        await t.fn();
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        const wrapped = new Error(`task ${id} failed: ${error.message}`);
        if (!firstError) {
          firstError = wrapped;
        }
        return;
      }

      // Decrement in-degree of dependents and enqueue newly ready tasks.
      for (const depID of dependents.get(id) ?? []) {
        const current = (inDegree.get(depID) ?? 1) - 1;
        inDegree.set(depID, current);
        if (current === 0) {
          ready.push(depID);
        }
      }
    };

    // Kahn's algorithm with concurrent execution.
    while (true) {
      const batch = ready.slice();
      ready = [];

      if (batch.length === 0) break;

      // Check for cancellation or prior error before dispatching.
      if (signal?.aborted) {
        return new Error(signal.reason ?? "aborted");
      }
      if (firstError) break;

      // Execute all tasks in the batch concurrently.
      await Promise.all(batch.map((id) => execOne(id)));

      if (firstError) {
        return firstError;
      }
    }

    // Check for cycles.
    for (const [id, deg] of inDegree) {
      if (deg > 0) {
        return new Error(`dependency cycle detected involving task "${id}"`);
      }
    }

    return undefined;
  }
}
