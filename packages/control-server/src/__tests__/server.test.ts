import { Server, type WorkerRuntime } from "../server.js";

class EmptyWorkers implements WorkerRuntime {
  public submittedTasks: Array<{ sessionId: string; task: string }> = [];

  startSession(_sessionId: string): void {}
  stopSession(_sessionId: string): void {}
  listSessions(): string[] { return []; }
  getStatus(_sessionId: string): [status: string, found: boolean] { return ["", false]; }
  submitTask(sessionId: string, task: string): void {
    this.submittedTasks.push({ sessionId, task });
  }
  shutdown(): void {}
}

describe("Server", () => {
  it("stops promptly after handling a keep-alive request", async () => {
    const server = new Server({ workers: new EmptyWorkers(), addr: ":0" });
    await server.start();

    const address = server.getHttpServer()!.address();
    if (address === null || typeof address === "string") {
      throw new Error("expected TCP server address");
    }

    const response = await fetch(`http://127.0.0.1:${address.port}/status`);
    expect(response.status).toBe(200);

    const started = Date.now();
    await server.stop();

    expect(Date.now() - started).toBeLessThan(1000);
  });

  it("submits task payloads to the worker runtime", async () => {
    const workers = new EmptyWorkers();
    workers.startSession = (_sessionId: string): void => {};
    workers.getStatus = (_sessionId: string): [status: string, found: boolean] => ["running", true];
    const server = new Server({ workers, addr: ":0" });
    await server.start();

    try {
      const address = server.getHttpServer()!.address();
      if (address === null || typeof address === "string") {
        throw new Error("expected TCP server address");
      }

      const response = await fetch(`http://127.0.0.1:${address.port}/sessions/session-1/task`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ task: "fix bug" }),
      });

      expect(response.status).toBe(200);
      expect(workers.submittedTasks).toEqual([
        { sessionId: "session-1", task: "fix bug" },
      ]);
    } finally {
      await server.stop();
    }
  });

  it("responds to tool approval requests via HTTP", async () => {
    const handler = {
      respond: (requestId: string, approved: boolean): boolean => {
        if (requestId === "req-123") return true;
        return false;
      },
    };
    const workers = new EmptyWorkers();
    const server = new Server({ workers, addr: ":0", approvalHandler: handler });
    await server.start();

    try {
      const address = server.getHttpServer()!.address();
      if (address === null || typeof address === "string") {
        throw new Error("expected TCP server address");
      }

      // Approve a valid request
      const res = await fetch(`http://127.0.0.1:${address.port}/sessions/s1/approve`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ requestId: "req-123", approved: true }),
      });
      expect(res.status).toBe(200);

      // Reject an unknown request
      const res2 = await fetch(`http://127.0.0.1:${address.port}/sessions/s1/approve`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ requestId: "unknown", approved: false }),
      });
      expect(res2.status).toBe(404);

      // Reject missing requestId
      const res3 = await fetch(`http://127.0.0.1:${address.port}/sessions/s1/approve`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ approved: true }),
      });
      expect(res3.status).toBe(400);
    } finally {
      await server.stop();
    }
  });

});
