/**
 * @orangecoding/goal — Goal evaluation with maker/checker separation.
 *
 * Re-exports all public API from the package.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
export {
  GoalStatus,
  DEFAULT_GOAL_ENGINE_CONFIG,
  GoalError,
  newGoalError,
} from "./types.js";
export type {
  GoalConfig,
  GoalEngineConfig,
  GoalState,
  EvaluationResult,
  EvaluationContext,
  GoalResult,
  GoalStatus as GoalStatusType,
} from "./types.js";

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------
export { GoalEvaluator } from "./evaluator.js";
export type { GoalEvaluatorConfig } from "./evaluator.js";

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------
export { FileGoalStore, MemoryGoalStore } from "./store.js";
export type { GoalStore } from "./store.js";

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------
export { GoalEngine } from "./goal.js";
