/**
 * Tests for the goal package — GoalEngine, GoalEvaluator (fallback),
 * MemoryGoalStore, and types.
 */

import { jest } from "@jest/globals";
import { GoalEngine } from "../goal.js";
import { GoalEvaluator } from "../evaluator.js";
import { MemoryGoalStore } from "../store.js";
import { GoalStatus, GoalError, newGoalError, DEFAULT_GOAL_ENGINE_CONFIG } from "../types.js";
import type { GoalConfig, GoalState, EvaluationResult } from "../types.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeGoalConfig(overrides: Partial<GoalConfig> = {}): GoalConfig {
  return {
    id: "test-goal",
    description: "Test goal",
    condition: "all tests pass",
    ...overrides,
  };
}

function makeEvalResult(overrides: Partial<EvaluationResult> = {}): EvaluationResult {
  return {
    completed: false,
    confidence: 0.5,
    reason: "not yet",
    remainingBlockers: [],
    suggestions: [],
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// GoalEngine — lifecycle
// ---------------------------------------------------------------------------

describe("GoalEngine", () => {
  it("throws when onExecuteIteration is not set", async () => {
    const evaluator = new GoalEvaluator();
    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, {}, store);

    await expect(engine.start(makeGoalConfig())).rejects.toThrow("onExecuteIteration");
  });

  it("completes when evaluator says goal is met", async () => {
    const evaluator = new GoalEvaluator();
    // Override evaluate to always return completed
    jest.spyOn(evaluator, "evaluate").mockResolvedValue(
      makeEvalResult({ completed: true, confidence: 0.8 }),
    );

    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, {}, store);
    engine.onExecuteIteration = async () => "done";

    const result = await engine.start(makeGoalConfig());
    expect(result.success).toBe(true);
    expect(result.finalStatus).toBe(GoalStatus.Completed);
    expect(result.iterations).toBe(1);
  });

  it("iterates multiple times before completion", async () => {
    const evaluator = new GoalEvaluator();
    let callCount = 0;
    jest.spyOn(evaluator, "evaluate").mockImplementation(async () => {
      callCount++;
      if (callCount >= 3) {
        return makeEvalResult({ completed: true, confidence: 0.9 });
      }
      return makeEvalResult({ completed: false, confidence: 0.3 });
    });

    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, {}, store);
    engine.onExecuteIteration = async () => `iteration ${callCount}`;

    const result = await engine.start(makeGoalConfig());
    expect(result.success).toBe(true);
    expect(result.iterations).toBe(3);
  });

  it("fails when max iterations reached", async () => {
    const evaluator = new GoalEvaluator();
    jest.spyOn(evaluator, "evaluate").mockResolvedValue(
      makeEvalResult({ completed: false, confidence: 0.2 }),
    );

    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, { defaultMaxIterations: 3 }, store);
    engine.onExecuteIteration = async () => "not done yet";

    const result = await engine.start(makeGoalConfig());
    expect(result.success).toBe(false);
    expect(result.finalStatus).toBe(GoalStatus.Failed);
    expect(result.iterations).toBe(3);
  });

  it("fails when executor throws", async () => {
    const evaluator = new GoalEvaluator();
    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, {}, store);
    engine.onExecuteIteration = async () => {
      throw new Error("executor crashed");
    };

    const result = await engine.start(makeGoalConfig());
    expect(result.success).toBe(false);
    expect(result.finalStatus).toBe(GoalStatus.Failed);
  });

  it("cancels via abort signal", async () => {
    const evaluator = new GoalEvaluator();
    jest.spyOn(evaluator, "evaluate").mockResolvedValue(
      makeEvalResult({ completed: false, confidence: 0.1 }),
    );

    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, { defaultMaxIterations: 100 }, store);

    const controller = new AbortController();
    let iterCount = 0;
    engine.onExecuteIteration = async () => {
      iterCount++;
      if (iterCount >= 2) controller.abort();
      return "working...";
    };

    const result = await engine.start(makeGoalConfig(), controller.signal);
    expect(result.success).toBe(false);
    expect(result.finalStatus).toBe(GoalStatus.Failed);
  });

  it("calls onIteration callback after each iteration", async () => {
    const evaluator = new GoalEvaluator();
    let evalCount = 0;
    jest.spyOn(evaluator, "evaluate").mockImplementation(async () => {
      evalCount++;
      return makeEvalResult({
        completed: evalCount >= 2,
        confidence: evalCount >= 2 ? 0.9 : 0.2,
      });
    });

    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, {}, store);
    engine.onExecuteIteration = async () => "output";

    const iterationStates: GoalState[] = [];
    engine.onIteration = (state) => iterationStates.push({ ...state });

    await engine.start(makeGoalConfig());
    expect(iterationStates).toHaveLength(2);
  });

  it("calls onCompleted callback when goal succeeds", async () => {
    const evaluator = new GoalEvaluator();
    jest.spyOn(evaluator, "evaluate").mockResolvedValue(
      makeEvalResult({ completed: true, confidence: 0.9 }),
    );

    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, {}, store);
    engine.onExecuteIteration = async () => "done";

    let completedCalled = false;
    engine.onCompleted = () => { completedCalled = true; };

    await engine.start(makeGoalConfig());
    expect(completedCalled).toBe(true);
  });

  it("calls onFailed callback when goal fails", async () => {
    const evaluator = new GoalEvaluator();
    jest.spyOn(evaluator, "evaluate").mockResolvedValue(
      makeEvalResult({ completed: false, confidence: 0.1 }),
    );

    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, { defaultMaxIterations: 1 }, store);
    engine.onExecuteIteration = async () => "not done";

    let failedCalled = false;
    engine.onFailed = () => { failedCalled = true; };

    await engine.start(makeGoalConfig());
    expect(failedCalled).toBe(true);
  });

  it("respects maxConcurrentGoals limit", async () => {
    const evaluator = new GoalEvaluator();
    jest.spyOn(evaluator, "evaluate").mockImplementation(
      () => new Promise(() => {}), // never resolves — blocks
    );

    const store = new MemoryGoalStore();
    const engine = new GoalEngine(evaluator, { maxConcurrentGoals: 1 }, store);
    engine.onExecuteIteration = async () => {
      await new Promise((r) => setTimeout(r, 10000));
      return "slow";
    };

    // Start first goal (don't await — it will block)
    const p1 = engine.start(makeGoalConfig({ id: "goal-1" }));

    // Wait a bit for the first goal to be registered
    await new Promise((r) => setTimeout(r, 10));

    // Second goal should fail
    await expect(engine.start(makeGoalConfig({ id: "goal-2" }))).rejects.toThrow("maximum concurrent goals reached");

    // Clean up
    await engine.cancel("goal-1");
  });
});

