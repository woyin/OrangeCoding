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
   * Checks permissions for a tool operation. Evaluates rules in order: file
   * path rules first (if ctx.filePath is set), then command rules (for bash),
   * then tool-level rules. First match wins; falls back to defaultDecision.
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
   * Checks whether network access to `host` is allowed. Consults the
   * allowedHosts list first (exact or wildcard match), then network-action
   * rules, then the default decision.
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

    // Check rules for network action (matched by tool name, e.g. "fetch").
    for (const rule of this._compiledRules) {
      if (rule.action === "network" || rule.action === "all") {
        if (rule.toolRegex && rule.toolRegex.test("fetch")) {
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
   * Returns true if the environment variable `name` is accessible. When
   * allowedEnvVars is unset, all variables are accessible.
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

  /**
   * Walks compiled rules in order (first match wins), checking action + path +
   * tool. Path and tool matching use the pre-compiled regexes (or the literal
   * startsWith fast path), so this loop allocates nothing per iteration.
   */
  private _matchRules(
    toolName: string,
    absPath: string,
    action: PermissionAction,
  ): PermissionCheckResult {
    for (const rule of this._compiledRules) {
      // Action must match (or be the wildcard "all").
      if (rule.action !== "all" && rule.action !== action) continue;

      // Path criterion: if the rule has a path, the path must match.
      if (rule.pathPattern && !matchPathCompiled(absPath, rule)) continue;

      // Tool criterion: if the rule names a tool, the tool name must match.
      if (rule.toolRegex && !rule.toolRegex.test(toolName)) continue;

      // Must have at least one criterion to match (avoid catch-all empty rules).
      if (!rule.pathPattern && !rule.toolRegex) continue;

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

  /** Matches execute-action rules by tool name (for bash command checks). */
  private _matchCommandRules(
    toolName: string,
    _command: string,
    action: PermissionAction,
  ): PermissionCheckResult {
    for (const rule of this._compiledRules) {
      if (rule.action !== "all" && rule.action !== action && rule.action !== "execute") continue;
      if (rule.toolRegex && !rule.toolRegex.test(toolName)) continue;
      if (!rule.toolRegex) continue;

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

  /** Matches tool-level rules (tool-only, no path constraint). */
  private _matchToolRules(
    toolName: string,
    action: PermissionAction,
  ): PermissionCheckResult | null {
    for (const rule of this._compiledRules) {
      if (rule.action !== "all" && rule.action !== action) continue;
      if (rule.toolRegex && rule.toolRegex.test(toolName) && !rule.pathPattern) {
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

/**
 * A PermissionRule with its glob patterns pre-compiled to RegExp at construction
 * time. Precompiling avoids re-running globToRegex() + new RegExp() on every
 * permission check (the hot path in tool execution); each check becomes a pair
 * of regex.test() calls with zero allocation.
 *
 * `isPathLiteral` marks directory-prefix patterns (no glob metachars) which we
 * match with a fast startsWith rather than the regex engine.
 */
interface CompiledRule {
  original: PermissionRule;
  pathPattern: string | null;
  /** Pre-compiled regex for path matching, or null when path is a literal/dir prefix. */
  pathRegex: RegExp | null;
  /** True when pathPattern has no glob chars — matched via string compare. */
  isPathLiteral: boolean;
  /** Pre-compiled regex for tool-name matching, or null. */
  toolRegex: RegExp | null;
  action: PermissionAction;
  decision: PermissionDecision;
}

/**
 * Compiles a PermissionRule into a CompiledRule with pre-compiled regexes.
 * Path patterns are resolved against workingDir first. Literal (non-glob) path
 * patterns are flagged so matchPathPattern can use a fast startsWith check.
 */
function compileRule(rule: PermissionRule, workingDir: string): CompiledRule {
  let pathPattern: string | null = null;
  let pathRegex: RegExp | null = null;
  let isPathLiteral = false;

  if (rule.path) {
    // Resolve relative patterns against working dir; keep absolute/glob patterns as-is.
    if (rule.path.startsWith("/") || rule.path.startsWith("*")) {
      pathPattern = rule.path.replace(/\\/g, "/");
    } else {
      pathPattern = resolve(workingDir, rule.path).replace(/\\/g, "/");
    }

    if (pathPattern.includes("*") || pathPattern.includes("?")) {
      // Glob pattern — compile once, reuse across all future checks.
      pathRegex = new RegExp(`^${globToRegex(pathPattern)}$`);
    } else {
      // Literal directory/file — match with startsWith, no regex needed.
      isPathLiteral = true;
    }
  }

  let toolRegex: RegExp | null = null;
  if (rule.tool) {
    toolRegex = new RegExp(`^${globToRegex(rule.tool)}$`);
  }

  return {
    original: rule,
    pathPattern,
    pathRegex,
    isPathLiteral,
    toolRegex,
    action: rule.action,
    decision: rule.decision,
  };
}

// ---------------------------------------------------------------------------
// Pattern Matching
// ---------------------------------------------------------------------------

/**
 * Matches a resolved absolute path against a pre-compiled rule. Literal
 * directory patterns use a startsWith fast path; glob patterns use the
 * pre-compiled pathRegex. Both inputs are slash-normalized first.
 */
function matchPathCompiled(absPath: string, rule: CompiledRule): boolean {
  const normalizedPath = absPath.replace(/\\/g, "/");

  if (rule.isPathLiteral && rule.pathPattern) {
    // Directory/file prefix match — no regex engine needed.
    return normalizedPath === rule.pathPattern ||
           normalizedPath.startsWith(rule.pathPattern + "/");
  }
  if (rule.pathRegex) {
    return rule.pathRegex.test(normalizedPath);
  }
  return false;
}

/**
 * Simple glob matching supporting *, **, and ?.
 *
 * Note: this compiles a RegExp per call and is retained for standalone use
 * (e.g. matchHost callers). The hot permission-check path pre-compiles rule
 * patterns once at construction via compileRule() and does not call this.
 */
function matchGlob(str: string, pattern: string): boolean {
  const regexStr = globToRegex(pattern);
  const regex = new RegExp(`^${regexStr}$`);
  return regex.test(str);
}

/**
 * Converts a glob pattern (supporting *, **, ?, and [...] classes) into a
 * regex source string. `*` matches within a path segment; `**` matches across
 * segments (optionally including the trailing slash); `?` matches one non-slash
 * char; `.` is escaped.
 */
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
