/**
 * Tests for the Permission Sandbox system.
 */

import { PermissionDecision } from "../permissions.js";
import { SandboxPermissionManager, strictSandbox, devSandbox } from "../sandbox.js";
import type { SandboxConfig } from "../sandbox.js";

describe("SandboxPermissionManager", () => {
  describe("basic operations", () => {
    it("uses default decision when no rules match", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [],
        defaultDecision: PermissionDecision.Deny,
      };
      const mgr = new SandboxPermissionManager(config);
      const result = mgr.check("read_file", {
        workingDir: "/project",
        filePath: "/project/src/index.ts",
        isReadOnly: true,
      });
      expect(result.decision).toBe(PermissionDecision.Deny);
    });

    it("matches read rules for read operations", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [
          {
            path: "src/**",
            action: "read",
            decision: PermissionDecision.Allow,
          },
        ],
        defaultDecision: PermissionDecision.Deny,
      };
      const mgr = new SandboxPermissionManager(config);
      const result = mgr.check("read_file", {
        workingDir: "/project",
        filePath: "/project/src/index.ts",
        isReadOnly: true,
      });
      expect(result.decision).toBe(PermissionDecision.Allow);
    });

    it("matches write rules for write operations", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [
          {
            path: "src/**",
            action: "write",
            decision: PermissionDecision.Allow,
          },
        ],
        defaultDecision: PermissionDecision.Deny,
      };
      const mgr = new SandboxPermissionManager(config);
      const result = mgr.check("write_file", {
        workingDir: "/project",
        filePath: "/project/src/index.ts",
        isReadOnly: false,
      });
      expect(result.decision).toBe(PermissionDecision.Allow);
    });

    it("read rules don't match write operations", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [
          {
            path: "src/**",
            action: "read",
            decision: PermissionDecision.Allow,
          },
        ],
        defaultDecision: PermissionDecision.Deny,
      };
      const mgr = new SandboxPermissionManager(config);
      const result = mgr.check("write_file", {
        workingDir: "/project",
        filePath: "/project/src/index.ts",
        isReadOnly: false,
      });
      expect(result.decision).toBe(PermissionDecision.Deny);
    });

    it("tool-specific rules work", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [
          {
            tool: "bash",
            action: "execute",
            decision: PermissionDecision.Ask,
          },
        ],
        defaultDecision: PermissionDecision.Allow,
      };
      const mgr = new SandboxPermissionManager(config);
      const result = mgr.check("bash", {
        workingDir: "/project",
        command: "ls",
        isReadOnly: false,
      });
      expect(result.decision).toBe(PermissionDecision.Ask);
    });
  });

  describe("network permissions", () => {
    it("allows whitelisted hosts", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [],
        defaultDecision: PermissionDecision.Deny,
        allowedHosts: ["github.com", "*.npmjs.org"],
      };
      const mgr = new SandboxPermissionManager(config);
      expect(mgr.checkNetwork("github.com").decision).toBe(PermissionDecision.Allow);
      expect(mgr.checkNetwork("registry.npmjs.org").decision).toBe(PermissionDecision.Allow);
    });

    it("denies non-whitelisted hosts", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [],
        defaultDecision: PermissionDecision.Deny,
        allowedHosts: ["github.com"],
      };
      const mgr = new SandboxPermissionManager(config);
      expect(mgr.checkNetwork("evil.com").decision).toBe(PermissionDecision.Deny);
    });

    it("wildcard allows all hosts", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [],
        defaultDecision: PermissionDecision.Deny,
        allowedHosts: ["*"],
      };
      const mgr = new SandboxPermissionManager(config);
      expect(mgr.checkNetwork("anything.com").decision).toBe(PermissionDecision.Allow);
    });
  });

  describe("environment variable permissions", () => {
    it("allows all env vars when no restrictions", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [],
        defaultDecision: PermissionDecision.Allow,
      };
      const mgr = new SandboxPermissionManager(config);
      expect(mgr.checkEnvVar("SECRET_KEY")).toBe(true);
    });

    it("restricts to allowed list", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [],
        defaultDecision: PermissionDecision.Allow,
        allowedEnvVars: ["PATH", "HOME"],
      };
      const mgr = new SandboxPermissionManager(config);
      expect(mgr.checkEnvVar("PATH")).toBe(true);
      expect(mgr.checkEnvVar("HOME")).toBe(true);
      expect(mgr.checkEnvVar("SECRET")).toBe(false);
    });
  });

  describe("rule ordering (first match wins)", () => {
    it("first matching rule takes precedence", () => {
      const config: SandboxConfig = {
        workingDir: "/project",
        rules: [
          {
            path: "src/secret.ts",
            action: "read",
            decision: PermissionDecision.Deny,
            reason: "secret file",
          },
          {
            path: "src/**",
            action: "read",
            decision: PermissionDecision.Allow,
          },
        ],
        defaultDecision: PermissionDecision.Deny,
      };
      const mgr = new SandboxPermissionManager(config);
      const result = mgr.check("read_file", {
        workingDir: "/project",
        filePath: "/project/src/secret.ts",
        isReadOnly: true,
      });
      expect(result.decision).toBe(PermissionDecision.Deny);
      expect(result.reason).toContain("secret file");
    });
  });
});

describe("preset configurations", () => {
  it("strictSandbox creates a valid config", () => {
    const config = strictSandbox("/project");
    expect(config.workingDir).toBe("/project");
    expect(config.defaultDecision).toBe(PermissionDecision.Ask);
    expect(config.rules.length).toBeGreaterThan(0);
    expect(config.allowedHosts).toBeDefined();
  });

  it("devSandbox creates a valid config", () => {
    const config = devSandbox("/project");
    expect(config.workingDir).toBe("/project");
    expect(config.defaultDecision).toBe(PermissionDecision.Allow);
    expect(config.allowedHosts).toEqual(["*"]);
  });

  it("strict sandbox allows reading project files", () => {
    const config = strictSandbox("/project");
    const mgr = new SandboxPermissionManager(config);
    const result = mgr.check("read_file", {
      workingDir: "/project",
      filePath: "/project/src/app.ts",
      isReadOnly: true,
    });
    expect(result.decision).toBe(PermissionDecision.Allow);
  });

  it("strict sandbox allows writing to src", () => {
    const config = strictSandbox("/project");
    const mgr = new SandboxPermissionManager(config);
    const result = mgr.check("write_file", {
      workingDir: "/project",
      filePath: "/project/src/app.ts",
      isReadOnly: false,
    });
    expect(result.decision).toBe(PermissionDecision.Allow);
  });

  it("strict sandbox denies system paths", () => {
    const config = strictSandbox("/project");
    const mgr = new SandboxPermissionManager(config);
    const result = mgr.check("write_file", {
      workingDir: "/project",
      filePath: "/etc/passwd",
      isReadOnly: false,
    });
    expect(result.decision).toBe(PermissionDecision.Deny);
  });
});
