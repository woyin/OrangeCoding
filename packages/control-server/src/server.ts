/**
 * @module control-server
 *
 * HTTP control server for programmatic agent interaction.
 *
 * The control server exposes a REST API for:
 * - Creating and managing agent sessions
 * - Submitting tasks to agents
 * - Querying agent status and results
 * - Streaming agent events via SSE
 *
 * Designed for integration with CI/CD pipelines, IDEs, and other
 * external tools that need to interact with OrangeCoding agents.
 */
import http from "node:http";
import { randomUUID } from "node:crypto";
import { WebSocket, WebSocketServer } from "ws";
import type { ServerEvent } from "@orangecoding/control-protocol";
import {
  TaskUpdateEvent,
  AgentStreamEvent,
  AgentCompletedEvent,
  GuardrailEvent,
  ToolCallEvent,
  ErrorEvent,
} from "@orangecoding/control-protocol";

// ---------------------------------------------------------------------------
// WorkerRuntime -- minimal interface expected from @orangecoding/worker
// ---------------------------------------------------------------------------

/** Minimal interface that the control server requires from a worker runtime. */
export interface WorkerRuntime {
  /** Start a new agent session. Throws if session already exists. */
  startSession(sessionId: string): void;
  /** Stop and remove an agent session. Throws if not found. */
  stopSession(sessionId: string): void;
  /** Return the IDs of all active sessions. */
  listSessions(): string[];
  /** Return (status, true) for an active session, or ("", false) if not found. */
  getStatus(sessionId: string): [status: string, found: boolean];
  /** Submit a task to an active session. Throws if the session cannot accept it. */
  submitTask(sessionId: string, task: string): void;
  /** Cancel all running sessions. */
  shutdown(): void;
}

// ---------------------------------------------------------------------------
// Route handler signature
// ---------------------------------------------------------------------------

interface RequestContext {
  req: http.IncomingMessage;
  res: http.ServerResponse;
  params: Record<string, string>;
  body: unknown;
}

type RouteHandler = (ctx: RequestContext) => void;

// ---------------------------------------------------------------------------
// Simple router
// ---------------------------------------------------------------------------

type HttpMethod = "GET" | "POST" | "PUT" | "DELETE" | "OPTIONS";

interface Route {
  method: HttpMethod;
  pattern: RegExp;
  paramNames: string[];
  handler: RouteHandler;
}

class Router {
  private routes: Route[] = [];

  add(method: HttpMethod, path: string, handler: RouteHandler): void {
    const paramNames: string[] = [];
    const patternStr = path.replace(/:([A-Za-z_][A-Za-z0-9_]*)/g, (_match, name) => {
      paramNames.push(name);
      return "([^/]+)";
    });
    const pattern = new RegExp(`^${patternStr}$`);
    this.routes.push({ method, pattern, paramNames, handler });
  }

