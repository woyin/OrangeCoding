/**
 * Core Tool interface and supporting types.
 *
 * Ported from modules/tools/tool.go.
 * Enhanced with per-tool call budgets (maxUses/softLimit).
 */

import type { PermissionContext, PermissionDecision } from "./permissions.js";

// ---------------------------------------------------------------------------
// ToolMetadata
// ---------------------------------------------------------------------------

export interface ToolMetadata {
  readonly isReadOnly: boolean;
  readonly isConcurrencySafe: boolean;
  readonly isDestructive: boolean;
  readonly isEnabled: boolean;
  /** Maximum number of times this tool can be called per agent run. 0 = unlimited. */
  readonly maxUses: number;
  /** Soft limit — warn when reached but do not block. 0 = no soft limit. */
  readonly softLimit: number;
}

/** Returns metadata with only isEnabled set to true. */
export function defaultMetadata(): ToolMetadata {
  return { isReadOnly: false, isConcurrencySafe: false, isDestructive: false, isEnabled: true, maxUses: 0, softLimit: 0 };
}

/** Returns metadata for read-only, concurrency-safe tools. */
export function readOnlyMetadata(): ToolMetadata {
  return { isReadOnly: true, isConcurrencySafe: true, isDestructive: false, isEnabled: true, maxUses: 0, softLimit: 0 };
}

/** Returns metadata for tools that modify the filesystem or state. */
export function destructiveMetadata(): ToolMetadata {
  return { isReadOnly: false, isConcurrencySafe: false, isDestructive: true, isEnabled: true, maxUses: 0, softLimit: 0 };
}

/**
 * Build ToolMetadata with custom maxUses and softLimit.
 * Usage: withBudget(defaultMetadata(), { maxUses: 10, softLimit: 7 })
 */
export function withBudget(
  base: ToolMetadata,
  budget: { maxUses?: number; softLimit?: number },
): ToolMetadata {
  return {
    ...base,
    maxUses: budget.maxUses ?? base.maxUses,
    softLimit: budget.softLimit ?? base.softLimit,
  };
}

// ---------------------------------------------------------------------------
// ToolError
// ---------------------------------------------------------------------------

/**
 * Structured error kind names, mirroring Go's ToolError.Kind values.
 */
export type ToolErrorKind =
  | "invalid_params"
  | "execution_error"
  | "security_violation"
  | "not_found"
  | "budget_exceeded";

/**
 * A structured error returned by tool execution.
 * Mirrors Go's `tools.ToolError`.
 */
export class ToolError extends Error {
  public readonly kind: ToolErrorKind;

  constructor(kind: ToolErrorKind, message: string) {
    super(`${kind}: ${message}`);
    this.name = "ToolError";
    this.kind = kind;
  }
}

// ---------------------------------------------------------------------------
// Tool interface
// ---------------------------------------------------------------------------

/**
 * The interface that every tool must implement.
 * Mirrors Go's `tools.Tool`.
 */
export interface Tool {
  /** Unique tool identifier (e.g. "bash", "read_file"). */
  name(): string;

  /** Human-readable description of what the tool does. */
  description(): string;

  /** JSON Schema describing the tool's input parameters. */
  parameters(): Record<string, unknown>;

  /** Execute the tool with the given JSON-compatible input and return a string result. */
  execute(ctx: unknown, input: unknown): Promise<string>;

  /** Metadata about the tool's behaviour. */
  metadata(): ToolMetadata;

  /**
   * Check permissions before execution.
   * Default implementation returns Allow.
   */
  checkPermissions?(ctx: PermissionContext): PermissionDecision;
}

// ---------------------------------------------------------------------------
// ToolBudgetTracker — per-tool call budget tracking
// ---------------------------------------------------------------------------

/** Result of a budget check. */
export type BudgetCheckResult =
  | { kind: "allow" }
  | { kind: "warn"; remaining: number; message: string }
  | { kind: "deny"; message: string };

/**
 * Tracks per-tool call budgets across an agent run.
 * Mirrors OpenAI Agents SDK's tool.max_uses feature.
 */
export class ToolBudgetTracker {
  private _counts = new Map<string, number>();

  /** Record a tool call. Returns the new count. */
  recordUse(toolName: string): number {
    const current = this._counts.get(toolName) ?? 0;
    const next = current + 1;
    this._counts.set(toolName, next);
    return next;
  }

  /** Get the current usage count for a tool. */
  getCount(toolName: string): number {
    return this._counts.get(toolName) ?? 0;
  }

  /** Check if a tool call is within budget. Does NOT increment the counter. */
  checkBudget(toolName: string, meta: ToolMetadata): BudgetCheckResult {
    const count = this.getCount(toolName);

    // Hard limit check
    if (meta.maxUses > 0 && count >= meta.maxUses) {
      return {
        kind: "deny",
        message: `Tool "${toolName}" has reached its maximum usage limit of ${meta.maxUses} calls.`,
      };
    }

    // Soft limit warning
    if (meta.softLimit > 0 && count >= meta.softLimit) {
      const remaining = meta.maxUses > 0 ? meta.maxUses - count : -1;
      return {
        kind: "warn",
        remaining,
        message: `Tool "${toolName}" has been used ${count} times (soft limit: ${meta.softLimit}).`,
      };
    }

    return { kind: "allow" };
  }

  /** Get a snapshot of all tool usage counts. */
  snapshot(): Record<string, number> {
    const result: Record<string, number> = {};
    for (const [name, count] of this._counts) {
      result[name] = count;
    }
    return result;
  }

  /** Reset all counters (e.g. for a new agent run). */
  reset(): void {
    this._counts.clear();
  }

  /** Total number of tool calls across all tools. */
  totalCalls(): number {
    let total = 0;
    for (const count of this._counts.values()) {
      total += count;
    }
    return total;
  }
}
