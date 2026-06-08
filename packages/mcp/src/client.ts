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

  constructor(private readonly transport: Transport) {}

  /** Close shuts down the client. */
  async close(): Promise<void> {
    if (!this.closed) {
      this.closed = true;
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

    // Read responses until we find one matching our ID.
    for (;;) {
      if (this.closed) {
        throw newProtocolError("client closed");
      }

      const raw = await this.transport.receive();
      const resp: Response = JSON.parse(new TextDecoder().decode(raw));

      // Skip notifications (no ID)
      if (resp.id === undefined || resp.id === null) {
        continue;
      }

      return resp;
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
}
