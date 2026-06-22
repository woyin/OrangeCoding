/**
 * Tests for the orchestrator package — Orchestrator (workflow registry),
 * MakerChecker, and types.
 */

import { Orchestrator } from "../workflow.js";
import { MakerChecker } from "../maker-checker.js";
import {
  ReviewVerdict,
  OrchestratorError,
  newOrchestratorError,
  DEFAULT_MAKER_CHECKER_CONFIG,
} from "../types.js";
import type { LoopWorkflowDefinition, WorkflowTrigger } from "../types.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeWorkflow(overrides: Partial<LoopWorkflowDefinition> = {}): LoopWorkflowDefinition {
  return {
    name: "test-workflow",
    description: "A test workflow",
    trigger: { type: "manual", task: "do something" },
    task: "Execute the task",
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Orchestrator — workflow registration
// ---------------------------------------------------------------------------

describe("Orchestrator — registration", () => {
  it("registers a workflow", () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow());
    expect(orch.get("test-workflow")).toBeDefined();
  });

  it("throws on duplicate registration", () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow());
    expect(() => orch.register(makeWorkflow())).toThrow("already registered");
  });

  it("unregisters a workflow", () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow());
    orch.unregister("test-workflow");
    expect(orch.get("test-workflow")).toBeUndefined();
  });

  it("lists all registered workflows", () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow({ name: "wf-1" }));
    orch.register(makeWorkflow({ name: "wf-2" }));
    orch.register(makeWorkflow({ name: "wf-3" }));

    const list = orch.list();
    expect(list).toHaveLength(3);
    expect(list.map((w) => w.name)).toEqual(
      expect.arrayContaining(["wf-1", "wf-2", "wf-3"]),
    );
  });

  it("returns undefined for non-existent workflow", () => {
    const orch = new Orchestrator();
    expect(orch.get("non-existent")).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// Orchestrator — execution
// ---------------------------------------------------------------------------

describe("Orchestrator — execution", () => {
  it("executes a workflow via onExecuteTask callback", async () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow({ name: "wf-1", task: "build the thing" }));

    let receivedTask = "";
    orch.onExecuteTask = async (task: string) => {
      receivedTask = task;
      return "done";
    };

    const result = await orch.execute("wf-1");
    expect(result).toBe("done");
    expect(receivedTask).toBe("build the thing");
  });

  it("throws when executing non-existent workflow", async () => {
    const orch = new Orchestrator();
    orch.onExecuteTask = async () => "result";
    await expect(orch.execute("nope")).rejects.toThrow("not found");
  });

  it("throws when onExecuteTask is not set", async () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow());
    await expect(orch.execute("test-workflow")).rejects.toThrow("onExecuteTask");
  });

  it("calls onWorkflowComplete on success", async () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow());
    orch.onExecuteTask = async () => "success result";

    let completedName = "";
    orch.onWorkflowComplete = (name) => { completedName = name; };

    await orch.execute("test-workflow");
    expect(completedName).toBe("test-workflow");
  });

  it("calls onWorkflowFail on error and rethrows", async () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow());
    orch.onExecuteTask = async () => {
      throw new Error("task failed");
    };

    let failedName = "";
    orch.onWorkflowFail = (name) => { failedName = name; };

    await expect(orch.execute("test-workflow")).rejects.toThrow("task failed");
    expect(failedName).toBe("test-workflow");
  });

  it("uses provided trigger override", async () => {
    const orch = new Orchestrator();
    orch.register(makeWorkflow());

    let receivedTrigger: WorkflowTrigger | undefined;
    orch.onExecuteTask = async (_task, trigger) => {
      receivedTrigger = trigger;
      return "ok";
    };

    const override: WorkflowTrigger = { type: "cron", cron: "0 0 * * *" };
    await orch.execute("test-workflow", override);
    expect(receivedTrigger).toEqual(override);
  });
});

// ---------------------------------------------------------------------------
// MakerChecker — basic execution
// ---------------------------------------------------------------------------

describe("MakerChecker", () => {
  it("executes and returns approved result", async () => {
    const mc = new MakerChecker({
      maker: { provider: {} },
      checker: { provider: {} },
    });

    const result = await mc.execute("build a feature");
    expect(result.success).toBe(true);
    expect(result.finalVerdict).toBe(ReviewVerdict.Approved);
    expect(result.iterations).toBeGreaterThanOrEqual(1);
    expect(result.makerOutput).toBeTruthy();
    expect(result.checkerReports.length).toBeGreaterThanOrEqual(1);
  });

  it("calls onIterationStart callback", async () => {
    const mc = new MakerChecker({
      maker: { provider: {} },
      checker: { provider: {} },
    });

    const iterations: number[] = [];
    mc.onIterationStart = (i) => iterations.push(i);

    await mc.execute("task");
    expect(iterations.length).toBeGreaterThanOrEqual(1);
  });

  it("calls onReviewCompleted callback", async () => {
    const mc = new MakerChecker({
      maker: { provider: {} },
      checker: { provider: {} },
    });

    let reviewCalled = false;
    mc.onReviewCompleted = () => { reviewCalled = true; };

    await mc.execute("task");
    expect(reviewCalled).toBe(true);
  });

  it("respects maxReviewIterations config", async () => {
    const mc = new MakerChecker({
      maker: { provider: {} },
      checker: { provider: {} },
      maxReviewIterations: 2,
    });

    const result = await mc.execute("task");
    expect(result.iterations).toBeLessThanOrEqual(2);
  });

  it("rejects when human approval is denied", async () => {
    const mc = new MakerChecker({
      maker: { provider: {} },
      checker: { provider: {} },
      requireHumanApproval: true,
    });

    mc.onHumanApprovalRequested = async () => false;

    const result = await mc.execute("task");
    expect(result.success).toBe(false);
    expect(result.finalVerdict).toBe(ReviewVerdict.Rejected);
  });
});

// ---------------------------------------------------------------------------
// Types — error constructors and constants
// ---------------------------------------------------------------------------

describe("Orchestrator types", () => {
  it("ReviewVerdict has all expected values", () => {
    expect(ReviewVerdict.Approved).toBe("approved");
    expect(ReviewVerdict.ChangesNeeded).toBe("changes_needed");
    expect(ReviewVerdict.Rejected).toBe("rejected");
  });

  it("newOrchestratorError creates an OrchestratorError with code", () => {
    const err = newOrchestratorError("TEST", "test message", { extra: true });
    expect(err).toBeInstanceOf(OrchestratorError);
    expect(err.code).toBe("TEST");
    expect(err.message).toBe("test message");
    expect(err.name).toBe("OrchestratorError");
  });

  it("DEFAULT_MAKER_CHECKER_CONFIG has sensible defaults", () => {
    expect(DEFAULT_MAKER_CHECKER_CONFIG.maxReviewIterations).toBe(3);
    expect(DEFAULT_MAKER_CHECKER_CONFIG.requireHumanApproval).toBe(false);
  });
});
