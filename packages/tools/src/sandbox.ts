/**
 * Permission Sandbox — fine-grained, pattern-based permission control.
 *
 * Inspired by Claude Code's permission system, this provides:
 * - Path-pattern-based file access control (glob patterns)
 * - Per-tool permission rules
 * - Read/write/execute permission granularity
 * - Allow/deny/ask decision hierarchy
 * - Permission inheritance and precedence
 */

import { resolve, sep, relative } from "node:path";
import { PermissionDecision } from "./permissions.js";
import type { PermissionContext } from "./permissions.js";

// ---------------------------------------------------------------------------
// Permission Rule
// ---------------------------------------------------------------------------

export type PermissionAction = "read" | "write" | "execute" | "network" | "all";

export interface PermissionRule {
  /** Glob pattern for file paths (e.g., "src/**", "*.ts", "/tmp/**") */
  path?: string;
  /** Tool name or pattern (e.g., "bash", "write_file", "fetch") */
  tool?: string;
  /** What action this rule governs */
  action: PermissionAction;
  /** The decision to make */
  decision: PermissionDecision;
  /** Optional reason for this rule (shown to user) */
  reason?: string;
}

// ---------------------------------------------------------------------------
// SandboxConfig
// ---------------------------------------------------------------------------

export interface SandboxConfig {
  /** Working directory for resolving relative paths */
  workingDir: string;
  /** Ordered list of permission rules (first match wins) */
  rules: PermissionRule[];
  /** Default decision when no rule matches */
  defaultDecision: PermissionDecision;
  /** Allowed network hosts (empty = no network unless rule allows) */
  allowedHosts?: string[];
  /** Allowed environment variable names */
  allowedEnvVars?: string[];
}

// ---------------------------------------------------------------------------
// SandboxPermissionManager
// ---------------------------------------------------------------------------

/**
 * Manages fine-grained permissions for tool execution.
 *
 * Rules are evaluated in order; the first matching rule determines the decision.
 * If no rule matches, the default decision is used.
 */
export class SandboxPermissionManager {
  private readonly _config: SandboxConfig;
  private readonly _compiledRules: CompiledRule[];

  constructor(config: SandboxConfig) {
    this._config = config;
    this._compiledRules = config.rules.map((r) => compileRule(r, config.workingDir));
  }

  /**
   * Check permissions for a tool operation.
   */
  check(toolName: string, ctx: PermissionContext): PermissionCheckResult {
    const action = ctx.isReadOnly ? "read" : "write";

    // Check file-based permissions
    if (ctx.filePath) {
      const absPath = resolve(this._config.workingDir, ctx.filePath);
      const fileResult = this._matchRules(toolName, absPath, action);
      if (fileResult.matchedRule) {
        return fileResult;  // A specific rule matched, use its decision
      }
    }

    // Check command-based permissions
    if (ctx.command && toolName === "bash") {
      const cmdResult = this._matchCommandRules(toolName, ctx.command, "execute");
      if (cmdResult.matchedRule) {
        return cmdResult;  // A specific rule matched, use its decision
      }
    }

    // Check tool-level permissions
    const toolResult = this._matchToolRules(toolName, action);
    if (toolResult) {
      return toolResult;
    }

    // Default
    return {
      decision: this._config.defaultDecision,
      matchedRule: null,
      reason: "no matching rule; using default decision",
    };
  }

  /**
   * Check network access permissions.
   */
  checkNetwork(host: string): PermissionCheckResult {
    // Check allowed hosts list
    if (this._config.allowedHosts) {
      for (const allowed of this._config.allowedHosts) {
        if (matchHost(host, allowed)) {
          return {
            decision: PermissionDecision.Allow,
            matchedRule: null,
            reason: `host "${host}" matches allowed pattern "${allowed}"`,
          };
        }
      }
    }

    // Check rules for network action
    for (const rule of this._compiledRules) {
      if (rule.action === "network" || rule.action === "all") {
        if (rule.toolPattern && matchGlob("fetch", rule.toolPattern)) {
          return {
            decision: rule.decision,
            matchedRule: rule.original,
            reason: rule.original.reason ?? `rule matched for network access`,
          };
        }
      }
    }

    return {
      decision: this._config.defaultDecision,
      matchedRule: null,
      reason: `no network rule matched for host "${host}"`,
    };
  }

