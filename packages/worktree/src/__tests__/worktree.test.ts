/**
 * Tests for the worktree package — types, constants, and error constructors.
 * The WorktreeManager is git-dependent so we focus on pure logic.
 */

import {
  WorktreeStatus,
  MergeStrategy,
  DEFAULT_WORKTREE_CONFIG,
  WorktreeError,
  newWorktreeError,
} from "../types.js";
import type { WorktreeInfo, WorktreeConfig } from "../types.js";

// ---------------------------------------------------------------------------
// WorktreeStatus constants
// ---------------------------------------------------------------------------

describe("WorktreeStatus", () => {
  it("has all expected status values", () => {
    expect(WorktreeStatus.Active).toBe("active");
    expect(WorktreeStatus.Idle).toBe("idle");
    expect(WorktreeStatus.Dirty).toBe("dirty");
    expect(WorktreeStatus.Merged).toBe("merged");
    expect(WorktreeStatus.Orphaned).toBe("orphaned");
  });
});

// ---------------------------------------------------------------------------
// MergeStrategy constants
// ---------------------------------------------------------------------------

describe("MergeStrategy", () => {
  it("has all expected strategy values", () => {
    expect(MergeStrategy.Auto).toBe("auto");
    expect(MergeStrategy.Manual).toBe("manual");
    expect(MergeStrategy.Squash).toBe("squash");
  });
});

// ---------------------------------------------------------------------------
// DEFAULT_WORKTREE_CONFIG
// ---------------------------------------------------------------------------

describe("DEFAULT_WORKTREE_CONFIG", () => {
  it("has sensible defaults", () => {
    expect(DEFAULT_WORKTREE_CONFIG.baseDir).toBe(".claude/worktrees");
    expect(DEFAULT_WORKTREE_CONFIG.baseRef).toBe("HEAD");
    expect(DEFAULT_WORKTREE_CONFIG.autoCleanup).toBe(true);
    expect(DEFAULT_WORKTREE_CONFIG.cleanupAgeMs).toBe(86_400_000); // 24 hours
  });
});

// ---------------------------------------------------------------------------
// WorktreeError
// ---------------------------------------------------------------------------

describe("WorktreeError", () => {
  it("creates an error with code and details", () => {
    const err = newWorktreeError("CREATE_FAILED", "cannot create worktree", { branch: "test" });
    expect(err).toBeInstanceOf(WorktreeError);
    expect(err).toBeInstanceOf(Error);
    expect(err.code).toBe("CREATE_FAILED");
    expect(err.message).toBe("cannot create worktree");
    expect(err.details).toEqual({ branch: "test" });
    expect(err.name).toBe("WorktreeError");
  });

  it("creates an error without details", () => {
    const err = newWorktreeError("MERGE_FAILED", "merge conflict");
    expect(err.details).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// WorktreeInfo type shape
// ---------------------------------------------------------------------------

describe("WorktreeInfo", () => {
  it("can be constructed with required fields", () => {
    const info: WorktreeInfo = {
      path: "/repo/.claude/worktrees/feature-1",
      branch: "feature-1",
      createdAt: new Date(),
      lastUsedAt: new Date(),
      status: WorktreeStatus.Active,
    };

    expect(info.path).toBe("/repo/.claude/worktrees/feature-1");
    expect(info.branch).toBe("feature-1");
    expect(info.status).toBe(WorktreeStatus.Active);
  });

  it("allows optional fields", () => {
    const info: WorktreeInfo = {
      path: "/repo/.claude/worktrees/feature-2",
      branch: "feature-2",
      createdAt: new Date(),
      lastUsedAt: new Date(),
      status: WorktreeStatus.Active,
      agentId: "agent-123",
      headSha: "abc123def456",
    };

    expect(info.agentId).toBe("agent-123");
    expect(info.headSha).toBe("abc123def456");
  });
});

// ---------------------------------------------------------------------------
// WorktreeConfig type shape
// ---------------------------------------------------------------------------

describe("WorktreeConfig", () => {
  it("can be constructed with all fields", () => {
    const cfg: WorktreeConfig = {
      baseDir: "/custom/dir",
      baseRef: "origin/main",
      autoCleanup: false,
      cleanupAgeMs: 3600_000,
    };

    expect(cfg.baseDir).toBe("/custom/dir");
    expect(cfg.baseRef).toBe("origin/main");
    expect(cfg.autoCleanup).toBe(false);
    expect(cfg.cleanupAgeMs).toBe(3600_000);
  });
});
