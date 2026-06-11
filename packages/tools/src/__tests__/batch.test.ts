import type { ToolCall } from "@orangecoding/core";
import { executeBatch } from "../batch.js";
import { ToolRegistry } from "../registry.js";
import type { Tool, ToolMetadata } from "../tool.js";

function tool(name: string, isConcurrencySafe: boolean, fn: () => Promise<string>): Tool {
  const metadata: ToolMetadata = {
    isReadOnly: isConcurrencySafe,
    isConcurrencySafe,
    isDestructive: !isConcurrencySafe,
    isEnabled: true,
  };

  return {
    name: () => name,
    description: () => name,
    parameters: () => ({}),
    execute: fn,
    metadata: () => metadata,
  };
}

function call(id: string, functionName: string): ToolCall {
  return { id, function_name: functionName, arguments: {} };
}

describe("executeBatch", () => {
  it("does not execute a later concurrency-safe tool before an earlier unsafe tool completes", async () => {
    const registry = new ToolRegistry();
    const events: string[] = [];
    let unsafeRelease!: () => void;

    registry.register(tool("write", false, async () => {
      events.push("write:start");
      await new Promise<void>((resolve) => {
        unsafeRelease = resolve;
      });
      events.push("write:end");
      return "write-output";
    }));

    registry.register(tool("read", true, async () => {
      events.push("read:start");
      return "read-output";
    }));

    const resultsPromise = executeBatch(undefined, registry, [
      call("1", "write"),
      call("2", "read"),
    ]);

    await Promise.resolve();
    expect(events).toEqual(["write:start"]);

    unsafeRelease();
    const results = await resultsPromise;

    expect(events).toEqual(["write:start", "write:end", "read:start"]);
    expect(results.map((result) => result.toolCallID)).toEqual(["1", "2"]);
  });

  it("executes adjacent concurrency-safe tools in the same batch", async () => {
    const registry = new ToolRegistry();
    const events: string[] = [];
    let releaseFirst!: () => void;

    registry.register(tool("safe-a", true, async () => {
      events.push("safe-a:start");
      await new Promise<void>((resolve) => {
        releaseFirst = resolve;
      });
      events.push("safe-a:end");
      return "a";
    }));
    registry.register(tool("safe-b", true, async () => {
      events.push("safe-b:start");
      return "b";
    }));

    const resultsPromise = executeBatch(undefined, registry, [
      call("1", "safe-a"),
      call("2", "safe-b"),
    ]);

    await Promise.resolve();
    expect(events).toEqual(["safe-a:start", "safe-b:start"]);

    releaseFirst();
    const results = await resultsPromise;

    expect(results.map((result) => result.content)).toEqual(["a", "b"]);
  });
});