  /**
   * Check if an environment variable is accessible.
   */
  checkEnvVar(name: string): boolean {
    if (!this._config.allowedEnvVars) {
      return true; // No restrictions
    }
    return this._config.allowedEnvVars.includes(name);
  }

  /**
   * Get the current sandbox configuration.
   */
  getConfig(): Readonly<SandboxConfig> {
    return this._config;
  }

  // Private methods

  private _matchRules(
    toolName: string,
    absPath: string,
    action: PermissionAction,
  ): PermissionCheckResult {
    for (const rule of this._compiledRules) {
      // Check action match
      if (rule.action !== "all" && rule.action !== action) continue;

      // Check path match
      if (rule.pathPattern && !matchPathPattern(absPath, rule.pathPattern, this._config.workingDir)) {
        continue;
      }

      // Check tool match
      if (rule.toolPattern && !matchGlob(toolName, rule.toolPattern)) {
        continue;
      }

      // Must have at least one criterion to match
      if (!rule.pathPattern && !rule.toolPattern) continue;

      return {
        decision: rule.decision,
        matchedRule: rule.original,
        reason: rule.original.reason ?? `matched rule for ${action} on ${absPath}`,
      };
    }

    return {
      decision: this._config.defaultDecision,
      matchedRule: null,
      reason: "no file rule matched",
    };
  }

  private _matchCommandRules(
    toolName: string,
    _command: string,
    action: PermissionAction,
  ): PermissionCheckResult {
    for (const rule of this._compiledRules) {
      if (rule.action !== "all" && rule.action !== action && rule.action !== "execute") continue;
      if (rule.toolPattern && !matchGlob(toolName, rule.toolPattern)) continue;
      if (!rule.toolPattern) continue;

      return {
        decision: rule.decision,
        matchedRule: rule.original,
        reason: rule.original.reason ?? `matched execute rule for ${toolName}`,
      };
    }

    return {
      decision: this._config.defaultDecision,
      matchedRule: null,
      reason: "no command rule matched",
    };
  }

  private _matchToolRules(
    toolName: string,
    action: PermissionAction,
  ): PermissionCheckResult | null {
    for (const rule of this._compiledRules) {
      if (rule.action !== "all" && rule.action !== action) continue;
      if (rule.toolPattern && matchGlob(toolName, rule.toolPattern) && !rule.pathPattern) {
        return {
          decision: rule.decision,
          matchedRule: rule.original,
          reason: rule.original.reason ?? `matched tool rule for ${toolName}`,
        };
      }
    }
    return null;
  }
}

// ---------------------------------------------------------------------------
// PermissionCheckResult
// ---------------------------------------------------------------------------

export interface PermissionCheckResult {
  decision: PermissionDecision;
  matchedRule: PermissionRule | null;
  reason: string;
}

// ---------------------------------------------------------------------------
// Compiled Rule (internal)
// ---------------------------------------------------------------------------

interface CompiledRule {
  original: PermissionRule;
  pathPattern: string | null;
  toolPattern: string | null;
  action: PermissionAction;
  decision: PermissionDecision;
}

function compileRule(rule: PermissionRule, workingDir: string): CompiledRule {
  let pathPattern: string | null = null;
  if (rule.path) {
    // Resolve relative patterns against working dir
    if (rule.path.startsWith("/") || rule.path.startsWith("*")) {
      pathPattern = rule.path;
    } else {
      pathPattern = resolve(workingDir, rule.path);
    }
  }

  return {
    original: rule,
    pathPattern,
    toolPattern: rule.tool ?? null,
    action: rule.action,
    decision: rule.decision,
  };
}

// ---------------------------------------------------------------------------
// Pattern Matching
// ---------------------------------------------------------------------------

/**
 * Match a file path against a glob-like pattern.
 * Supports: *, **, ?, and path separators.
 */