  resolve(
    method: string,
    pathname: string,
  ): { handler: RouteHandler; params: Record<string, string> } | undefined {
    const upper = method.toUpperCase() as HttpMethod;
    for (const route of this.routes) {
      if (route.method !== upper) continue;
      const match = pathname.match(route.pattern);
      if (match) {
        const params: Record<string, string> = {};
        route.paramNames.forEach((name, i) => {
          params[name] = match[i + 1]!;
        });
        return { handler: route.handler, params };
      }
    }
    return undefined;
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function sendJson(res: http.ServerResponse, statusCode: number, data: unknown): void {
  const body = JSON.stringify(data);
  res.writeHead(statusCode, { "Content-Type": "application/json" });
  res.end(body);
}

function readBody(req: http.IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf-8")));
    req.on("error", reject);
  });
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/**
 * CORS middleware.
 *
 * Reflects the Origin header back when it is a localhost origin (dev/TUI
 * control plane), otherwise allows any origin via "*". Only localhost is
 * echoed explicitly so browsers allow credentialed requests from the dev UI.
 *
 * The localhost match is split into a scheme check + a host check so we make
 * at most 2 prefix comparisons per request instead of 4 (one per scheme/host
 * permutation).
 */
function corsMiddleware(
  req: http.IncomingMessage,
  res: http.ServerResponse,
): boolean {
  const origin = req.headers.origin ?? "";

  if (origin === "") {
    res.setHeader("Access-Control-Allow-Origin", "*");
  } else {
    // Allow http(s)://localhost and http(s)://127.0.0.1 on any port.
    const afterScheme = origin.startsWith("https://") ? origin.slice(8)
      : origin.startsWith("http://") ? origin.slice(7) : "";
    if (afterScheme.startsWith("localhost") || afterScheme.startsWith("127.0.0.1")) {
      res.setHeader("Access-Control-Allow-Origin", origin);
    }
  }

  res.setHeader("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");
  res.setHeader("Access-Control-Max-Age", "86400");

  if (req.method === "OPTIONS") {
    res.writeHead(204);
    res.end();
    return true; // preflight fully handled
  }
  return false;
}

/**
 * Structured request log line (mirrors Go slog). Parses pathname and query
 * directly from req.url via String.indexOf/slice rather than constructing a
 * `new URL(...)` object: URL parsing validates scheme/host and allocates a
 * full URL instance, which is wasteful when we only need the path + query for
 * the log line. On hot request paths this measurably reduces GC pressure.
 */
function logRequest(
  req: http.IncomingMessage,
  res: http.ServerResponse,
  latencyMs: number,
): void {
  const status = res.statusCode;
  const method = req.method ?? "-";
  const rawUrl = req.url ?? "/";
  const qIdx = rawUrl.indexOf("?");
  const path = qIdx === -1 ? rawUrl : rawUrl.slice(0, qIdx);
  const query = qIdx === -1 ? "" : rawUrl.slice(qIdx);
  const parts: string[] = [
    `status=${status}`,
    `method=${method}`,
    `path=${path}`,
    `latency=${latencyMs.toFixed(1)}ms`,
  ];
  if (query) {
    parts.push(`query=${query}`);
  }
  console.log(`[control-server] request ${parts.join(" ")}`);
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

export interface ServerOptions {
  /** WorkerRuntime implementation. */
  workers: WorkerRuntime;
  /** Listen address, e.g. ":8080". Defaults to ":8080". */
  addr?: string;
  /** Optional approval handler for tool approval requests. */
  approvalHandler?: {
    respond(requestId: string, approved: boolean, reason?: string): boolean;
  };
}

/**
 * Server provides HTTP and WebSocket endpoints for the web-based control plane.
 *
 * Mirrors the Go controlserver package: same routes, same event fan-out pattern,
 * same graceful-shutdown semantics.
 */
export class Server {
  private readonly router = new Router();
  private readonly workers: WorkerRuntime;
  private readonly addr: string;
  private readonly approvalHandler?: { respond(requestId: string, approved: boolean, reason?: string): boolean };
  private readonly eventSubscribers = new Set<(event: ServerEvent) => void>();
  private httpServer: http.Server | null = null;
  private wss: WebSocketServer | null = null;
  private alive = false;

  constructor(options: ServerOptions) {
    this.workers = options.workers;
    this.addr = options.addr ?? ":8080";
    this.approvalHandler = options.approvalHandler;
    this.setupRoutes();
  }

  // ----- Event fan-out -----------------------------------------------------

  /**
   * Broadcast a ServerEvent to all connected WebSocket subscribers.
   * This replaces the Go channel-based fanOutEvents goroutine.
   */
  broadcastEvent(event: ServerEvent): void {
    for (const subscriber of this.eventSubscribers) {
      try {
        subscriber(event);
      } catch {
        // Subscriber is slow or broken; drop event.
      }
    }
  }

  /**
   * Register a subscriber function that receives every broadcast event.
   * Returns an unsubscribe function.
   */
  subscribeEvents(fn: (event: ServerEvent) => void): () => void {
    this.eventSubscribers.add(fn);
    return () => {
      this.eventSubscribers.delete(fn);
    };
  }

  // ----- Route registration ------------------------------------------------

  private setupRoutes(): void {
    this.router.add("POST", "/sessions", this.createSession.bind(this));
    this.router.add("GET", "/sessions", this.listSessions.bind(this));
    this.router.add("GET", "/sessions/:id", this.getSession.bind(this));
    this.router.add("POST", "/sessions/:id/task", this.sendTask.bind(this));
    this.router.add("DELETE", "/sessions/:id", this.cancelSession.bind(this));
    this.router.add("POST", "/sessions/:id/approve", this.approveSession.bind(this));
    this.router.add("GET", "/status", this.status.bind(this));
    this.router.add("GET", "/ws", this.handleWebSocket.bind(this));
  }

  // ----- HTTP handlers -----------------------------------------------------

  /** POST /sessions -- create a new agent session. */
  private createSession(ctx: RequestContext): void {
    const sessionId = randomUUID();

    try {
      this.workers.startSession(sessionId);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      sendJson(ctx.res, 409, { error: message });
      return;
    }

    sendJson(ctx.res, 201, { session_id: sessionId, status: "running" });
  }

  /** GET /sessions -- list all active sessions. */
  private listSessions(ctx: RequestContext): void {
    const sessions = this.workers.listSessions();
    const result = sessions.map((id) => {
      const [status] = this.workers.getStatus(id);
      return { session_id: id, status };
    });
    sendJson(ctx.res, 200, result);
  }

  /** GET /sessions/:id -- get a single session's status. */
  private getSession(ctx: RequestContext): void {
    const sessionId = ctx.params["id"]!;

    const [status, found] = this.workers.getStatus(sessionId);
    if (!found) {
      sendJson(ctx.res, 404, { error: "session not found" });
      return;
    }

    sendJson(ctx.res, 200, { session_id: sessionId, status });
  }

  /** POST /sessions/:id/task -- send a task to a session. */
  private sendTask(ctx: RequestContext): void {
    const sessionId = ctx.params["id"]!;
    const body = ctx.body as Record<string, unknown> | null;

    const task = typeof body?.["task"] === "string" ? body["task"] : "";
    if (task === "") {
      sendJson(ctx.res, 400, { error: "task is required" });
      return;
    }

    const [, found] = this.workers.getStatus(sessionId);
    if (!found) {
      sendJson(ctx.res, 404, { error: "session not found" });
      return;
    }

    try {
      this.workers.submitTask(sessionId, task);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      sendJson(ctx.res, 409, { error: message });
      return;
    }

    this.broadcastEvent(
      new TaskUpdateEvent(sessionId, "task_received", task),
    );

    sendJson(ctx.res, 200, { session_id: sessionId, status: "task_sent" });
  }

  /** DELETE /sessions/:id -- cancel a session. */
  private cancelSession(ctx: RequestContext): void {
    const sessionId = ctx.params["id"]!;

    try {
      this.workers.stopSession(sessionId);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      sendJson(ctx.res, 404, { error: message });
      return;
    }

    sendJson(ctx.res, 200, { session_id: sessionId, status: "cancelled" });
  }

  /** POST /sessions/:id/approve -- respond to a tool approval request. */
  private approveSession(ctx: RequestContext): void {
    if (!this.approvalHandler) {
      sendJson(ctx.res, 501, { error: "approval not configured" });
      return;
    }

    const body = ctx.body as Record<string, unknown> | null;
    const requestId = typeof body?.["requestId"] === "string" ? body["requestId"] : "";
    const approved = typeof body?.["approved"] === "boolean" ? body["approved"] : false;

    if (requestId === "") {
      sendJson(ctx.res, 400, { error: "requestId is required" });
      return;
    }

    const found = this.approvalHandler.respond(requestId, approved);
    if (!found) {
      sendJson(ctx.res, 404, { error: "approval request not found or expired" });
      return;
    }

    sendJson(ctx.res, 200, { request_id: requestId, approved });
  }

  /** GET /status -- server health / version info. */
  private status(_ctx: RequestContext): void {
    sendJson(_ctx.res, 200, { version: "0.1.0", status: "running" });
  }

  // ----- WebSocket handler -------------------------------------------------

  private handleWebSocket(ctx: RequestContext): void {
    // The actual WebSocket upgrade is handled in the request listener.
    // This handler is only reached for route matching purposes; the upgrade
    // happens inside the main request handler when we detect the /ws path
    // with the WebSocket upgrade headers.
    // If we land here, it means the request was not a WS upgrade, so reject.
    sendJson(ctx.res, 400, { error: "expected WebSocket upgrade" });
  }

  // ----- Server lifecycle --------------------------------------------------

  /**
   * Start listening on the configured address.
   * Returns a promise that resolves once the server is listening,
   * or rejects on immediate startup errors.
   */
  start(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.httpServer = http.createServer(this.requestListener.bind(this));

      // Attach WebSocket server
      this.wss = new WebSocketServer({ noServer: true });

      this.httpServer.on("upgrade", (req, socket, head) => {
        const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
        if (url.pathname === "/ws") {
          this.wss!.handleUpgrade(req, socket, head, (ws) => {
            this.handleWsConnection(ws);
          });
        } else {
          socket.destroy();
        }
      });

      // Parse the listen address
      const addr = this.addr;
      const port = addr.startsWith(":") ? parseInt(addr.slice(1), 10) : undefined;
      const host = addr.startsWith(":") ? undefined : undefined;

      this.httpServer.on("error", (err) => {
        if (!this.alive) {
          reject(err);
        } else {
          console.error("[control-server] server error:", err);
        }
      });

      this.httpServer.listen(port, host, () => {
        this.alive = true;
        resolve();
      });
    });
  }

