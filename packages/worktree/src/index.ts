/**
 * @orangecoding/worktree — Git worktree isolation for parallel agent execution.
 *
 * Re-exports all public API from the package.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
export {
  WorktreeStatus,
  MergeStrategy,
  DEFAULT_WORKTREE_CONFIG,
  WorktreeError,
  newWorktreeError,
} from "./types.js";
export type { WorktreeInfo, WorktreeConfig, WorktreeStatus as WorktreeStatusType, MergeStrategy as MergeStrategyType } from "./types.js";

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------
export { WorktreeManager } from "./manager.js";

// ---------------------------------------------------------------------------
// Merger
// ---------------------------------------------------------------------------
export { mergeWorktree } from "./merger.js";