// ---------------------------------------------------------------------------
// GoalEvaluator — fallback evaluation
// ---------------------------------------------------------------------------

describe("GoalEvaluator — fallback", () => {
  it("returns not completed when no provider is set and output has errors", async () => {
    const evaluator = new GoalEvaluator();
    const result = await evaluator.evaluate("tests pass", {
      iteration: 1,
      recentOutput: "Error: test failed",
    });
    expect(result.completed).toBe(false);
    expect(result.confidence).toBeLessThanOrEqual(0.5);
  });

  it("returns completed when success indicators present after sufficient iterations", async () => {
    const evaluator = new GoalEvaluator();
    const result = await evaluator.evaluate("tests pass", {
      iteration: 5,
      recentOutput: "All tests pass, success!",
    });
    expect(result.completed).toBe(true);
    expect(result.confidence).toBeGreaterThanOrEqual(0.5);
  });

  it("returns low confidence when evidence is insufficient", async () => {
    const evaluator = new GoalEvaluator();
    const result = await evaluator.evaluate("tests pass", {
      iteration: 1,
      recentOutput: "started working on it",
    });
    expect(result.completed).toBe(false);
    expect(result.confidence).toBeLessThan(0.5);
  });
});

// ---------------------------------------------------------------------------
// GoalEvaluator — summarize
// ---------------------------------------------------------------------------