function matchPathPattern(absPath: string, pattern: string, workingDir: string): boolean {
  // Normalize
  const normalizedPath = absPath.replace(/\\/g, "/");
  let normalizedPattern = pattern.replace(/\\/g, "/");

  // If pattern is a directory, match everything inside
  if (!normalizedPattern.includes("*") && !normalizedPattern.includes("?")) {
    // Exact path or directory prefix match
    const resolvedPattern = resolve(workingDir, normalizedPattern).replace(/\\/g, "/");
    return normalizedPath === resolvedPattern ||
           normalizedPath.startsWith(resolvedPattern + "/");
  }

  return matchGlob(normalizedPath, normalizedPattern);
}

/**
 * Simple glob matching supporting *, **, and ?.
 */
function matchGlob(str: string, pattern: string): boolean {
  // Convert glob to regex
  const regexStr = globToRegex(pattern);
  const regex = new RegExp(`^${regexStr}$`);
  return regex.test(str);
}

function globToRegex(pattern: string): string {
  let result = "";
  let i = 0;
  while (i < pattern.length) {
    const ch = pattern[i];
    if (ch === "*") {
      if (i + 1 < pattern.length && pattern[i + 1] === "*") {
        // ** matches everything including /
        if (i + 2 < pattern.length && pattern[i + 2] === "/") {
          result += "(?:.*/)?";
          i += 3;
        } else {
          result += ".*";
          i += 2;
        }
      } else {
        // * matches everything except /
        result += "[^/]*";
        i++;
      }
    } else if (ch === "?") {
      result += "[^/]";
      i++;
    } else if (ch === ".") {
      result += "\\.";
      i++;
    } else if (ch === "[") {
      // Pass through character classes
      let j = i + 1;
      while (j < pattern.length && pattern[j] !== "]") j++;
      result += pattern.slice(i, j + 1);
      i = j + 1;
    } else {
      result += ch;
      i++;
    }
  }
  return result;
}

/**
 * Match a hostname against a pattern (supports wildcards like *.example.com).
 */
function matchHost(host: string, pattern: string): boolean {
  if (pattern === "*") return true;
  if (pattern.startsWith("*.")) {
    const suffix = pattern.slice(1); // ".example.com"
    return host.endsWith(suffix) || host === pattern.slice(2);
  }
  return host === pattern;
}

// ---------------------------------------------------------------------------
// Preset Configurations
// ---------------------------------------------------------------------------

/**
 * Creates a restrictive sandbox that only allows access to the working directory.
 */
export function strictSandbox(workingDir: string): SandboxConfig {
  return {
    workingDir,
    rules: [
      // Allow reading anything in the project
      {
        path: "**",
        action: "read",
        decision: PermissionDecision.Allow,
        reason: "read access to project files",
      },
      // Allow writing only to src and tests
      {
        path: "src/**",
        action: "write",
        decision: PermissionDecision.Allow,
        reason: "write access to source files",
      },
      {
        path: "tests/**",
        action: "write",
        decision: PermissionDecision.Allow,
        reason: "write access to test files",
      },
      {
        path: "__tests__/**",
        action: "write",
        decision: PermissionDecision.Allow,
        reason: "write access to test files",
      },
      // Deny writing to config files
      {
        path: "*.json",
        action: "write",
        decision: PermissionDecision.Ask,
        reason: "ask before modifying config files",
      },
      // Deny dangerous paths
      {
        path: "/etc/**",
        action: "all",
        decision: PermissionDecision.Deny,
        reason: "system files are off-limits",
      },
      {
        path: "/usr/**",
        action: "all",
        decision: PermissionDecision.Deny,
        reason: "system files are off-limits",
      },
    ],
    defaultDecision: PermissionDecision.Ask,
    allowedHosts: ["*.npmjs.org", "registry.npmjs.org", "github.com", "*.github.com"],
  };
}

/**
 * Creates a permissive sandbox for development use.
 */
export function devSandbox(workingDir: string): SandboxConfig {
  return {
    workingDir,
    rules: [
      // Deny system paths
      {
        path: "/etc/**",
        action: "all",
        decision: PermissionDecision.Deny,
        reason: "system files are off-limits",
      },
      {
        path: "/usr/**",
        action: "all",
        decision: PermissionDecision.Deny,
        reason: "system files are off-limits",
      },
    ],
    defaultDecision: PermissionDecision.Allow,
    allowedHosts: ["*"],
  };
}
