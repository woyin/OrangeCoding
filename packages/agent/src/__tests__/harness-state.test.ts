/**
 * Tests for the harness-state module — state constants, checkpoint store,
 * trace events, and clone utilities.
 */

import {
  HarnessState,
  MemoryCheckpointStore,
  cloneHarnessCheckpoint,
} from "../harness-state.js";
import type { HarnessCheckpoint, CheckpointStore } from "../harness-state.js";
import { SessionId, TokenUsage } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeCheckpoint(overrides: Partial<HarnessCheckpoint> = {}): HarnessCheckpoint {
  return {
    runID: "run-1",
    sessionID: SessionId.create(),
    task: "test task",
    state: HarnessState.Init,
    iteration: 0,
    toolCallsMade: 0,
    tokenUsage: TokenUsage.create(0, 0),
    createdAt: new Date(),
    updatedAt: new Date(),
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// HarnessState constants
// ---------------------------------------------------------------------------

describe("HarnessState", () => {
  it("has all expected state values", () => {
    expect(HarnessState.Init).toBe("init");
    expect(HarnessState.BuildContext).toBe("build_context");
    expect(HarnessState.ModelCall).toBe("model_call");
    expect(HarnessState.GuardrailCheck).toBe("guardrail_check");
    expect(HarnessState.ToolDispatch).toBe("tool_dispatch");
    expect(HarnessState.Observe).toBe("observe");
    expect(HarnessState.MemoryUpdate).toBe("memory_update");
    expect(HarnessState.Checkpoint).toBe("checkpoint");
    expect(HarnessState.DecideNext).toBe("decide_next");
    expect(HarnessState.Completed).toBe("completed");
    expect(HarnessState.Stopped).toBe("stopped");
    expect(HarnessState.Failed).toBe("failed");
  });
});

// ---------------------------------------------------------------------------
// MemoryCheckpointStore
// ---------------------------------------------------------------------------

describe("MemoryCheckpointStore", () => {
  it("saves and loads a checkpoint", async () => {
    const store = new MemoryCheckpointStore();
    const cp = makeCheckpoint({ runID: "test-run" });

    await store.save(undefined, cp);
    const loaded = await store.load(undefined, "test-run");

    expect(loaded.runID).toBe("test-run");
    expect(loaded.task).toBe("test task");
    expect(loaded.state).toBe(HarnessState.Init);
  });

  it("throws when loading a non-existent checkpoint", async () => {
    const store = new MemoryCheckpointStore();
    await expect(store.load(undefined, "missing")).rejects.toThrow("not found");
  });

  it("throws when deleting a non-existent checkpoint", async () => {
    const store = new MemoryCheckpointStore();
    await expect(store.delete(undefined, "missing")).rejects.toThrow("not found");
  });

  it("deletes an existing checkpoint", async () => {
    const store = new MemoryCheckpointStore();
    await store.save(undefined, makeCheckpoint({ runID: "to-delete" }));
    await store.delete(undefined, "to-delete");
    await expect(store.load(undefined, "to-delete")).rejects.toThrow("not found");
  });

  it("lists checkpoints with prefix filter", async () => {
    const store = new MemoryCheckpointStore();
    await store.save(undefined, makeCheckpoint({ runID: "prefix-a-1" }));
    await store.save(undefined, makeCheckpoint({ runID: "prefix-a-2" }));
    await store.save(undefined, makeCheckpoint({ runID: "prefix-b-1" }));
    await store.save(undefined, makeCheckpoint({ runID: "other-1" }));

    const results = await store.list(undefined, "prefix-a");
    expect(results).toHaveLength(2);
    expect(results.every((r) => r.runID.startsWith("prefix-a"))).toBe(true);
  });

  it("lists all checkpoints when prefix is empty", async () => {
    const store = new MemoryCheckpointStore();
    await store.save(undefined, makeCheckpoint({ runID: "a" }));
    await store.save(undefined, makeCheckpoint({ runID: "b" }));
    await store.save(undefined, makeCheckpoint({ runID: "c" }));

    const results = await store.list(undefined, "");
    expect(results).toHaveLength(3);
  });

  it("overwrites existing checkpoint on save", async () => {
    const store = new MemoryCheckpointStore();
    const cp = makeCheckpoint({ runID: "overwrite", iteration: 1 });
    await store.save(undefined, cp);

    cp.iteration = 5;
    await store.save(undefined, cp);

    const loaded = await store.load(undefined, "overwrite");
    expect(loaded.iteration).toBe(5);
  });

  it("save updates the updatedAt timestamp", async () => {
    const store = new MemoryCheckpointStore();
    const cp = makeCheckpoint({ runID: "ts-test" });
    const before = new Date();

    await store.save(undefined, cp);
    const loaded = await store.load(undefined, "ts-test");

    expect(loaded.updatedAt.getTime()).toBeGreaterThanOrEqual(before.getTime());
  });
});

// ---------------------------------------------------------------------------
// cloneHarnessCheckpoint
// ---------------------------------------------------------------------------

describe("cloneHarnessCheckpoint", () => {
  it("creates a deep copy of the checkpoint", () => {
    const original = makeCheckpoint({
      contextBlocks: [{ kind: "system", content: "hello", stable: true, priority: 1, tokenEstimate: 5 }],
      memoryKeys: ["key1", "key2"],
      recentToolKeys: ["tool1"],
      trace: [
        { from: HarnessState.Init, to: HarnessState.BuildContext, createdAt: new Date() },
      ],
    });

    const cloned = cloneHarnessCheckpoint(original);

    // Values match
    expect(cloned.runID).toBe(original.runID);
    expect(cloned.contextBlocks).toHaveLength(1);
    expect(cloned.memoryKeys).toEqual(["key1", "key2"]);
    expect(cloned.trace).toHaveLength(1);

    // But references are different
    expect(cloned.contextBlocks).not.toBe(original.contextBlocks);
    expect(cloned.memoryKeys).not.toBe(original.memoryKeys);
    expect(cloned.recentToolKeys).not.toBe(original.recentToolKeys);
    expect(cloned.trace).not.toBe(original.trace);
  });

  it("handles undefined optional fields gracefully", () => {
    const cp = makeCheckpoint();
    const cloned = cloneHarnessCheckpoint(cp);
    expect(cloned.contextBlocks).toBeUndefined();
    expect(cloned.memoryKeys).toBeUndefined();
    expect(cloned.trace).toBeUndefined();
  });
});
