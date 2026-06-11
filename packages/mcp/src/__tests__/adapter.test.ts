/**
 * Tests for the MCP Tool Adapter and Tool Manager.
 */

import { McpToolAdapter, McpToolManager } from "../adapter.js";
import type { McpClient } from "../client.js";
import type { ToolInfo } from "../jsonrpc.js";

// Manual mock helpers (no jest.fn() — project uses ESM + ts-jest)
type MockFn<T extends (...args: any[]) => any> = {
  (...args: Parameters<T>): ReturnType<T>;
  calls: Parameters<T>[];
  callCount: number;
};

function mockFn<T extends (...args: any[]) => any>(impl: T): MockFn<T> {
  const calls: any[] = [];
  const fn = ((...args: any[]) => {
    calls.push(args);
    fn.calls = calls;
    fn.callCount = calls.length;
    return impl(...args);
  }) as MockFn<T>;
  fn.calls = calls;
  fn.callCount = 0;
  return fn;
}

function mockResolvedValue<T>(value: T) {
  return mockFn((() => Promise.resolve(value)) as any) as any;
}

function mockResolvedValues<T>(...values: T[]) {
  let idx = 0;
  return mockFn((() => {
    const v = values[Math.min(idx, values.length - 1)];
    idx++;
    return Promise.resolve(v);
  }) as any) as any;
}

function mockRejectedValue(err: Error) {
  return mockFn((() => Promise.reject(err)) as any) as any;
}

function createMockClient(overrides: Record<string, any> = {}): McpClient {
  return {
    initialize: mockResolvedValue({ name: "test-server", version: "1.0.0" }),
    listTools: mockResolvedValue([]),
    callTool: mockResolvedValue("ok"),
    close: mockResolvedValue(undefined),
    ...overrides,
  } as unknown as McpClient;
}

describe("McpToolAdapter", () => {
  it("wraps tool info correctly", () => {
    const client = createMockClient();
    const info: ToolInfo = {
      name: "read_document",
      description: "Read a document from the database",
      inputSchema: {
        type: "object",
        properties: { id: { type: "string" } },
        required: ["id"],
      },
    };

    const adapter = new McpToolAdapter(client, info);

    expect(adapter.name()).toBe("read_document");
    expect(adapter.description()).toBe("Read a document from the database");
    expect(adapter.parameters()).toEqual(info.inputSchema);
    expect(adapter.metadata().isEnabled).toBe(true);
    expect(adapter.metadata().isConcurrencySafe).toBe(true);
  });

  it("uses default description when none provided", () => {
    const client = createMockClient();
    const info: ToolInfo = { name: "my_tool" };
    const adapter = new McpToolAdapter(client, info);
    expect(adapter.description()).toBe("MCP tool: my_tool");
  });

  it("uses empty schema when none provided", () => {
    const client = createMockClient();
    const info: ToolInfo = { name: "my_tool" };
    const adapter = new McpToolAdapter(client, info);
    expect(adapter.parameters()).toEqual({ type: "object", properties: {} });
  });

  it("calls MCP client on execute", async () => {
    const callTool = mockResolvedValue("result text");
    const client = createMockClient({ callTool });
    const info: ToolInfo = { name: "do_thing" };

    const adapter = new McpToolAdapter(client, info);
    const result = await adapter.execute(null, { arg1: "value" });

    expect(callTool.calls[0]).toEqual(["do_thing", { arg1: "value" }]);
    expect(result).toBe("result text");
  });

  it("handles null/undefined results", async () => {
    const client = createMockClient({
      callTool: mockResolvedValue(null),
    });
    const adapter = new McpToolAdapter(client, { name: "t" });
    expect(await adapter.execute(null, {})).toBe("");
  });

  it("handles MCP content block arrays", async () => {
    const client = createMockClient({
      callTool: mockResolvedValue([
        { type: "text", text: "Hello " },
        { type: "text", text: "World" },
      ]),
    });
    const adapter = new McpToolAdapter(client, { name: "t" });
    expect(await adapter.execute(null, {})).toBe("Hello \nWorld");
  });

  it("handles content wrapper objects", async () => {
    const client = createMockClient({
      callTool: mockResolvedValue({
        content: [{ type: "text", text: "wrapped result" }],
      }),
    });
    const adapter = new McpToolAdapter(client, { name: "t" });
    expect(await adapter.execute(null, {})).toBe("wrapped result");
  });

  it("handles JSON object results", async () => {
    const client = createMockClient({
      callTool: mockResolvedValue({ key: "value" }),
    });
    const adapter = new McpToolAdapter(client, { name: "t" });
    const result = await adapter.execute(null, {});
    expect(result).toContain('"key"');
    expect(result).toContain('"value"');
  });

  it("wraps client errors with context", async () => {
    const client = createMockClient({
      callTool: mockRejectedValue(new Error("connection lost")),
    });
    const adapter = new McpToolAdapter(client, { name: "failing_tool" });
    await expect(adapter.execute(null, {})).rejects.toThrow("mcp_tool_error [failing_tool]");
    await expect(adapter.execute(null, {})).rejects.toThrow("connection lost");
  });
});

