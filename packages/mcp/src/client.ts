import type { Transport } from "./transport.js";
import { JSONRPC_VERSION, type Request, type Response, type Notification, type ToolInfo } from "./jsonrpc.js";
import { newProtocolError } from "@orangecoding/core";

/** Describes the MCP server identity returned by Initialize. */
export interface ServerInfo {
  name: string;
  version: string;
}

/** McpClient is an MCP protocol client that communicates over a Transport. */
export class McpClient {
  private idCounter = 0;
  private closed = false;
  private pendingResponses: Map<number, Response> = new Map();
  private waitingResolvers: Map<number, (resp: Response) => void> = new Map();

  constructor(private readonly transport: Transport) {}

  /** Close shuts down the client. */
  async close(): Promise<void> {
    if (!this.closed) {
      this.closed = true;
      // Reject all waiting requests
      for (const [id, resolve] of this.waitingResolvers) {
        this.waitingResolvers.delete(id);
        resolve({
          jsonrpc: JSONRPC_VERSION,
          id,
          error: { code: -32000, message: "client closed" },
        });
      }
      await this.transport.close();
    }
  }

  /** nextID generates the next request ID. */
  private nextID(): number {
    return ++this.idCounter;
  }

  /** sendRequest sends a JSON-RPC request and waits for the matching response. */
  private async sendRequest(method: string, params?: unknown): Promise<Response> {
    const id = this.nextID();

    const req: Request = {
      jsonrpc: JSONRPC_VERSION,
      id,
      method,
      params,
    };

    const data = new TextEncoder().encode(JSON.stringify(req));
    await this.transport.send(data);

    // Check if a response was already received (from a previous read cycle)
    const cached = this.pendingResponses.get(id);
    if (cached) {
      this.pendingResponses.delete(id);
      return cached;
    }

    // Wait for the matching response
    return new Promise<Response>((resolve) => {
      this.waitingResolvers.set(id, resolve);
      this.drainResponses().catch(() => {
        // Transport closed or error — resolve with error response
        if (this.waitingResolvers.has(id)) {
          this.waitingResolvers.delete(id);
          resolve({
            jsonrpc: JSONRPC_VERSION,
            id,
            error: { code: -32000, message: "transport error" },
          });
        }
      });
    });
  }

  /** Continuously read responses from the transport and dispatch to waiting requests. */
  private async drainResponses(): Promise<void> {
    while (!this.closed) {
      const raw = await this.transport.receive();
      const msg: Response = JSON.parse(new TextDecoder().decode(raw));

      // Skip notifications (no ID)
      if (msg.id === undefined || msg.id === null) {
        continue;
      }

      const respId = msg.id as number;

      // Check if someone is waiting for this response
      const resolver = this.waitingResolvers.get(respId);
      if (resolver) {
        this.waitingResolvers.delete(respId);
        resolver(msg);
      } else {
        // Cache for later retrieval
        this.pendingResponses.set(respId, msg);
      }
    }
  }

  /** Initialize sends the MCP "initialize" request and returns server info. */
  async initialize(): Promise<ServerInfo> {
    const params = {
      capabilities: {},
      clientInfo: {
        name: "orange-mcp-client",
        version: "0.1.0",
      },
    };

    const resp = await this.sendRequest("initialize", params);

    if (resp.error) {
      throw newProtocolError(`initialize error [${resp.error.code}]: ${resp.error.message}`);
    }

    const result = resp.result as { serverInfo: ServerInfo };
    if (!result?.serverInfo) {
      throw newProtocolError("parse server info: missing serverInfo");
    }

    // Send "notifications/initialized" as per MCP spec.
    const notif: Notification = {
      jsonrpc: JSONRPC_VERSION,
      method: "notifications/initialized",
    };
    await this.transport.send(new TextEncoder().encode(JSON.stringify(notif)));

    return result.serverInfo;
  }

  /** ListTools sends the MCP "tools/list" request and returns the available tools. */
  async listTools(): Promise<ToolInfo[]> {
    const resp = await this.sendRequest("tools/list");

    if (resp.error) {
      throw newProtocolError(`tools/list error [${resp.error.code}]: ${resp.error.message}`);
    }

    const result = resp.result as { tools: ToolInfo[] };
    return result?.tools ?? [];
  }

  /** CallTool sends the MCP "tools/call" request and returns the raw result. */
  async callTool(name: string, args?: unknown): Promise<unknown> {
    const params = {
      name,
      arguments: args,
    };

    const resp = await this.sendRequest("tools/call", params);

    if (resp.error) {
      throw newProtocolError(`tools/call error [${resp.error.code}]: ${resp.error.message}`);
    }

    return resp.result;
  }

  /** ListPrompts sends the MCP "prompts/list" request. */
  async listPrompts(): Promise<PromptInfo[]> {
    const resp = await this.sendRequest("prompts/list");

    if (resp.error) {
      throw newProtocolError(`prompts/list error [${resp.error.code}]: ${resp.error.message}`);
    }

    const result = resp.result as { prompts: PromptInfo[] };
    return result?.prompts ?? [];
  }

  /** GetPrompt sends the MCP "prompts/get" request. */
  async getPrompt(name: string, args?: Record<string, string>): Promise<PromptResult> {
    const params = { name, arguments: args };

    const resp = await this.sendRequest("prompts/get", params);

    if (resp.error) {
      throw newProtocolError(`prompts/get error [${resp.error.code}]: ${resp.error.message}`);
    }

    return resp.result as PromptResult;
  }

  /** ListResources sends the MCP "resources/list" request. */
  async listResources(): Promise<ResourceInfo[]> {
    const resp = await this.sendRequest("resources/list");

    if (resp.error) {
      throw newProtocolError(`resources/list error [${resp.error.code}]: ${resp.error.message}`);
    }

    const result = resp.result as { resources: ResourceInfo[] };
    return result?.resources ?? [];
  }

  /** ReadResource sends the MCP "resources/read" request. */
  async readResource(uri: string): Promise<ResourceResult> {
    const params = { uri };

    const resp = await this.sendRequest("resources/read", params);

    if (resp.error) {
      throw newProtocolError(`resources/read error [${resp.error.code}]: ${resp.error.message}`);
    }

    return resp.result as ResourceResult;
  }
}

// ---------------------------------------------------------------------------
// Prompt types (MCP spec)
// ---------------------------------------------------------------------------

export interface PromptInfo {
  name: string;
  description?: string;
  arguments?: Array<{
    name: string;
    description?: string;
    required?: boolean;
  }>;
}

export interface PromptResult {
  description?: string;
  messages: Array<{
    role: "user" | "assistant";
    content: {
      type: "text" | "image" | "resource";
      text?: string;
      data?: string;
      mimeType?: string;
      uri?: string;
    };
  }>;
}

// ---------------------------------------------------------------------------
// Resource types (MCP spec)
// ---------------------------------------------------------------------------

export interface ResourceInfo {
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
}

export interface ResourceResult {
  contents: Array<{
    uri: string;
    mimeType?: string;
    text?: string;
    blob?: string;
  }>;
}
