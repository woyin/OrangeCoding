/**
 * Core types for the worktree package.
 */

// ---------------------------------------------------------------------------
// Worktree Info
// ---------------------------------------------------------------------------

export interface WorktreeInfo {
  /** Absolute path to the worktree checkout */
  path: string;
  /** Branch name the worktree is on */
  branch: string;
  /** Creation timestamp */
  createdAt: Date;
  /** Last usage timestamp */
  lastUsedAt: Date;
  /** Current status */
  status: WorktreeStatus;
  /** Optional agent ID currently assigned */
  agentId?: string;
  /** Git HEAD commit SHA */
  headSha?: string;
}

export const WorktreeStatus = {
  Active: "active",
  Idle: "idle",
  Dirty: "dirty",
  Merged: "merged",
  Orphaned: "orphaned",
} as const;

export type WorktreeStatus = (typeof WorktreeStatus)[keyof typeof WorktreeStatus];

// ---------------------------------------------------------------------------
// Merge Strategy
// ---------------------------------------------------------------------------

export const MergeStrategy = {
  Auto: "auto",
  Manual: "manual",
  Squash: "squash",
} as const;

export type MergeStrategy = (typeof MergeStrategy)[keyof typeof MergeStrategy];

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export interface WorktreeConfig {
  /** Base directory under which worktrees are created (default: .claude/worktrees/) */
  baseDir: string;
  /** Git ref to branch from (default: HEAD or origin/main) */
  baseRef: string;
  /** Whether to auto-cleanup stale worktrees (default: true) */
  autoCleanup: boolean;
  /** Remove worktrees idle longer than this in ms (default: 24 hours) */
  cleanupAgeMs: number;
}

export const DEFAULT_WORKTREE_CONFIG: Required<WorktreeConfig> = {
  baseDir: ".claude/worktrees",
  baseRef: "HEAD",
  autoCleanup: true,
  cleanupAgeMs: 86_400_000, // 24 hours
};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export class WorktreeError extends Error {
  constructor(message: string, public readonly code: string, public readonly details?: unknown) {
    super(message);
    this.name = "WorktreeError";
  }
}

export function newWorktreeError(code: string, message: string, details?: unknown): WorktreeError {
  return new WorktreeError(message, code, details);
}
