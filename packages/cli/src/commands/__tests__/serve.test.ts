import { jest } from "@jest/globals";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { AgentId, GuardrailDecisionEvent, SessionId } from "@orangecoding/core";
import { defaultConfig } from "@orangecoding/config";
import { createServeRuntime } from "../serve.js";

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

describe("createServeRuntime", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "orangecoding-serve-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("wires audit recorder into worker agent events when audit is enabled", async () => {
    const cfg = defaultConfig();
    cfg.audit = {
      enabled: true,
      dir,
    };

    const runtime = await createServeRuntime(cfg);
    const agentId = AgentId.create();
    const sessionId = SessionId.create();

    await runtime.agentEventHandler?.(new GuardrailDecisionEvent(
      agentId,
      sessionId,
      "pre_tool",
      "deny",
      "dangerous shell command",
      "dangerous_tool",
    ));

    const entries = await runtime.auditLog!.getEntries();
    expect(entries).toHaveLength(1);
    expect(entries[0]!.action).toBe("guardrail_decision");
    expect(entries[0]!.agentId).toBe(agentId.toString());
  });

  it("creates a startable control server backed by the serve runtime", async () => {
    const cfg = defaultConfig();
    cfg.audit = {
      enabled: false,
      dir,
    };

    const runtime = await createServeRuntime(cfg, ":0");
    await runtime.server.start();

    try {
      const address = runtime.server.getHttpServer()!.address();
      if (address === null || typeof address === "string") {
        throw new Error("expected TCP server address");
      }

      const response = await fetch(`http://127.0.0.1:${address.port}/status`);
      const body = await response.json() as { status: string };

      expect(response.status).toBe(200);
      expect(body.status).toBe("running");
    } finally {
      await runtime.server.stop();
    }
  });

  it("HTTP task submission flows through to the worker executor", async () => {
    const cfg = defaultConfig();
    cfg.audit = {
      enabled: false,
      dir,
    };

    const runtime = await createServeRuntime(cfg, ":0");
    await runtime.server.start();

    try {
      const address = runtime.server.getHttpServer()!.address();
      if (address === null || typeof address === "string") {
        throw new Error("expected TCP server address");
      }
      const base = `http://127.0.0.1:${address.port}`;

      // 1. Create a session
      const createRes = await fetch(`${base}/sessions`, { method: "POST" });
      expect(createRes.status).toBe(201);
      const createBody = await createRes.json() as { session_id: string; status: string };
      const sessionId = createBody.session_id;

      // Wait for the executor to enter idle (ready for tasks)
      await waitFor(() => {
        const [status, found] = runtime.workerRuntime.getStatus(sessionId);
        return found && status === "idle";
      });

      // 2. Submit a task via HTTP
      const taskRes = await fetch(`${base}/sessions/${sessionId}/task`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ task: "implement feature Y" }),
      });
      expect(taskRes.status).toBe(200);

      const taskBody = await taskRes.json() as { session_id: string; status: string };
      expect(taskBody.session_id).toBe(sessionId);
      expect(taskBody.status).toBe("task_sent");

      // 3. Verify the task was recorded in the worker runtime
      expect(runtime.workerRuntime.pendingTasksFor(sessionId)).toEqual([
        "implement feature Y",
      ]);
    } finally {
      await runtime.server.stop();
    }
  });
});
