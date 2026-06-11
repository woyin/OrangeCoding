/**
 * ExecuteBatch - runs tool calls with concurrency safety and per-tool budgets.
 *
 * Ported from modules/tools/batch.go.
 * Enhanced with ToolBudgetTracker integration.
 */

import type { ToolRegistry } from "./registry.js";
import type { ToolCall } from "@orangecoding/core";
import { ToolBudgetTracker } from "./tool.js";

// ---------------------------------------------------------------------------
// ExecuteResult
// ---------------------------------------------------------------------------

/** Holds the outcome of executing a single tool call in a batch. */
export interface ExecuteResult {
  toolCallID: string;
  content: string;
  isError: boolean;
  durationMs: number;
  /** Set when a soft-limit warning was triggered. */
  budgetWarning?: string;
}

interface ResolvedCall {
  call: ToolCall;
  concurrent: boolean;
}

interface ExecutionBatch {
  calls: ResolvedCall[];
  concurrent: boolean;
}

// ---------------------------------------------------------------------------
// ExecuteBatch
// ---------------------------------------------------------------------------

/**
 * Runs tool calls respecting concurrency safety and per-tool budgets.
 * - Safe tools (isConcurrencySafe=true) run concurrently via Promise.all.
 * - Unsafe tools (isConcurrencySafe=false) run serially to prevent races.
 * - Per-tool maxUses budgets are checked before execution.
 * Results are returned in the same order as the input calls array.
 */
export async function executeBatch(
  ctx: unknown,
  registry: ToolRegistry,
  calls: ToolCall[],
  budgetTracker?: ToolBudgetTracker,
): Promise<ExecuteResult[]> {
  const batches = partitionToolCalls(registry, calls);
  const results: ExecuteResult[] = [];

  for (const batch of batches) {
    if (batch.concurrent) {
      results.push(...await Promise.all(batch.calls.map((item) =>
        executeOne(ctx, registry, item.call, budgetTracker),
      )));
    } else {
      for (const item of batch.calls) {
        results.push(await executeOne(ctx, registry, item.call, budgetTracker));
      }
    }
  }

  return results;
}

export function partitionToolCalls(registry: ToolRegistry, calls: ToolCall[]): ExecutionBatch[] {
  const batches: ExecutionBatch[] = [];

  for (const call of calls) {
    const [tool, ok] = registry.get(call.function_name);
    const concurrent = !ok || tool.metadata().isConcurrencySafe;
    const previous = batches[batches.length - 1];

    if (concurrent && previous?.concurrent) {
      previous.calls.push({ call, concurrent });
      continue;
    }

    batches.push({ calls: [{ call, concurrent }], concurrent });
  }

  return batches;
}

async function executeOne(
  ctx: unknown,
  registry: ToolRegistry,
  call: ToolCall,
  budgetTracker?: ToolBudgetTracker,
): Promise<ExecuteResult> {
  const start = Date.now();
  const [tool, ok] = registry.get(call.function_name);

  if (!ok) {
    return {
      toolCallID: call.id,
      content: "tool not found: " + call.function_name,
      isError: true,
      durationMs: Date.now() - start,
    };
  }

  // Per-tool budget check
  let budgetWarning: string | undefined;
  if (budgetTracker) {
    const meta = tool.metadata();
    const check = budgetTracker.checkBudget(call.function_name, meta);
    if (check.kind === "deny") {
      return {
        toolCallID: call.id,
        content: check.message,
        isError: true,
        durationMs: Date.now() - start,
      };
    }
    if (check.kind === "warn") {
      budgetWarning = check.message;
    }
    // Record the usage
    budgetTracker.recordUse(call.function_name);
  }

  try {
    const out = await tool.execute(ctx, call.arguments);
    const result: ExecuteResult = {
      toolCallID: call.id,
      content: out,
      isError: false,
      durationMs: Date.now() - start,
    };
    if (budgetWarning) {
      result.budgetWarning = budgetWarning;
    }
    return result;
  } catch (err) {
    return {
      toolCallID: call.id,
      content: err instanceof Error ? err.message : String(err),
      isError: true,
      durationMs: Date.now() - start,
    };
  }
}
