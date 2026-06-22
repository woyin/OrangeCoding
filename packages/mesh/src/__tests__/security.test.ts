/**
 * Tests for the mesh security module — PermissionGuard and CommandApprovalGuard.
 */

import { PermissionGuard, CommandApprovalGuard } from "../security.js";

// ---------------------------------------------------------------------------
// PermissionGuard
// ---------------------------------------------------------------------------

describe("PermissionGuard", () => {
  it("allows tools that match the policy", () => {
    const guard = new PermissionGuard();
    guard.setPolicy("executor", ["bash", "read_file", "write_file"]);

    const [ok, reason] = guard.check("executor", "bash");
    expect(ok).toBe(true);
    expect(reason).toBe("");
  });

  it("rejects tools not in the policy", () => {
    const guard = new PermissionGuard();
    guard.setPolicy("executor", ["read_file"]);

    const [ok, reason] = guard.check("executor", "bash");
    expect(ok).toBe(false);
    expect(reason).toContain("not allowed");
  });

  it("rejects when no policy exists for the role", () => {
    const guard = new PermissionGuard();

    const [ok, reason] = guard.check("unknown_role", "bash");
    expect(ok).toBe(false);
    expect(reason).toContain("no policy");
  });

  it("supports multiple roles with different policies", () => {
    const guard = new PermissionGuard();
    guard.setPolicy("executor", ["bash"]);
    guard.setPolicy("reviewer", ["read_file", "grep"]);

    expect(guard.check("executor", "bash")[0]).toBe(true);
    expect(guard.check("executor", "grep")[0]).toBe(false);
    expect(guard.check("reviewer", "grep")[0]).toBe(true);
    expect(guard.check("reviewer", "bash")[0]).toBe(false);
  });

  it("validateToolCall always returns true (base implementation)", () => {
    const guard = new PermissionGuard();
    const [ok] = guard.validateToolCall("agent-1", "bash");
    expect(ok).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// CommandApprovalGuard
// ---------------------------------------------------------------------------

describe("CommandApprovalGuard", () => {
  it("allows safe commands", () => {
    const guard = new CommandApprovalGuard();
    const [ok] = guard.check("ls -la");
    expect(ok).toBe(true);
  });

  it("blocks 'rm -rf /'", () => {
    const guard = new CommandApprovalGuard();
    const [ok, reason] = guard.check("rm -rf /");
    expect(ok).toBe(false);
    expect(reason).toContain("blacklist");
  });

  it("blocks 'rm -rf /*'", () => {
    const guard = new CommandApprovalGuard();
    const [ok] = guard.check("rm -rf /*");
    expect(ok).toBe(false);
  });

  it("blocks mkfs commands", () => {
    const guard = new CommandApprovalGuard();
    const [ok] = guard.check("mkfs /dev/sda");
    expect(ok).toBe(false);
  });

  it("blocks dd if= commands", () => {
    const guard = new CommandApprovalGuard();
    const [ok] = guard.check("dd if=/dev/zero of=/dev/sda");
    expect(ok).toBe(false);
  });

  it("blocks fork bomb", () => {
    const guard = new CommandApprovalGuard();
    const [ok] = guard.check(":(){:|:&};:");
    expect(ok).toBe(false);
  });

  it("is case-insensitive", () => {
    const guard = new CommandApprovalGuard();
    const [ok] = guard.check("RM -RF /");
    expect(ok).toBe(false);
  });

  it("allows commands that contain blacklisted strings as substrings of paths", () => {
    const guard = new CommandApprovalGuard();
    // "dd if=" is blacklisted but "my_dd_if_config" is not
    const [ok] = guard.check("echo my_dd_if_config");
    expect(ok).toBe(true);
  });

  it("validateToolCall always returns true (base implementation)", () => {
    const guard = new CommandApprovalGuard();
    const [ok] = guard.validateToolCall("agent-1", "bash");
    expect(ok).toBe(true);
  });
});
