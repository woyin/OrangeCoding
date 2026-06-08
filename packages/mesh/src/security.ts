/**
 * @module security
 * Security guards for tool validation and command approval.
 */

import type { AgentId, AgentRole } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// SecurityGuard
// ---------------------------------------------------------------------------

/** Validates operations for security. */
export interface SecurityGuard {
  validateToolCall(agentID: AgentId, toolName: string): [boolean, string];
}

// ---------------------------------------------------------------------------
// PermissionGuard
// ---------------------------------------------------------------------------

/** Controls which tools each role can use. */
export class PermissionGuard implements SecurityGuard {
  private policies = new Map<string, Set<string>>();

  /** SetPolicy defines allowed tools for a role. */
  setPolicy(role: AgentRole, tools: string[]): void {
    this.policies.set(role, new Set(tools));
  }

  /** Check verifies if a tool is allowed for a role. Returns [ok, reason]. */
  check(role: AgentRole, toolName: string): [boolean, string] {
    const allowed = this.policies.get(role);
    if (!allowed) {
      return [false, `no policy for role ${role}`];
    }
    if (!allowed.has(toolName)) {
      return [false, `tool ${toolName} not allowed for role ${role}`];
    }
    return [true, ""];
  }

  /** ValidateToolCall implements SecurityGuard. */
  validateToolCall(_agentID: AgentId, _toolName: string): [boolean, string] {
    return [true, ""];
  }
}

// ---------------------------------------------------------------------------
// Approver
// ---------------------------------------------------------------------------

/** Decides whether to approve a pending command. */
export interface Approver {
  approve(command: string): Promise<boolean>;
}

// ---------------------------------------------------------------------------
// CommandApprovalGuard
// ---------------------------------------------------------------------------

/** Filters bash commands for safety. */
export class CommandApprovalGuard implements SecurityGuard {
  private blacklist: string[];
  private approver?: Approver;

  constructor(approver?: Approver) {
    this.approver = approver;
    this.blacklist = [
      "rm -rf /",
      "rm -rf /*",
      "mkfs",
      "dd if=",
      ":(){:|:&};:",
    ];
  }

  /** Check verifies if a command is safe to execute. Returns [ok, reason]. */
  check(command: string): [boolean, string] {
    const lower = command.toLowerCase();
    for (const pattern of this.blacklist) {
      if (lower.includes(pattern)) {
        return [false, "command matches blacklist: " + pattern];
      }
    }
    return [true, ""];
  }

  /** ValidateToolCall implements SecurityGuard. */
  validateToolCall(_agentID: AgentId, _toolName: string): [boolean, string] {
    return [true, ""];
  }
}
