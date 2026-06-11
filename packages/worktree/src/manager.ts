/**
 * WorktreeManager — manages git worktree lifecycle for parallel agent isolation.
 *
 * Worktrees allow multiple agents to work on the same repository simultaneously
 * without file collisions by creating separate working directories.
 */

import { execFile } from "node:child_process";
import { access, mkdir, readdir, rm, stat } from "node:fs/promises";
import { join } from "node:path";
import { promisify } from "node:util";
import { DEFAULT_WORKTREE_CONFIG, newWorktreeError, WorktreeStatus } from "./types.js";
import type { WorktreeConfig, WorktreeInfo, MergeStrategy } from "./types.js";
import { mergeWorktree } from "./merger.js";

const exec = promisify(execFile);

// ---------------------------------------------------------------------------
// Manager Class
// ---------------------------------------------------------------------------

export class WorktreeManager {
  private readonly _config: Required<WorktreeConfig>;
  private readonly _repoDir: string;
  private _worktrees: Map<string, WorktreeInfo> = new Map();
  private _initialized = false;

  /**
   * @param config - worktree configuration
   * @param repoDir - primary repository directory (default: process.cwd())
   */
  constructor(config?: Partial<WorktreeConfig>, repoDir?: string) {
    this._config = {
      baseDir: config?.baseDir ?? DEFAULT_WORKTREE_CONFIG.baseDir,
      baseRef: config?.baseRef ?? DEFAULT_WORKTREE_CONFIG.baseRef,
      autoCleanup: config?.autoCleanup ?? DEFAULT_WORKTREE_CONFIG.autoCleanup,
      cleanupAgeMs: config?.cleanupAgeMs ?? DEFAULT_WORKTREE_CONFIG.cleanupAgeMs,
    };
    this._repoDir = repoDir ?? process.cwd();
  }

  // -------------------------------------------------------------------------
  // Public Callbacks
  // -------------------------------------------------------------------------

  onCreated: ((info: WorktreeInfo) => void) | null = null;
  onRemoved: ((info: WorktreeInfo) => void) | null = null;
  onMerged: ((info: WorktreeInfo, success: boolean) => void) | null = null;

  // -------------------------------------------------------------------------
  // Lifecycle
  // -------------------------------------------------------------------------

  /**
   * Initialize the worktree manager: ensure base directory exists.
   */
  async init(): Promise<void> {
    if (this._initialized) return;
    await mkdir(this._config.baseDir, { recursive: true });
    this._initialized = true;
  }

  // -------------------------------------------------------------------------
  // Worktree Operations
  // -------------------------------------------------------------------------

  /**
   * Create a new git worktree on a named branch.
   *
   * @param name - worktree name (becomes the branch name)
   * @param branch - optional branch name override
   * @returns worktree info
   */
  async create(name: string, branch?: string): Promise<WorktreeInfo> {
    await this.init();

    const branchName = branch ?? name;
    const worktreePath = join(this._repoDir, this._config.baseDir, name);

    try {
      // Attempt to create a new branch and worktree
      await exec("git", ["worktree", "add", "-b", branchName, worktreePath, this._config.baseRef], {
        cwd: this._repoDir,
      });
    } catch (err) {
      // If branch already exists, try with existing branch
      try {
        await exec("git", ["worktree", "add", worktreePath, branchName], {
          cwd: this._repoDir,
        });
      } catch (err2) {
        throw newWorktreeError(
          "CREATE_FAILED",
          `failed to create worktree "${name}": ${(err as Error).message}; ${(err2 as Error).message}`
        );
      }
    }

    const info: WorktreeInfo = {
      path: worktreePath,
      branch: branchName,
      createdAt: new Date(),
      lastUsedAt: new Date(),
      status: WorktreeStatus.Active,
    };

    this._worktrees.set(name, info);
    this.onCreated?.(info);

    return info;
  }

  /**
   * Remove a worktree.
   */
  async remove(info: WorktreeInfo): Promise<void> {
    try {
      // Force clean-up to handle dirty worktrees
      await exec("git", ["worktree", "remove", "--force", info.path], { cwd: this._repoDir });
      await exec("git", ["branch", "-D", info.branch], { cwd: this._repoDir }).catch(() => {
        // Ignore branch deletion failure (branch may not exist)
      });
    } catch (err) {
      throw newWorktreeError(
        "REMOVE_FAILED",
        `failed to remove worktree "${info.branch}": ${(err as Error).message}`
      );
    }

    // Cleanup base dir entry
    for (const [name, wt] of this._worktrees) {
      if (wt.path === info.path) {
        this._worktrees.delete(name);
        break;
      }
    }

    this.onRemoved?.(info);
  }

  /**
   * List all known worktrees.
   */
  list(): WorktreeInfo[] {
    return [...this._worktrees.values()];
  }

  /**
   * Get a worktree by name.
   */
  get(name: string): WorktreeInfo | undefined {
    return this._worktrees.get(name);
  }

  /**
   * Merge a worktree back into the primary branch.s
   */
  async merge(info: WorktreeInfo, strategy: MergeStrategy): Promise<void> {
    try {
      await mergeWorktree(info, strategy, this._repoDir);
      info.status = WorktreeStatus.Merged;
      this.onMerged?.(info, true);
    } catch (err) {
      this.onMerged?.(info, false);
      throw err;
    }
  }

  // -------------------------------------------------------------------------
  // Cleanup
  // -------------------------------------------------------------------------

  /**
   * Clean up stale worktrees (idle longer than cleanupAgeMs).
   */
  async cleanup(signal?: AbortSignal): Promise<void> {
    const now = Date.now();
    const toRemove: WorktreeInfo[] = [];

    for (const info of this._worktrees.values()) {
      if (signal?.aborted) return;

      const age = now - info.lastUsedAt.getTime();
      if (age > this._config.cleanupAgeMs && info.status !== WorktreeStatus.Merged) {
        toRemove.push(info);
      }
    }

    for (const info of toRemove) {
      info.status = WorktreeStatus.Orphaned;
      await this.remove(info).catch(() => {
        // Ignore removal failures during cleanup
      });
    }

    await this.prune();
  }

  /**
   * Run `git worktree prune` to clean up orphaned worktree metadata.
   */
  async prune(): Promise<void> {
    try {
      await exec("git", ["worktree", "prune"], { cwd: this._repoDir });
    } catch {
      // Ignore prune failures
    }
  }

  // -------------------------------------------------------------------------
  // Status
  // -------------------------------------------------------------------------

  /**
   * Check whether a worktree is clean (no uncommitted changes).
   */
  async isClean(info: WorktreeInfo): Promise<boolean> {
    try {
      const { stdout } = await exec("git", ["status", "--porcelain"], { cwd: info.path });
      return stdout.trim().length === 0;
    } catch {
      return false;
    }
  }

  /**
   * Check whether a worktree has changes (uncommitted or untracked).
   */
  async hasChanges(info: WorktreeInfo): Promise<boolean> {
    return !(await this.isClean(info));
  }

  /**
   * Sync a worktree's git state from the primary repo.
   */
  async sync(info: WorktreeInfo): Promise<void> {
    try {
      await exec("git", ["fetch", "origin"], { cwd: info.path });
      await exec("git", ["rebase", `origin/${this._config.baseRef}`], { cwd: info.path }).catch(
        () => {
          // Rebase failure is non-fatal — may have conflicts
        }
      );
    } catch {
      // Sync failures are non-fatal
    }
  }
}
