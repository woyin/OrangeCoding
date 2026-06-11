/**
 * MakerChecker — orchestrates a maker (implementer) and checker (reviewer) workflow.
 *
 * The maker produces work and the checker reviews it. If the checker finds issues,
 * the maker iterates. This enforces the "maker/checker separation" pattern that
 * prevents the model that wrote the code from grading its own homework.
 */

import { MakerCheckerConfig, MakerCheckerContext, MakerCheckerResult, ReviewVerdict, ReviewResult, DEFAULT_MAKER_CHECKER_CONFIG, newOrchestratorError } from "./types.js";
import { TokenUsage } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// MakerChecker Class
// ---------------------------------------------------------------------------

export class MakerChecker {
  private readonly _config: Required<Pick<MakerCheckerConfig, "maxReviewIterations" | "requireHumanApproval">>;
  private readonly _makerCfg: MakerCheckerConfig["maker"];
  private readonly _checkerCfg: MakerCheckerConfig["checker"];
  private readonly _tokenBudget: number | undefined;

  // -------------------------------------------------------------------------
  // Callbacks
  // -------------------------------------------------------------------------

  /** Called when a plan is created by the maker */
  onPlanCreated: ((plan: string) => void) | null = null;

  /** Called when a review completes */
  onReviewCompleted: ((result: ReviewResult) => void) | null = null;

  /** Called at the start of each iteration */
  onIterationStart: ((iteration: number) => void) | null = null;

  /** Called when human approval is required. Return true to proceed. */
  onHumanApprovalRequested: (() => Promise<boolean>) | null = null;

  // -------------------------------------------------------------------------
  // Constructor
  // -------------------------------------------------------------------------

  constructor(config: MakerCheckerConfig) {
    this._makerCfg = config.maker;
    this._checkerCfg = config.checker;
    this._config = {
      maxReviewIterations: config.maxReviewIterations ?? DEFAULT_MAKER_CHECKER_CONFIG.maxReviewIterations,
      requireHumanApproval: config.requireHumanApproval ?? DEFAULT_MAKER_CHECKER_CONFIG.requireHumanApproval,
    };
    this._tokenBudget = config.tokenBudget;
  }

  // -------------------------------------------------------------------------
  // Execution
  // -------------------------------------------------------------------------

  /**
   * Execute a task through the maker-checker workflow.
   *
   * @param task - the task description
   * @param context - optional execution context
   * @returns the workflow result
   */
  async execute(task: string, context?: MakerCheckerContext): Promise<MakerCheckerResult> {
    const startTime = Date.now();
    const checkerReports: ReviewResult[] = [];

    let makerOutput = "";
    let finalVerdict: ReviewVerdict = ReviewVerdict.ChangesNeeded;
    let iteration = 0;

    while (iteration < this._config.maxReviewIterations) {
      iteration++;
      this.onIterationStart?.(iteration);

      // Step 1: Maker produces output
      const iterationContext = iteration > 1
        ? `Previous review identified issues:\n${checkerReports[checkerReports.length - 1]?.issues.join("\n") ?? "No issues"}\n\nPlease address these.`
        : context?.additionalContext ?? "";

      makerOutput = await this._executeMaker(task, iterationContext, context);

      // Step 2: Human approval checkpoint
      if (this._config.requireHumanApproval && this.onHumanApprovalRequested) {
        const approved = await this.onHumanApprovalRequested();
        if (!approved) {
          finalVerdict = ReviewVerdict.Rejected;
          break;
        }
      }

      // Step 3: Checker reviews
      const review = await this._executeReview(task, makerOutput, context);
      checkerReports.push(review);
      this.onReviewCompleted?.(review);

      if (review.verdict === ReviewVerdict.Approved) {
        finalVerdict = ReviewVerdict.Approved;
        break;
      }

      // If rejected (not just changes needed), abort
      if (review.verdict === ReviewVerdict.Rejected) {
        finalVerdict = ReviewVerdict.Rejected;
        break;
      }
    }

    return {
      success: finalVerdict === ReviewVerdict.Approved,
      iterations: iteration,
      finalVerdict,
      makerOutput,
      checkerReports,
      tokenUsage: TokenUsage.create(0, 0),
      durationMs: Date.now() - startTime,
    };
  }

  // -------------------------------------------------------------------------
  // Private
  // -------------------------------------------------------------------------

  private async _executeMaker(
    task: string,
    iterationContext: string,
    context?: MakerCheckerContext
  ): Promise<string> {
    // In a real implementation, this would:
    // 1. Create an AgentLoop with the maker's provider/model/systemPrompt
    // 2. Run it with the task + context
    // 3. Return the output

    // For now, return a placeholder indicating where integration happens
    return `Maker execution placeholder for task: ${task}`;
  }

  private async _executeReview(
    task: string,
    output: string,
    context?: MakerCheckerContext
  ): Promise<ReviewResult> {
    // In a real implementation, this would:
    // 1. Create an AgentLoop with the checker's provider/model/systemPrompt
    // 2. Feed it the task + output
    // 3. Parse the structured review result

    // For now, return a default approved verdict
    return {
      verdict: ReviewVerdict.Approved,
      issues: [],
      suggestions: [],
      confidence: 1.0,
      tokenUsage: TokenUsage.create(0, 0),
    };
  }
}
