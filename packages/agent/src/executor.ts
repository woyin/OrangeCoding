/**
 * ToolExecutor dispatches tool calls to the tool registry and collects results.
 * Ported from modules/agent/executor.go.
 */

import type { AgentId, ToolCall as CoreToolCall } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import type { ExecuteResult } from "@orangecoding/tools";
import type { SecurityGuard } from "./security-bridge.js";

const DEFAULT_TIMEOUT_MS = 30_000;
const MAX_CONCURRENT_TOOLS = 8;

export class ToolExecutor {
  private _registry: ToolRegistry;
  private _timeoutMs: number;
  private _guard: SecurityGuard | null;

  constructor(registry: ToolRegistry) {
    this._registry = registry;
    this._timeoutMs = DEFAULT_TIMEOUT_MS;
    this._guard = null;
  }

  /** Execute runs a single tool call. */
  async execute(signal: AbortSignal | undefined, call: CoreToolCall): Promise<ExecuteResult> {
    const start = Date.now();

    // Security guard check before execution
    if (this._guard !== null) {
      const [ok, reason] = this._guard.validateToolCall({} as AgentId, call.function_name);
      if (!ok) {
        return {
          toolCallID: call.id,
          content: "tool denied by security guard: " + reason,
          isError: true,
          durationMs: Date.now() - start,
        };
      }
    }

    const [tool, found] = this._registry.get(call.function_name);
    if (!found) {
      return {
        toolCallID: call.id,
        content: "tool not found: " + call.function_name,
        isError: true,
        durationMs: Date.now() - start,
      };
    }

    try {
      const out = await tool.execute(signal, call.arguments);
      return {
        toolCallID: call.id,
        content: out,
        isError: false,
        durationMs: Date.now() - start,
      };
    } catch (err) {
      return {
        toolCallID: call.id,
        content: err instanceof Error ? err.message : String(err),
        isError: true,
        durationMs: Date.now() - start,
      };
    }
  }

  /** ExecuteBatch runs tool calls concurrently with a bounded concurrency limit. */
  async executeBatch(signal: AbortSignal | undefined, calls: CoreToolCall[]): Promise<ExecuteResult[]> {
    const results: ExecuteResult[] = new Array(calls.length);

    // Use a simple semaphore pattern with Promise.all
    const semaphore: Promise<void>[] = [];
    let activeCount = 0;
    const waiters: (() => void)[] = [];

    const acquire = (): Promise<void> => {
      if (activeCount < MAX_CONCURRENT_TOOLS) {
        activeCount++;
        return Promise.resolve();
      }
      return new Promise<void>((resolve) => {
        waiters.push(() => {
          activeCount++;
          resolve();
        });
      });
    };

    const release = (): void => {
      activeCount--;
      const next = waiters.shift();
      if (next) next();
    };

    const tasks = calls.map(async (call, idx) => {
      await acquire();
      try {
        results[idx] = await this.execute(signal, call);
      } finally {
        release();
      }
    });

    await Promise.all(tasks);
    return results;
  }

  /** SetTimeout configures the per-call execution timeout in milliseconds. */
  setTimeout(ms: number): void {
    this._timeoutMs = ms;
  }

  /** SetSecurityGuard sets the security guard for tool execution. */
  setSecurityGuard(guard: SecurityGuard): void {
    this._guard = guard;
  }

  /** Registry exposes the underlying tool registry. */
  get registry(): ToolRegistry {
    return this._registry;
  }
}

/** FilteredRegistry creates a new registry containing only the named tools. */
export function filteredRegistry(parent: ToolRegistry, names: string[]): ToolRegistry {
  const filtered = new ToolRegistry();
  const nameSet = new Set(names);
  for (const t of parent.list()) {
    if (nameSet.has(t.name())) {
      filtered.register(t);
    }
  }
  return filtered;
}
