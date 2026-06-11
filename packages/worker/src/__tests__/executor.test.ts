import { jest } from "@jest/globals";
import { AgentId, GuardrailDecisionEvent, SessionId, type AgentEvent } from "@orangecoding/core";
import { AgentExecutor } from "../executor.js";

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

describe("AgentExecutor", () => {
  it("waits for tasks and processes them through the agent loop", async () => {
    const received: AgentEvent[] = [];
    const loop = new StubLoop();
    const executor = new AgentExecutor("session-1", loop as never);
    executor.setAgentEventHandler((event) => { received.push(event); });

    const controller = new AbortController();
    const runPromise = executor.run(controller.signal);

    // Executor should be idle, waiting for a task
    await waitFor(() => executor.status === "idle");

    // Submit a task
    executor.submitTask("fix the bug");

    // Wait for the event to be forwarded
    await waitFor(() => received.length >= 1);

    expect(received[0]).toMatchObject({
      eventType: "guardrail_decision",
      phase: "pre_tool",
      decision: "deny",
      reason: "dangerous shell command",
    });

    // The loop should have been called with the task as user message
    expect(loop.runs).toHaveLength(1);
    expect(loop.runs[0]!.task).toBe("fix the bug");

    // Cancel and wait for clean shutdown
    controller.abort();
    await runPromise.catch(() => {});
  });

  it("processes multiple tasks sequentially", async () => {
    const received: AgentEvent[] = [];
    const loop = new StubLoop();
    const executor = new AgentExecutor("session-1", loop as never);
    executor.setAgentEventHandler((event) => { received.push(event); });

    const controller = new AbortController();
    const runPromise = executor.run(controller.signal);

    // Submit two tasks
    executor.submitTask("task one");
    executor.submitTask("task two");

    // Wait for both tasks to be processed
    await waitFor(() => received.length >= 2);

    expect(loop.runs).toHaveLength(2);
    expect(loop.runs[0]!.task).toBe("task one");
    expect(loop.runs[1]!.task).toBe("task two");

    controller.abort();
    await runPromise.catch(() => {});
  });

  it("completes immediately when loop is null and no tasks submitted", async () => {
    const executor = new AgentExecutor("session-1", null);
    const controller = new AbortController();
    const runPromise = executor.run(controller.signal);

    // Null loop executor should be idle
    await waitFor(() => executor.status === "idle");

    controller.abort();
    await runPromise.catch(() => {});
  });
});
