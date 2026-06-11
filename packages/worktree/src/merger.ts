/**
 * Merge strategies for worktree branches.
 */

import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { WorktreeInfo, MergeStrategy } from "./types.js";
import { newWorktreeError } from "./types.js";

const exec = promisify(execFile);

/**
 * Merge a worktree branch back into the primary branch.
 *
 * @param info - worktree to merge from
 * @param strategy - merge strategy to use
 * @param repoDir - the primary repo directory
 */
export async function mergeWorktree(
  info: WorktreeInfo,
  strategy: MergeStrategy,
  repoDir: string
): Promise<void> {
  switch (strategy) {
    case "auto":
      await _mergeAuto(info, repoDir);
      break;
    case "squash":
      await _mergeSquash(info, repoDir);
      break;
    case "manual":
      // No-op — leave for the user
      break;
  }
}

async function _mergeAuto(info: WorktreeInfo, repoDir: string): Promise<void> {
  try {
    // Checkout primary and merge
    await exec("git", ["checkout", info.branch || "main"], { cwd: repoDir });
    await exec("git", ["merge", "--no-edit", `worktree/${info.branch}`], { cwd: repoDir });
  } catch (err) {
    throw newWorktreeError("MERGE_FAILED", `failed to auto-merge worktree: ${(err as Error).message}`);
  }
}

async function _mergeSquash(info: WorktreeInfo, repoDir: string): Promise<void> {
  try {
    const targetBranch = info.branch || "main";
    await exec("git", ["checkout", targetBranch], { cwd: repoDir });
    await exec("git", ["merge", "--squash", `worktree/${info.branch}`], { cwd: repoDir });
  } catch (err) {
    throw newWorktreeError("SQUASH_FAILED", `failed to squash-merge worktree: ${(err as Error).message}`);
  }
}
