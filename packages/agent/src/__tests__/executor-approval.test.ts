import { jest } from "@jest/globals";
import { ToolExecutor } from "../executor.js";
import {
  ToolRegistry,
  PermissionDecision,
  AutoApproveHandler,
  AutoDenyHandler,
  type ApprovalHandler,
  type ApprovalRequest,
  type ApprovalResult,
} from "@orangecoding/tools";
import type { Tool, ToolMetadata } from "@orangecoding/tools";
import type { PermissionContext } from "@orangecoding/tools";

/** A tool that always asks for permission. */
class AskTool implements Tool {
  name(): string { return "ask_tool"; }
  description(): string { return "A tool that asks for permission."; }
  parameters(): Record<string, unknown> { return { type: "object", properties: {} }; }
  metadata(): ToolMetadata { return { isReadOnly: false, isConcurrencySafe: false, isDestructive: true, isEnabled: true }; }
  async execute(_ctx: unknown, _input: unknown): Promise<string> { return "executed"; }
  checkPermissions(_ctx: PermissionContext): PermissionDecision { return PermissionDecision.Ask; }
}

/** A tool that always allows. */
class AllowTool implements Tool {
  name(): string { return "allow_tool"; }
  description(): string { return "A tool that allows."; }
  parameters(): Record<string, unknown> { return { type: "object", properties: {} }; }
  metadata(): ToolMetadata { return { isReadOnly: true, isConcurrencySafe: true, isDestructive: false, isEnabled: true }; }
  async execute(_ctx: unknown, _input: unknown): Promise<string> { return "executed"; }
  checkPermissions(_ctx: PermissionContext): PermissionDecision { return PermissionDecision.Allow; }
}

/** A tool that always denies. */
class DenyTool implements Tool {
  name(): string { return "deny_tool"; }
  description(): string { return "A tool that denies."; }
  parameters(): Record<string, unknown> { return { type: "object", properties: {} }; }
  metadata(): ToolMetadata { return { isReadOnly: false, isConcurrencySafe: false, isDestructive: true, isEnabled: true }; }
  async execute(_ctx: unknown, _input: unknown): Promise<string> { return "executed"; }
  checkPermissions(_ctx: PermissionContext): PermissionDecision { return PermissionDecision.Deny; }
}

/** A recording approval handler that tracks requests. */
class RecordingHandler implements ApprovalHandler {
  public requests: ApprovalRequest[] = [];
  public autoApprove = true;

  async requestApproval(request: ApprovalRequest): Promise<ApprovalResult> {
    this.requests.push(request);
    return { approved: this.autoApprove, reason: this.autoApprove ? "auto" : "denied" };
  }
}

describe("ToolExecutor approval", () => {
  it("executes a tool when permission returns Allow", async () => {
    const registry = new ToolRegistry();
    registry.register(new AllowTool());
    const executor = new ToolExecutor(registry);

    const result = await executor.execute(undefined, {
      id: "call-1",
      function_name: "allow_tool",
      arguments: {},
    });

    expect(result.isError).toBe(false);
    expect(result.content).toBe("executed");
  });

  it("denies a tool when permission returns Deny", async () => {
    const registry = new ToolRegistry();
    registry.register(new DenyTool());
    const executor = new ToolExecutor(registry);

    const result = await executor.execute(undefined, {
      id: "call-2",
      function_name: "deny_tool",
      arguments: {},
    });

    expect(result.isError).toBe(true);
    expect(result.content).toContain("denied");
  });

  it("asks the ApprovalHandler when permission returns Ask, and executes on approval", async () => {
    const registry = new ToolRegistry();
    registry.register(new AskTool());
    const handler = new RecordingHandler();
    handler.autoApprove = true;
    const executor = new ToolExecutor(registry);
    executor.setApprovalHandler(handler);

    const result = await executor.execute(undefined, {
      id: "call-3",
      function_name: "ask_tool",
      arguments: { key: "value" },
    });

    expect(handler.requests).toHaveLength(1);
    expect(handler.requests[0]!.toolName).toBe("ask_tool");
    expect(result.isError).toBe(false);
    expect(result.content).toBe("executed");
  });

  it("denies execution when ApprovalHandler denies the request", async () => {
    const registry = new ToolRegistry();
    registry.register(new AskTool());
    const handler = new RecordingHandler();
    handler.autoApprove = false;
    const executor = new ToolExecutor(registry);
    executor.setApprovalHandler(handler);

    const result = await executor.execute(undefined, {
      id: "call-4",
      function_name: "ask_tool",
      arguments: {},
    });

    expect(handler.requests).toHaveLength(1);
    expect(result.isError).toBe(true);
    expect(result.content).toContain("denied");
  });

  it("uses AutoApproveHandler when no handler is set and autoApproveTools is enabled", async () => {
    const registry = new ToolRegistry();
    registry.register(new AskTool());
    const executor = new ToolExecutor(registry);
    executor.setAutoApprove(true);

    const result = await executor.execute(undefined, {
      id: "call-5",
      function_name: "ask_tool",
      arguments: {},
    });

    expect(result.isError).toBe(false);
    expect(result.content).toBe("executed");
  });
});
