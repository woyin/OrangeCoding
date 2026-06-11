import type { Transport } from "./transport.js";
import {
  JSONRPC_VERSION,
  ErrorCode,
  type Request,
  type Response,
  type Notification,
  type ToolInfo,
} from "./jsonrpc.js";
import type { ServerInfo, PromptInfo, PromptResult, ResourceInfo, ResourceResult } from "./client.js";

// ---------------------------------------------------------------------------
// Handler types
// ---------------------------------------------------------------------------

/** ToolHandler is the function signature for handling a tool invocation. */
export type ToolHandler = (args: unknown) => Promise<unknown>;

/** PromptHandler is the function signature for handling a prompt request. */
export type PromptHandler = (args: Record<string, string> | undefined) => Promise<PromptResult>;

/** ResourceHandler is the function signature for handling a resource read request. */
export type ResourceHandler = (uri: string) => Promise<ResourceResult>;

/** NotificationHandler is the function signature for handling a notification. */
export type NotificationHandler = (method: string, params: unknown) => Promise<void>;

// ---------------------------------------------------------------------------
// Registered items
// ---------------------------------------------------------------------------

interface RegisteredTool {
  info: ToolInfo;
  handler: ToolHandler;
}

interface RegisteredPrompt {
  info: PromptInfo;
  handler: PromptHandler;
}

interface RegisteredResource {
  info: ResourceInfo;
  handler: ResourceHandler;
}

// ---------------------------------------------------------------------------
// McpServer
// ---------------------------------------------------------------------------

/** McpServer is an MCP protocol server that handles requests over a Transport. */
export class McpServer {
  private tools = new Map<string, RegisteredTool>();
  private prompts = new Map<string, RegisteredPrompt>();
  private resources = new Map<string, RegisteredResource>();
  private notificationHandlers = new Map<string, NotificationHandler>();
  private serverInfo: ServerInfo = { name: "orange-mcp-server", version: "0.1.0" };

  constructor(private readonly transport: Transport) {}

  /** SetServerInfo configures the server identity returned during initialization. */
  setServerInfo(name: string, version: string): void {
    this.serverInfo = { name, version };
  }

  // -----------------------------------------------------------------------
  // Tool registration
  // -----------------------------------------------------------------------

  /** RegisterTool registers a tool with its handler. */
  registerTool(tool: ToolInfo, handler: ToolHandler): void {
    this.tools.set(tool.name, { info: tool, handler });
  }

  // -----------------------------------------------------------------------
  // Prompt registration
  // -----------------------------------------------------------------------

  /** RegisterPrompt registers a prompt template with its handler. */
  registerPrompt(prompt: PromptInfo, handler: PromptHandler): void {
    this.prompts.set(prompt.name, { info: prompt, handler });
  }

  // -----------------------------------------------------------------------
  // Resource registration
  // -----------------------------------------------------------------------

  /** RegisterResource registers a resource with its handler. */
  registerResource(resource: ResourceInfo, handler: ResourceHandler): void {
    this.resources.set(resource.uri, { info: resource, handler });
  }

  // -----------------------------------------------------------------------
  // Notification handlers
  // -----------------------------------------------------------------------

  /** OnNotification registers a handler for a specific notification method. */
  onNotification(method: string, handler: NotificationHandler): void {
    this.notificationHandlers.set(method, handler);
  }

  // -----------------------------------------------------------------------
  // Serve — main server loop
  // -----------------------------------------------------------------------

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

      let parsed: { id?: unknown; method?: string; params?: unknown };
      try {
        parsed = JSON.parse(new TextDecoder().decode(raw));
      } catch {
        this.sendError(null, ErrorCode.ParseError, "parse error");
        continue;
      }

      // If no ID, it's a notification — dispatch to registered handlers.
      if (parsed.id === undefined || parsed.id === null) {
        if (parsed.method && this.notificationHandlers.has(parsed.method)) {
          const handler = this.notificationHandlers.get(parsed.method)!;
          try {
            await handler(parsed.method, parsed.params);
          } catch {
            // Notification handler errors are non-fatal
          }
        }
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

  // -----------------------------------------------------------------------
  // Request dispatch
  // -----------------------------------------------------------------------

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
      case "prompts/list":
        this.handleListPrompts(req);
        break;
      case "prompts/get":
        this.handleGetPrompt(req);
        break;
      case "resources/list":
        this.handleListResources(req);
        break;
      case "resources/read":
        this.handleReadResource(req);
        break;
      default:
        this.sendError(req.id, ErrorCode.MethodNotFound, `method not found: ${req.method}`);
    }
  }

  // -----------------------------------------------------------------------
  // Method handlers
  // -----------------------------------------------------------------------

  private handleInitialize(req: Request): void {
    const result = {
      capabilities: {
        tools: this.tools.size > 0 ? {} : undefined,
        prompts: this.prompts.size > 0 ? {} : undefined,
        resources: this.resources.size > 0 ? {} : undefined,
      },
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

  private handleListPrompts(req: Request): void {
    const prompts: PromptInfo[] = [];
    for (const rp of this.prompts.values()) {
      prompts.push(rp.info);
    }
    this.sendResponse(req.id, { prompts });
  }

  private async handleGetPrompt(req: Request): Promise<void> {
    const params = req.params as { name?: string; arguments?: Record<string, string> } | undefined;
    if (!params?.name) {
      this.sendError(req.id, ErrorCode.InvalidParams, "invalid params: name required");
      return;
    }

    const rp = this.prompts.get(params.name);
    if (!rp) {
      this.sendError(req.id, ErrorCode.MethodNotFound, `prompt not found: ${params.name}`);
      return;
    }

    try {
      const result = await rp.handler(params.arguments);
      this.sendResponse(req.id, result);
    } catch (err) {
      this.sendError(req.id, ErrorCode.InternalError, (err as Error).message);
    }
  }

  private handleListResources(req: Request): void {
    const resources: ResourceInfo[] = [];
    for (const rr of this.resources.values()) {
      resources.push(rr.info);
    }
    this.sendResponse(req.id, { resources });
  }

  private async handleReadResource(req: Request): Promise<void> {
    const params = req.params as { uri?: string } | undefined;
    if (!params?.uri) {
      this.sendError(req.id, ErrorCode.InvalidParams, "invalid params: uri required");
      return;
    }

    const rr = this.resources.get(params.uri);
    if (!rr) {
      this.sendError(req.id, ErrorCode.MethodNotFound, `resource not found: ${params.uri}`);
      return;
    }

    try {
      const result = await rr.handler(params.uri);
      this.sendResponse(req.id, result);
    } catch (err) {
      this.sendError(req.id, ErrorCode.InternalError, (err as Error).message);
    }
  }

  // -----------------------------------------------------------------------
  // Response helpers
  // -----------------------------------------------------------------------

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
