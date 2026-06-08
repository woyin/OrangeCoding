import type { Transport } from "./transport.js";
import {
  JSONRPC_VERSION,
  ErrorCode,
  type Request,
  type Response,
  type Notification,
  type ToolInfo,
} from "./jsonrpc.js";
import type { ServerInfo } from "./client.js";

/** ToolHandler is the function signature for handling a tool invocation. */
export type ToolHandler = (args: unknown) => Promise<unknown>;

interface RegisteredTool {
  info: ToolInfo;
  handler: ToolHandler;
}

/** McpServer is an MCP protocol server that handles requests over a Transport. */
export class McpServer {
  private tools = new Map<string, RegisteredTool>();
  private serverInfo: ServerInfo = { name: "orange-mcp-server", version: "0.1.0" };

  constructor(private readonly transport: Transport) {}

  /** SetServerInfo configures the server identity returned during initialization. */
  setServerInfo(name: string, version: string): void {
    this.serverInfo = { name, version };
  }

  /** RegisterTool registers a tool with its handler. */
  registerTool(tool: ToolInfo, handler: ToolHandler): void {
    this.tools.set(tool.name, { info: tool, handler });
  }

  /**
   * Serve starts the server's main loop. It reads requests from the transport
   * and dispatches them to the appropriate handler. It blocks until the
   * abort signal is triggered or the transport returns an error.
   */
  async serve(signal?: AbortSignal): Promise<void> {
    for (;;) {
      if (signal?.aborted) return;

      let raw: Uint8Array;
      try {
        raw = await this.transport.receive();
      } catch {
        if (signal?.aborted) return;
        throw new Error("receive error");
      }

      let parsed: { id?: unknown; method?: string };
      try {
        parsed = JSON.parse(new TextDecoder().decode(raw));
      } catch {
        this.sendError(null, ErrorCode.ParseError, "parse error");
        continue;
      }

      // If no ID, it's a notification -- ignore for now.
      if (parsed.id === undefined || parsed.id === null) {
        continue;
      }

      // Parse full request.
      let req: Request;
      try {
        req = JSON.parse(new TextDecoder().decode(raw));
      } catch {
        this.sendError(parsed.id, ErrorCode.InvalidRequest, "invalid request");
        continue;
      }

      this.handleRequest(req);
    }
  }

  private handleRequest(req: Request): void {
    switch (req.method) {
      case "initialize":
        this.handleInitialize(req);
        break;
      case "tools/list":
        this.handleListTools(req);
        break;
      case "tools/call":
        this.handleCallTool(req);
        break;
      default:
        this.sendError(req.id, ErrorCode.MethodNotFound, `method not found: ${req.method}`);
    }
  }

  private handleInitialize(req: Request): void {
    const result = {
      capabilities: { tools: {} },
      serverInfo: this.serverInfo,
    };
    this.sendResponse(req.id, result);

    // Send "notifications/initialized" as per MCP spec.
    const notif: Notification = {
      jsonrpc: JSONRPC_VERSION,
      method: "notifications/initialized",
    };
    this.transport.send(new TextEncoder().encode(JSON.stringify(notif))).catch(() => {
      // Log but don't propagate.
    });
  }

  private handleListTools(req: Request): void {
    const tools: ToolInfo[] = [];
    for (const rt of this.tools.values()) {
      tools.push(rt.info);
    }
    this.sendResponse(req.id, { tools });
  }

  private async handleCallTool(req: Request): Promise<void> {
    const params = req.params as { name?: string; arguments?: unknown } | undefined;
    if (!params?.name) {
      this.sendError(req.id, ErrorCode.InvalidParams, "invalid params");
      return;
    }

    const rt = this.tools.get(params.name);
    if (!rt) {
      this.sendError(req.id, ErrorCode.MethodNotFound, `tool not found: ${params.name}`);
      return;
    }

    try {
      const result = await rt.handler(params.arguments);
      this.sendResponse(req.id, result);
    } catch (err) {
      this.sendError(req.id, ErrorCode.InternalError, (err as Error).message);
    }
  }

  private sendResponse(id: unknown, result: unknown): void {
    const resp: Response = {
      jsonrpc: JSONRPC_VERSION,
      id,
      result,
    };
    this.transport.send(new TextEncoder().encode(JSON.stringify(resp))).catch(() => {
      // Log but don't propagate.
    });
  }

  private sendError(id: unknown, code: number, message: string): void {
    const resp: Response = {
      jsonrpc: JSONRPC_VERSION,
      id,
      error: { code, message },
    };
    this.transport.send(new TextEncoder().encode(JSON.stringify(resp))).catch(() => {
      // Log but don't propagate.
    });
  }
}
