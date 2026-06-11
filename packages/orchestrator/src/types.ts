/**
 * Core types for the orchestrator package.
 *
 * Provides Maker-Checker orchestration and loop workflow abstractions.
 */

import type { TokenUsage } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// Review Types
// ---------------------------------------------------------------------------

export const ReviewVerdict = {
  Approved: "approved",
  ChangesNeeded: "changes_needed",
  Rejected: "rejected",
} as const;

export type ReviewVerdict = (typeof ReviewVerdict)[keyof typeof ReviewVerdict];

export interface ReviewResult {
  /** Verdict from the review */
  verdict: ReviewVerdict;
  /** Specific issues found */
  issues: string[];
  /** Suggestions for improvement */
  suggestions: string[];
  /** Confidence score 0-1 */
  confidence: number;
  /** Token usage */
  tokenUsage: TokenUsage;
}

// ---------------------------------------------------------------------------
// Maker-Checker Types
// ---------------------------------------------------------------------------

export interface MakerCheckerConfig {
  /** Maker (implementer) agent configuration */
  maker: {
    provider: unknown; // AiProvider
    model?: string;
    systemPrompt?: string;
  };
  /** Checker (reviewer) agent configuration */
  checker: {
    provider: unknown; // AiProvider
    model?: string;
    systemPrompt?: string;
    /** If true, use a different model from the maker (recommended) */
    differentModel?: boolean;
  };
  /** Optional separate executor */
  executor?: {
    provider: unknown; // AiProvider
    model?: string;
  };
  /** Maximum plan-review iterations (default: 3) */
  maxReviewIterations?: number;
  /** Require human approval before implementation (default: false) */
  requireHumanApproval?: boolean;
  /** Token budget for the entire workflow (default: unlimited) */
  tokenBudget?: number;
}

export interface MakerCheckerContext {
  /** Optional worktree for isolation */
  worktreePath?: string;
  /** Additional context for the maker */
  additionalContext?: string;
  /** Existing code to work on */
  existingCode?: string;
  /** Constraints for the implementation */
  constraints?: string[];
}

export interface MakerCheckerResult {
  /** Whether the workflow succeeded */
  success: boolean;
  /** Number of iterations used */
  iterations: number;
  /** Final review verdict */
  finalVerdict: ReviewVerdict;
  /** Output from the maker agent */
  makerOutput: string;
  /** All review reports from each iteration */
  checkerReports: ReviewResult[];
  /** Token usage summary */
  tokenUsage: TokenUsage;
  /** Total duration in ms */
  durationMs: number;
}

export const DEFAULT_MAKER_CHECKER_CONFIG: Required<Pick<MakerCheckerConfig, "maxReviewIterations" | "requireHumanApproval">> = {
  maxReviewIterations: 3,
  requireHumanApproval: false,
};

// ---------------------------------------------------------------------------
// Loop Workflow Types
// ---------------------------------------------------------------------------

export interface LoopWorkflowDefinition {
  /** Workflow name */
  name: string;
  /** Human-readable description */
  description: string;
  /** The trigger definition */
  trigger: WorkflowTrigger;
  /** Workflow task/prompt */
  task: string;
}

export type WorkflowTrigger =
  | { type: "cron"; cron: string }
  | { type: "manual"; task: string }
  | { type: "event"; event: string };

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export class OrchestratorError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly detail?: unknown
  ) {
    super(message);
    this.name = "OrchestratorError";
  }
}

export function newOrchestratorError(code: string, message: string, detail?: unknown): OrchestratorError {
  return new OrchestratorError(message, code, detail);
}
