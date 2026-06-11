/**
 * GoalEngine — manages autonomous goal iteration with maker/checker separation.
 *
 * The engine runs a maker agent to work toward a goal, then uses a separate
 * evaluator (checker) to decide whether the goal is complete. The loop continues
 * until the condition is met, the iteration limit is reached, or the goal is
 * explicitly cancelled.
 */

import { randomUUID } from "node:crypto";
import { FileGoalStore, MemoryGoalStore } from "./store.js";
import { GoalEvaluator } from "./evaluator.js";
import {
  GoalConfig,
  GoalState,
  GoalStatus,
  GoalResult,
  GoalError,
  newGoalError,
  DEFAULT_GOAL_ENGINE_CONFIG,
  EvaluationResult,
  EvaluationContext,
} from "./types.js";
import type { GoalEngineConfig } from "./types.js";
import type { GoalStore } from "./store.js";
import type { TokenUsage } from "@orangecoding/core";
import { TokenUsage as TokenUsageClass } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// Engine Class
// ---------------------------------------------------------------------------

export class GoalEngine {
  private readonly _config: Required<GoalEngineConfig>;
  private readonly _store: GoalStore;
  private readonly _evaluator: GoalEvaluator;
  private readonly _goals: Map<string, GoalState> = new Map();
  private _running = false;

  // -------------------------------------------------------------------------
  // Callbacks
  // -------------------------------------------------------------------------

  /** Called when the engine needs to run a work iteration. Return the output. */
  onExecuteIteration: ((goal: GoalState) => Promise<string>) | null = null;

  /** Called after each iteration completes */
  onIteration: ((state: GoalState) => void) | null = null;

  /** Called when a goal completes successfully */
  onCompleted: ((state: GoalState, result: GoalResult) => void) | null = null;

  /** Called when a goal fails */
  onFailed: ((state: GoalState, error: Error) => void) | null = null;

  // -------------------------------------------------------------------------
  // Constructor
  // -------------------------------------------------------------------------

  constructor(
    evaluator: GoalEvaluator,
    config?: Partial<GoalEngineConfig>,
    store?: GoalStore
  ) {
    this._evaluator = evaluator;
    this._config = {
      defaultMaxIterations: config?.defaultMaxIterations ?? DEFAULT_GOAL_ENGINE_CONFIG.defaultMaxIterations,
      maxConcurrentGoals: config?.maxConcurrentGoals ?? DEFAULT_GOAL_ENGINE_CONFIG.maxConcurrentGoals,
      storeDir: config?.storeDir ?? DEFAULT_GOAL_ENGINE_CONFIG.storeDir,
    };
    this._store = store ?? new FileGoalStore(this._config.storeDir);
  }

  // -------------------------------------------------------------------------
  // Lifecycle
  // -------------------------------------------------------------------------

  /**
   * Start executing toward a goal.
   *
   * @param goalConfig - the goal to complete
   * @param signal - optional abort signal
   * @returns goal result when finished
   */
  async start(goalConfig: GoalConfig, signal?: AbortSignal): Promise<GoalResult> {
    if (this._goals.size >= this._config.maxConcurrentGoals) {
      throw newGoalError("MAX_CONCURRENT", "maximum concurrent goals reached");
    }

    if (!this.onExecuteIteration) {
      throw newGoalError("NO_EXECUTOR", "onExecuteIteration callback must be set before start()");
    }

    const maxIterations = goalConfig.maxIterations ?? this._config.defaultMaxIterations;
    const state: GoalState = {
      id: goalConfig.id || randomUUID(),
      config: { ...goalConfig, maxIterations },
      status: GoalStatus.Active,
      iteration: 0,
      totalTokensUsed: 0,
      lastEvalResult: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    };

    this._goals.set(state.id, state);
    await this._store.save(state);

    const startTime = Date.now();
    const results: EvaluationResult[] = [];

    try {
      while (
        state.status === GoalStatus.Active &&
        state.iteration < maxIterations &&
        !signal?.aborted
      ) {
        state.iteration++;
        state.updatedAt = new Date();

        // Execute one iteration (maker step)
        let iterationOutput = "";
        try {
          iterationOutput = await this.onExecuteIteration(state);
        } catch (err) {
          const error = err instanceof Error ? err : new Error(String(err));
          state.status = GoalStatus.Failed;
          this.onFailed?.(state, error);
          await this._store.save(state);
          return this._buildResult(state, startTime, error.message);
        }

        // Evaluate completion (checker step)
        const evalContext: EvaluationContext = {
          iteration: state.iteration,
          recentOutput: iterationOutput,
        };

        const evalResult = await this._evaluator.evaluate(state.config.condition, evalContext);
        state.lastEvalResult = evalResult;
        results.push(evalResult);
        state.updatedAt = new Date();

        this.onIteration?.(state);

        if (evalResult.completed && evalResult.confidence >= 0.5) {
          state.status = GoalStatus.Completed;
          this.onCompleted?.(state, this._buildResult(state, startTime));
          await this._store.save(state);
          return this._buildResult(state, startTime);
        }
      }

      // Exhausted iterations without completion
      if (state.iteration >= maxIterations) {
        state.status = GoalStatus.Failed;
        this.onFailed?.(state, new Error(`goal exceeded maximum iterations (${maxIterations})`));
        await this._store.save(state);
        return this._buildResult(state, startTime, "max iterations reached");
      }

      // Cancelled via signal
      state.status = GoalStatus.Failed;
      await this._store.save(state);
      return this._buildResult(state, startTime, "cancelled");
    } finally {
      this._goals.delete(state.id);
    }
  }

  /**
   * Pause a running goal.
   */
  async pause(goalId: string): Promise<void> {
    const state = this._goals.get(goalId);
    if (!state || state.status !== GoalStatus.Active) return;

    state.status = GoalStatus.Paused;
    state.updatedAt = new Date();
    await this._store.save(state);
  }

  /**
   * Resume a paused goal.
   */
  async resume(goalId: string): Promise<void> {
    const state = this._goals.get(goalId);
    if (!state || state.status !== GoalStatus.Paused) return;

    state.status = GoalStatus.Active;
    state.updatedAt = new Date();
    await this._store.save(state);
  }

  /**
   * Cancel a goal.
   */
  async cancel(goalId: string): Promise<void> {
    const state = this._goals.get(goalId);
    if (!state) return;

    state.status = GoalStatus.Failed;
    state.updatedAt = new Date();
    await this._store.save(state);
    this._goals.delete(goalId);
  }

  /**
   * Get the current state of a goal.
   */
  getState(goalId: string): GoalState | undefined {
    return this._goals.get(goalId);
  }

  /**
   * List all active goals.
   */
  listActive(): GoalState[] {
    return [...this._goals.values()].filter((g) => g.status === GoalStatus.Active || g.status === GoalStatus.Paused);
  }

  // -------------------------------------------------------------------------
  // Private
  // -------------------------------------------------------------------------

  private _buildResult(
    state: GoalState,
    startTime: number,
    errMsg?: string
  ): GoalResult {
    const lastEval = state.lastEvalResult ?? {
      completed: false,
      confidence: 0,
      reason: errMsg ?? "no evaluation available",
      remainingBlockers: [],
      suggestions: [],
    };

    return {
      goalId: state.id,
      success: state.status === GoalStatus.Completed,
      iterations: state.iteration,
      tokenUsage: TokenUsageClass.create(0, 0),
      durationMs: Date.now() - startTime,
      finalEval: lastEval,
      finalStatus: state.status,
    };
  }
}
