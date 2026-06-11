/**
 * ToolExecutor dispatches tool calls to the tool registry and collects results.
 * Ported from modules/agent/executor.go.
 */

import type { AgentId, ToolCall as CoreToolCall } from "@orangecoding/core";
import { ToolRegistry, executeBatch, PermissionDecision, AutoApproveHandler } from "@orangecoding/tools";
import type { ExecuteResult, PermissionContext, ApprovalHandler, ApprovalRequest } from "@orangecoding/tools";
import type { SecurityGuard } from "./security-bridge.js";
import { randomUUID } from "node:crypto";

const DEFAULT_TIMEOUT_MS = 30_000;

export class ToolExecutor {
  private _registry: ToolRegistry;
  private _timeoutMs: number;
  private _guard: SecurityGuard | null;
  private _approvalHandler: ApprovalHandler | null;
  private _autoApprove: boolean;

  constructor(registry: ToolRegistry) {
    this._registry = registry;
    this._timeoutMs = DEFAULT_TIMEOUT_MS;
    this._guard = null;
    this._approvalHandler = null;
    this._autoApprove = false;
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

    // Permission check
    if (tool.checkPermissions) {
      const permCtx: PermissionContext = {
        workingDir: process.cwd(),
        isReadOnly: tool.metadata().isReadOnly,
      };
      const decision = tool.checkPermissions(permCtx);

      if (decision === PermissionDecision.Deny) {
        return {
          toolCallID: call.id,
          content: "tool denied by permission policy",
          isError: true,
          durationMs: Date.now() - start,
        };
      }

      if (decision === PermissionDecision.Ask) {
        const approved = await this.requestApproval(call);
        if (!approved) {
          return {
            toolCallID: call.id,
            content: "tool execution denied by user",
            isError: true,
            durationMs: Date.now() - start,
          };
        }
      }
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

  /** ExecuteBatch runs tool calls respecting concurrency safety. */
  async executeBatch(signal: AbortSignal | undefined, calls: CoreToolCall[]): Promise<ExecuteResult[]> {
    // Use the concurrency-safe executeBatch from @orangecoding/tools
    return executeBatch(signal, this._registry, calls);
  }

  /** SetTimeout configures the per-call execution timeout in milliseconds. */
  setTimeout(ms: number): void {
    this._timeoutMs = ms;
  }

  /** SetSecurityGuard sets the security guard for tool execution. */
  setSecurityGuard(guard: SecurityGuard): void {
    this._guard = guard;
  }

  /**
   * SetApprovalHandler configures the handler for tool approval requests.
   * When a tool's checkPermissions returns Ask, this handler is invoked.
   */
  setApprovalHandler(handler: ApprovalHandler): void {
    this._approvalHandler = handler;
  }

  /**
   * SetAutoApprove enables or disables automatic approval of all tools.
   * When enabled, Ask decisions are auto-approved without prompting.
   */
  setAutoApprove(enabled: boolean): void {
    this._autoApprove = enabled;
  }

  /** Registry exposes the underlying tool registry. */
  get registry(): ToolRegistry {
    return this._registry;
  }

  /** Request approval for a tool call. Returns true if approved. */
  private async requestApproval(call: CoreToolCall): Promise<boolean> {
    const handler = this._approvalHandler ?? (this._autoApprove ? new AutoApproveHandler() : null);

    if (handler === null) {
      // No handler configured — deny by default for safety
      return false;
    }

    const request: ApprovalRequest = {
      requestId: randomUUID(),
      toolName: call.function_name,
      toolArguments: call.arguments,
      reason: `Tool "${call.function_name}" requires approval before execution.`,
    };

    const result = await handler.requestApproval(request);
    return result.approved;
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
