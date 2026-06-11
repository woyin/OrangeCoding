/**
 * @orangecoding/orchestrator — Maker-checker orchestration and loop workflows.
 *
 * Re-exports all public API from the package.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
export {
  ReviewVerdict,
  DEFAULT_MAKER_CHECKER_CONFIG,
  OrchestratorError,
  newOrchestratorError,
} from "./types.js";
export type {
  ReviewVerdict as ReviewVerdictType,
  ReviewResult,
  MakerCheckerConfig,
  MakerCheckerContext,
  MakerCheckerResult,
  LoopWorkflowDefinition,
  WorkflowTrigger,
} from "./types.js";

// ---------------------------------------------------------------------------
// MakerChecker
// ---------------------------------------------------------------------------
export { MakerChecker } from "./maker-checker.js";

// ---------------------------------------------------------------------------
// Workflow
// ---------------------------------------------------------------------------
export { Orchestrator } from "./workflow.js";