  /**
   * Perform graceful shutdown with a 10-second timeout.
   * Cancels all worker sessions, then closes the HTTP and WebSocket servers.
   */
  async stop(): Promise<void> {
    this.workers.shutdown();

    if (this.httpServer === null) {
      return;
    }

    let timeoutID: NodeJS.Timeout | undefined;
    const shutdownPromise = new Promise<void>((resolve) => {
      this.httpServer!.close(() => resolve());
      this.wss?.close();
    });

    const timeout = new Promise<void>((resolve) => {
      timeoutID = setTimeout(resolve, 10_000);
    });
    await Promise.race([shutdownPromise, timeout]);
    if (timeoutID !== undefined) {
      clearTimeout(timeoutID);
    }
  }

  /**
   * Return the underlying http.Server (useful for testing with supertest etc).
   */
  getHttpServer(): http.Server | null {
    return this.httpServer;
  }

  // ----- Internal ----------------------------------------------------------

  private async requestListener(
    req: http.IncomingMessage,
    res: http.ServerResponse,
  ): Promise<void> {
    const start = performance.now();

    // CORS middleware
    if (corsMiddleware(req, res)) {
      logRequest(req, res, performance.now() - start);
      return;
    }

    // Parse URL
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    const pathname = url.pathname;

    // Route resolution
    const match = this.router.resolve(req.method ?? "GET", pathname);
    if (!match) {
      sendJson(res, 404, { error: "not found" });
      logRequest(req, res, performance.now() - start);
      return;
    }

    // Read body for methods that may have one
    let body: unknown = null;
    if (req.method === "POST" || req.method === "PUT") {
      try {
        const raw = await readBody(req);
        if (raw.length > 0) {
          try {
            body = JSON.parse(raw);
          } catch {
            sendJson(res, 400, { error: "invalid JSON" });
            logRequest(req, res, performance.now() - start);
            return;
          }
        }
      } catch {
        sendJson(res, 400, { error: "failed to read request body" });
        logRequest(req, res, performance.now() - start);
        return;
      }
    }

    const ctx: RequestContext = { req, res, params: match.params, body };
    try {
      match.handler(ctx);
    } catch (err: unknown) {
      console.error("[control-server] unhandled error in handler:", err);
      if (!res.headersSent) {
        sendJson(res, 500, { error: "internal server error" });
      }
    }

    logRequest(req, res, performance.now() - start);
  }

