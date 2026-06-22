/**
 * Tests for the ToolRegistry — registration, lookup, listing, and replacement.
 */

import { ToolRegistry } from "../registry.js";
import type { Tool } from "../tool.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Creates a mock tool with a given name. */
function mockTool(name: string): Tool {
  return {
    name: () => name,
    description: () => `Mock tool: ${name}`,
    parameters: () => ({ type: "object", properties: {} }),
    execute: async () => `executed ${name}`,
  };
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

describe("ToolRegistry", () => {
  it("registers and retrieves a tool", () => {
    const reg = new ToolRegistry();
    const tool = mockTool("test-tool");
    reg.register(tool);

    const [found, ok] = reg.get("test-tool");
    expect(ok).toBe(true);
    expect(found).toBe(tool);
  });

  it("returns false for non-existent tool", () => {
    const reg = new ToolRegistry();
    const [found, ok] = reg.get("missing");
    expect(ok).toBe(false);
    expect(found).toBeUndefined();
  });

  it("replaces a tool with the same name", () => {
    const reg = new ToolRegistry();
    const tool1 = mockTool("tool-a");
    const tool2 = mockTool("tool-a");

    reg.register(tool1);
    reg.register(tool2);

    const [found] = reg.get("tool-a");
    expect(found).toBe(tool2);
    expect(reg.list()).toHaveLength(1);
  });

  it("lists all registered tools", () => {
    const reg = new ToolRegistry();
    reg.register(mockTool("alpha"));
    reg.register(mockTool("beta"));
    reg.register(mockTool("gamma"));

    const tools = reg.list();
    expect(tools).toHaveLength(3);
    expect(tools.map((t) => t.name())).toEqual(
      expect.arrayContaining(["alpha", "beta", "gamma"]),
    );
  });

  it("starts with an empty registry", () => {
    const reg = new ToolRegistry();
    expect(reg.list()).toHaveLength(0);
  });
});
