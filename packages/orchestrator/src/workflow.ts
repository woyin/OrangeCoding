/**
 * LoopWorkflow — the top-level loop abstraction that wires the 6 building blocks together.
 *
 * A LoopWorkflow combines:
 * - Scheduler (cron-based automation)
 * - WorktreeManager (parallel isolation)
 * - GoalEngine (maker/checker stop condition)
 * - PluginManager (runtime extensions)
 * - SkillDiscoverer (SKILL.md file loading)
 * - MakerChecker (code review workflow)
 */

import type { LoopWorkflowDefinition, WorkflowTrigger, MakerCheckerResult } from "./types.js";

// ---------------------------------------------------------------------------
// Orchestrator Class
// ---------------------------------------------------------------------------

export class Orchestrator {
  private readonly _workflows: Map<string, LoopWorkflowDefinition> = new Map();

  // -------------------------------------------------------------------------
  // Callbacks
  // -------------------------------------------------------------------------

  /** Called when a workflow needs to execute its task */
  onExecuteTask: ((task: string, trigger: WorkflowTrigger) => Promise<string>) | null = null;

  /** Called when a workflow completes */
  onWorkflowComplete: ((name: string, result: string) => void) | null = null;

  /** Called when a workflow fails */
  onWorkflowFail: ((name: string, error: Error) => void) | null = null;

  // -------------------------------------------------------------------------
  // Workflow Registration
  // -------------------------------------------------------------------------

  /**
   * Register a loop workflow.
   */
  register(workflow: LoopWorkflowDefinition): void {
    if (this._workflows.has(workflow.name)) {
      throw new Error(`workflow already registered: ${workflow.name}`);
    }
    this._workflows.set(workflow.name, workflow);
  }

  /**
   * Unregister a workflow.
   */
  unregister(name: string): void {
    this._workflows.delete(name);
  }

  /**
   * Get a registered workflow.
   */
  get(name: string): LoopWorkflowDefinition | undefined {
    return this._workflows.get(name);
  }

  /**
   * List all registered workflows.
   */
  list(): LoopWorkflowDefinition[] {
    return [...this._workflows.values()];
  }

  // -------------------------------------------------------------------------
  // Execution
  // -------------------------------------------------------------------------

  /**
   * Execute a loop workflow by name with the given trigger.
   */
  async execute(name: string, trigger?: WorkflowTrigger): Promise<string> {
    const workflow = this._workflows.get(name);
    if (!workflow) {
      throw new Error(`workflow not found: ${name}`);
    }

    const actualTrigger = trigger ?? workflow.trigger;

    if (!this.onExecuteTask) {
      throw new Error("onExecuteTask callback must be set before execute()");
    }

    try {
      const result = await this.onExecuteTask(workflow.task, actualTrigger);
      this.onWorkflowComplete?.(name, result);
      return result;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      this.onWorkflowFail?.(name, error);
      throw error;
    }
  }

  /**
   * Run a simple maker-checker workflow.
   *
   * This is a convenience method that creates a MakerChecker on the fly.
   */
  async runMakerChecker(
    task: string,
    makerConfig: { model?: string; systemPrompt?: string },
    checkerConfig: { model?: string; systemPrompt?: string }
  ): Promise<MakerCheckerResult> {
    const { MakerChecker } = await import("./maker-checker.js");
    const checker = new MakerChecker({
      maker: { provider: {}, ...makerConfig },
      checker: { provider: {}, ...checkerConfig },
      maxReviewIterations: 3,
    });

    return checker.execute(task);
  }
}
