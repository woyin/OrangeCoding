/**
 * Core types for the goal evaluation system.
 */

import type { TokenUsage } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

export const GoalStatus = {
  Active: "active",
  Completed: "completed",
  Failed: "failed",
  Paused: "paused",
} as const;

export type GoalStatus = (typeof GoalStatus)[keyof typeof GoalStatus];

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export interface GoalConfig {
  /** Unique identifier for the goal */
  id: string;
  /** Human-readable description of what the goal is about */
  description: string;
  /** The stop condition — natural language description of done state */
  condition: string;
  /** Maximum iterations before giving up (default: 50) */
  maxIterations?: number;
  /** Total token budget (default: unlimited) */
  maxTokens?: number;
  /** How often to re-evaluate in ms (default: 30_000) */
  intervalMs?: number;
  /** Model name override for the evaluator (cheaper model recommended) */
  evaluatorModel?: string;
}

export interface GoalEngineConfig {
  /** Default max iterations (default: 50) */
  defaultMaxIterations: number;
  /** Maximum concurrent goals (default: 3) */
  maxConcurrentGoals: number;
  /** Store for goal state persistence */
  storeDir: string;
}

export const DEFAULT_GOAL_ENGINE_CONFIG: Required<GoalEngineConfig> = {
  defaultMaxIterations: 50,
  maxConcurrentGoals: 3,
  storeDir: ".claude/goals",
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export interface GoalState {
  /** Unique ID */
  id: string;
  /** Goal configuration */
  config: GoalConfig;
  /** Current status */
  status: GoalStatus;
  /** Current iteration number */
  iteration: number;
  /** Tokens used so far */
  totalTokensUsed: number;
  /** Last evaluation result */
  lastEvalResult: EvaluationResult | null;
  /** Creation timestamp */
  createdAt: Date;
  /** Last update timestamp */
  updatedAt: Date;
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

export interface EvaluationResult {
  /** Whether the goal condition is met */
  completed: boolean;
  /** Confidence score 0-1 */
  confidence: number;
  /** Reason for this evaluation */
  reason: string;
  /** Specific blockers remaining */
  remainingBlockers: string[];
  /** Hints for what to try next */
  suggestions: string[];
  /** Token usage of this evaluation */
  tokenUsage?: TokenUsage;
}

export interface EvaluationContext {
  /** Current iteration number */
  iteration: number;
  /** Recent output from the last agent run */
  recentOutput: string;
  /** Test results if available */
  testResults?: string;
  /** Lint output if available */
  lintOutput?: string;
  /** Any additional context */
  additionalContext?: string;
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

export interface GoalResult {
  /** Goal ID */
  goalId: string;
  /** Whether the goal was achieved */
  success: boolean;
  /** Total iterations used */
  iterations: number;
  /** Tokens consumed */
  tokenUsage: TokenUsage;
  /** Duration in ms */
  durationMs: number;
  /** Final evaluation result */
  finalEval: EvaluationResult;
  /** Status at completion */
  finalStatus: GoalStatus;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export class GoalError extends Error {
  constructor(message: string, public readonly code: string) {
    super(message);
    this.name = "GoalError";
  }
}

export function newGoalError(code: string, message: string): GoalError {
  return new GoalError(message, code);
}
