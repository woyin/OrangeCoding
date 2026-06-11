import { jest } from "@jest/globals";
import { AgentId, GuardrailDecisionEvent, SessionId, type AgentEvent } from "@orangecoding/core";
import { WorkerRuntime } from "../runtime.js";

function waitFor(predicate: () => boolean, timeoutMs = 1000): Promise<void> {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const timer = setInterval(() => {
      if (predicate()) {
        clearInterval(timer);
        resolve();
        return;
      }
      if (Date.now() - started > timeoutMs) {
        clearInterval(timer);
        reject(new Error("condition not met within timeout"));
      }
    }, 5);
  });
}

/** StubLoop records each run call and emits one event per run. */
class StubLoop {
  public runs: Array<{ task: string }> = [];
  public context = {
    addUserMessage: (content: string): void => {
      this.runs.push({ task: content });
    },
  };

  async run(_opts: unknown, eventCb: ((event: AgentEvent) => void) | null): Promise<unknown> {
    eventCb?.(new GuardrailDecisionEvent(
      AgentId.create(),
      SessionId.create(),
      "pre_tool",
      "deny",
      "dangerous shell command",
      "dangerous_tool",
    ));
    return { toolCallsMade: 0, tokensUsed: { promptTokens: 0, completionTokens: 0, totalTokens: 0 }, durationMs: 0, stopReason: "completed", progress: [] };
  }
}

describe("WorkerRuntime", () => {
  it("passes AgentLoop events to the configured runtime agent event handler", async () => {
    const received: AgentEvent[] = [];
    const runtime = new WorkerRuntime(null, (event) => {
      received.push(event);
    });

    runtime.startSession("session-1", new StubLoop() as never);
    // Submit a task to trigger the agent loop
    runtime.submitTask("session-1", "test task");
    await waitFor(() => received.length === 1);

    expect(received[0]).toMatchObject({
      eventType: "guardrail_decision",
      phase: "pre_tool",
      decision: "deny",
    });

    runtime.shutdown();
  });

  it("records submitted tasks for active sessions", async () => {
    const runtime = new WorkerRuntime(null);

    runtime.startSession("session-1", null);
    runtime.submitTask("session-1", "fix bug");

    expect(runtime.pendingTasksFor("session-1")).toEqual(["fix bug"]);
    expect(() => runtime.submitTask("missing", "task")).toThrow(/not found/);

    runtime.shutdown();
  });

  it("forwards submitted tasks to the executor for processing", async () => {
    const received: AgentEvent[] = [];
    const runtime = new WorkerRuntime(null, (event) => {
      received.push(event);
    });

    const loop = new StubLoop();
    runtime.startSession("session-1", loop as never);

    // Submit a task — should be forwarded to executor
    runtime.submitTask("session-1", "implement feature X");
    await waitFor(() => received.length >= 1);

    expect(loop.runs).toHaveLength(1);
    expect(loop.runs[0]!.task).toBe("implement feature X");

    runtime.shutdown();
  });
});