describe("GoalEvaluator.summarize", () => {
  it("formats a completed result", () => {
    const summary = GoalEvaluator.summarize({
      completed: true,
      confidence: 0.95,
      reason: "All tests pass",
      remainingBlockers: [],
      suggestions: [],
    });
    expect(summary).toContain("Completed: true");
    expect(summary).toContain("0.95");
  });

  it("includes blockers and suggestions when present", () => {
    const summary = GoalEvaluator.summarize({
      completed: false,
      confidence: 0.3,
      reason: "Not done",
      remainingBlockers: ["Missing tests"],
      suggestions: ["Add unit tests"],
    });
    expect(summary).toContain("Blockers: Missing tests");
    expect(summary).toContain("Suggestions: Add unit tests");
  });
});

// ---------------------------------------------------------------------------
// MemoryGoalStore
// ---------------------------------------------------------------------------

describe("MemoryGoalStore", () => {
  it("saves and loads goal state", async () => {
    const store = new MemoryGoalStore();
    const state: GoalState = {
      id: "g1",
      config: makeGoalConfig(),
      status: GoalStatus.Active,
      iteration: 0,
      totalTokensUsed: 0,
      lastEvalResult: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    };

    await store.save(state);
    const loaded = await store.load("g1");
    expect(loaded).not.toBeNull();
    expect(loaded!.id).toBe("g1");
  });

  it("returns null for non-existent goal", async () => {
    const store = new MemoryGoalStore();
    const loaded = await store.load("nope");
    expect(loaded).toBeNull();
  });

  it("deletes a goal", async () => {
    const store = new MemoryGoalStore();
    await store.save({
      id: "g1",
      config: makeGoalConfig(),
      status: GoalStatus.Active,
      iteration: 0,
      totalTokensUsed: 0,
      lastEvalResult: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    });

    await store.delete("g1");
    expect(await store.load("g1")).toBeNull();
  });

  it("lists all stored goal IDs", async () => {
    const store = new MemoryGoalStore();
    await store.save({
      id: "g1",
      config: makeGoalConfig(),
      status: GoalStatus.Active,
      iteration: 0,
      totalTokensUsed: 0,
      lastEvalResult: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    });
    await store.save({
      id: "g2",
      config: makeGoalConfig({ id: "g2" }),
      status: GoalStatus.Active,
      iteration: 0,
      totalTokensUsed: 0,
      lastEvalResult: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    });

    const ids = await store.list();
    expect(ids).toEqual(expect.arrayContaining(["g1", "g2"]));
    expect(ids).toHaveLength(2);
  });

  it("save stores a copy (mutations do not affect store)", async () => {
    const store = new MemoryGoalStore();
    const state: GoalState = {
      id: "g1",
      config: makeGoalConfig(),
      status: GoalStatus.Active,
      iteration: 0,
      totalTokensUsed: 0,
      lastEvalResult: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    };

    await store.save(state);
    state.status = GoalStatus.Completed;

    const loaded = await store.load("g1");
    expect(loaded!.status).toBe(GoalStatus.Active);
  });
});

// ---------------------------------------------------------------------------
// Types — error constructors and constants
// ---------------------------------------------------------------------------

describe("Goal types", () => {
  it("GoalStatus has all expected values", () => {
    expect(GoalStatus.Active).toBe("active");
    expect(GoalStatus.Completed).toBe("completed");
    expect(GoalStatus.Failed).toBe("failed");
    expect(GoalStatus.Paused).toBe("paused");
  });

  it("newGoalError creates a GoalError with code", () => {
    const err = newGoalError("TEST_CODE", "test message");
    expect(err).toBeInstanceOf(GoalError);
    expect(err.code).toBe("TEST_CODE");
    expect(err.message).toBe("test message");
    expect(err.name).toBe("GoalError");
  });

  it("DEFAULT_GOAL_ENGINE_CONFIG has sensible defaults", () => {
    expect(DEFAULT_GOAL_ENGINE_CONFIG.defaultMaxIterations).toBe(50);
    expect(DEFAULT_GOAL_ENGINE_CONFIG.maxConcurrentGoals).toBe(3);
    expect(DEFAULT_GOAL_ENGINE_CONFIG.storeDir).toBe(".claude/goals");
  });
});
