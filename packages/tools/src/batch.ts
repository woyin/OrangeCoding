/**
 * ExecuteBatch - runs all tool calls concurrently using Promise.all.
 *
 * Ported from modules/tools/batch.go.
 */

import type { Tool } from "./tool.js";
import type { ToolRegistry } from "./registry.js";
import type { ToolCall } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// ExecuteResult
// ---------------------------------------------------------------------------

/** Holds the outcome of executing a single tool call in a batch. */
export interface ExecuteResult {
  toolCallID: string;
  content: string;
  isError: boolean;
  durationMs: number;
}

// ---------------------------------------------------------------------------
// ExecuteBatch
// ---------------------------------------------------------------------------

/**
 * Runs all tool calls concurrently using Promise.all.
 * Each call is dispatched in its own async task.
 * Results are returned in the same order as the input calls array.
 */
export async function executeBatch(
  ctx: unknown,
  registry: ToolRegistry,
  calls: ToolCall[],
): Promise<ExecuteResult[]> {
  const promises = calls.map(async (call) => {
    const start = Date.now();

    const [tool, ok] = registry.get(call.function_name);
    if (!ok) {
      return {
        toolCallID: call.id,
        content: "tool not found: " + call.function_name,
        isError: true,
        durationMs: Date.now() - start,
      } satisfies ExecuteResult;
    }

    try {
      const out = await tool.execute(ctx, call.arguments);
      return {
        toolCallID: call.id,
        content: out,
        isError: false,
        durationMs: Date.now() - start,
      } satisfies ExecuteResult;
    } catch (err) {
      return {
        toolCallID: call.id,
        content: err instanceof Error ? err.message : String(err),
        isError: true,
        durationMs: Date.now() - start,
      } satisfies ExecuteResult;
    }
  });

  return Promise.all(promises);
}
