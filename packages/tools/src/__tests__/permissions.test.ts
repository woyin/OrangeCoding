/**
 * Tests for the permission types module.
 */

import { PermissionDecision } from "../permissions.js";
import type { PermissionContext } from "../permissions.js";

describe("PermissionDecision", () => {
  it("has all expected enum values", () => {
    expect(PermissionDecision.Allow).toBe(0);
    expect(PermissionDecision.Deny).toBe(1);
    expect(PermissionDecision.Ask).toBe(2);
    expect(PermissionDecision.AutoApprove).toBe(3);
    expect(PermissionDecision.Conditional).toBe(4);
  });

  it("values are distinct", () => {
    const values = [
      PermissionDecision.Allow,
      PermissionDecision.Deny,
      PermissionDecision.Ask,
      PermissionDecision.AutoApprove,
      PermissionDecision.Conditional,
    ];
    expect(new Set(values).size).toBe(5);
  });
});

describe("PermissionContext", () => {
  it("can be constructed with required fields", () => {
    const ctx: PermissionContext = {
      workingDir: "/workspace",
      isReadOnly: true,
    };
    expect(ctx.workingDir).toBe("/workspace");
    expect(ctx.isReadOnly).toBe(true);
    expect(ctx.filePath).toBeUndefined();
    expect(ctx.command).toBeUndefined();
  });

  it("supports optional filePath and command", () => {
    const ctx: PermissionContext = {
      workingDir: "/workspace",
      isReadOnly: false,
      filePath: "/workspace/src/main.ts",
      command: "rm -rf /tmp/build",
    };
    expect(ctx.filePath).toBe("/workspace/src/main.ts");
    expect(ctx.command).toBe("rm -rf /tmp/build");
  });
});