describe("McpToolManager", () => {
  it("adds a server with pre-connected client", async () => {
    const manager = new McpToolManager();
    const tools: ToolInfo[] = [
      { name: "tool_a", description: "Tool A" },
      { name: "tool_b", description: "Tool B" },
    ];
    const client = createMockClient({
      listTools: mockResolvedValue(tools),
    });

    const adapters = await manager.addServer({ id: "test", client });
    expect(adapters).toHaveLength(2);
    expect(adapters[0].name()).toBe("tool_a");
    expect(adapters[1].name()).toBe("tool_b");

    await manager.close();
  });

  it("rejects duplicate server IDs", async () => {
    const manager = new McpToolManager();
    const client = createMockClient();

    await manager.addServer({ id: "dup", client });
    await expect(
      manager.addServer({ id: "dup", client: createMockClient() })
    ).rejects.toThrow("already registered");

    await manager.close();
  });

  it("lists all tools from multiple servers", async () => {
    const manager = new McpToolManager();

    await manager.addServer({
      id: "s1",
      client: createMockClient({
        listTools: mockResolvedValue([{ name: "a" }]),
      }),
    });

    await manager.addServer({
      id: "s2",
      client: createMockClient({
        listTools: mockResolvedValue([{ name: "b" }, { name: "c" }]),
      }),
    });

    const allTools = manager.getAllTools();
    expect(allTools).toHaveLength(3);
    expect(allTools.map((t) => t.name())).toEqual(["a", "b", "c"]);

    await manager.close();
  });

  it("removes a server", async () => {
    const manager = new McpToolManager();
    const client = createMockClient({
      listTools: mockResolvedValue([{ name: "x" }]),
    });

    await manager.addServer({ id: "removable", client });
    expect(manager.listServers()).toContain("removable");

    await manager.removeServer("removable");
    expect(manager.listServers()).not.toContain("removable");
    expect(manager.getAllTools()).toHaveLength(0);
  });

  it("refreshes tools from a server", async () => {
    const manager = new McpToolManager();
    const listTools = mockResolvedValues(
      [{ name: "initial" }],
      [{ name: "refreshed_1" }, { name: "refreshed_2" }],
    );
    const client = createMockClient({ listTools });

    await manager.addServer({ id: "s", client });
    expect(manager.getServerTools("s")).toHaveLength(1);

    const newTools = await manager.refreshServer("s");
    expect(newTools).toHaveLength(2);
    expect(newTools[0].name()).toBe("refreshed_1");

    await manager.close();
  });

  it("handles initialization failure gracefully", async () => {
    const manager = new McpToolManager();
    const client = createMockClient({
      initialize: mockRejectedValue(new Error("auth failed")),
    });

    await expect(
      manager.addServer({ id: "broken", client })
    ).rejects.toThrow("auth failed");

    expect(client.close.calls.length).toBeGreaterThanOrEqual(1);
    expect(manager.listServers()).toHaveLength(0);
  });

  it("requires either client or command", async () => {
    const manager = new McpToolManager();
    await expect(
      manager.addServer({ id: "bad" })
    ).rejects.toThrow("must provide either client or command");
  });

  it("closes all servers on shutdown", async () => {
    const manager = new McpToolManager();
    const c1 = createMockClient();
    const c2 = createMockClient();

    await manager.addServer({ id: "s1", client: c1 });
    await manager.addServer({ id: "s2", client: c2 });

    await manager.close();
    expect(c1.close.calls.length).toBeGreaterThanOrEqual(1);
    expect(c2.close.calls.length).toBeGreaterThanOrEqual(1);
    expect(manager.listServers()).toHaveLength(0);
  });
});