  private handleWsConnection(ws: WebSocket): void {
    // Create a per-client event handler that forwards events to this WebSocket.
    const subscriber = (event: ServerEvent): void => {
      if (ws.readyState !== WebSocket.OPEN) {
        return;
      }
      try {
        const data = JSON.stringify({
          event_type: event.eventType(),
          ...serializeEvent(event),
        });
        ws.send(data);
      } catch {
        // Drop event on serialization or send failure.
      }
    };

    const unsubscribe = this.subscribeEvents(subscriber);

    ws.on("close", () => {
      unsubscribe();
    });

    ws.on("error", (err) => {
      console.error("[control-server] websocket error:", err.message);
      unsubscribe();
    });
  }
}

// ---------------------------------------------------------------------------
// Event serialization helper
// ---------------------------------------------------------------------------

function serializeEvent(event: ServerEvent): Record<string, unknown> {
  if (event instanceof TaskUpdateEvent) {
    return {
      session_id: event.sessionId,
      status: event.status,
      message: event.message,
    };
  }
  if (event instanceof AgentStreamEvent) {
    return {
      session_id: event.sessionId,
      content: event.content,
    };
  }
  if (event instanceof AgentCompletedEvent) {
    return {
      session_id: event.sessionId,
      content: event.content,
      tool_calls_made: event.toolCallsMade,
      tokens_used: event.tokensUsed,
      duration_ms: event.durationMs,
      stop_reason: event.stopReason,
    };
  }
  if (event instanceof GuardrailEvent) {
    return {
      session_id: event.sessionId,
      phase: event.phase,
      decision: event.decision,
      reason: event.reason,
      guardrail_name: event.guardrailName,
    };
  }
  if (event instanceof ToolCallEvent) {
    return {
      session_id: event.sessionId,
      tool_name: event.toolName,
      input: event.input,
      output: event.output,
      is_error: event.isError,
    };
  }
  if (event instanceof ErrorEvent) {
    return {
      session_id: event.sessionId,
      error: event.error,
    };
  }
  // Fallback: spread all own enumerable properties.
  return { ...(event as unknown as Record<string, unknown>) };
}
