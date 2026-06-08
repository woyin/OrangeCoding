/**
 * @module budget
 * Tool execution budget enforcement.
 */

import type { AgentId } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// ToolBudget
// ---------------------------------------------------------------------------

/** Limits tool usage per agent. */
export interface ToolBudget {
  maxCalls: number;
  maxTokens: number;
  maxWallTimeMs: number;
}

// ---------------------------------------------------------------------------
// BudgetUsage
// ---------------------------------------------------------------------------

/** Tracks current consumption. */
export interface BudgetUsage {
  calls: number;
  tokens: number;
  elapsedMs: number;
}

// ---------------------------------------------------------------------------
// BudgetGuard
// ---------------------------------------------------------------------------

/** Enforces tool execution budgets. */
export class BudgetGuard {
  private budgets = new Map<string, ToolBudget>();
  private usage = new Map<string, BudgetUsage>();

  /** SetBudget sets the budget for an agent. */
  setBudget(agentID: AgentId, budget: ToolBudget): void {
    this.budgets.set(agentID.toString(), budget);
    this.usage.set(agentID.toString(), { calls: 0, tokens: 0, elapsedMs: 0 });
  }

  /** Check verifies if an agent has budget remaining. Returns [ok, reason]. */
  check(agentID: AgentId): [boolean, string] {
    const key = agentID.toString();
    const budget = this.budgets.get(key);
    if (!budget) {
      return [true, ""];
    }

    let usage = this.usage.get(key);
    if (!usage) {
      usage = { calls: 0, tokens: 0, elapsedMs: 0 };
      this.usage.set(key, usage);
    }

    if (budget.maxCalls > 0 && usage.calls >= budget.maxCalls) {
      return [false, `call budget exceeded: ${usage.calls}/${budget.maxCalls}`];
    }

    usage.calls++;
    return [true, ""];
  }
}
