/**
 * MCP Tool Adapter — bridges MCP tools into the native Tool interface.
 *
 * Allows the agent to use any MCP server's tools transparently as if they
 * were built-in tools, enabling dynamic tool discovery and extensibility.
 */

import type { McpClient } from "./client.js";
import type { ToolInfo } from "./jsonrpc.js";

// ---------------------------------------------------------------------------
// Tool interface (mirrored from @orangecoding/tools to avoid circular deps)
// ---------------------------------------------------------------------------

export interface ToolMetadata {
  readonly isReadOnly: boolean;
  readonly isConcurrencySafe: boolean;
  readonly isDestructive: boolean;
  readonly isEnabled: boolean;
}

export interface AdaptedTool {
  name(): string;
  description(): string;
  parameters(): Record<string, unknown>;
  execute(ctx: unknown, input: unknown): Promise<string>;
  metadata(): ToolMetadata;
}

// ---------------------------------------------------------------------------
// McpToolAdapter
// ---------------------------------------------------------------------------

/**
 * Adapts a single MCP tool into the native Tool interface.
 */
export class McpToolAdapter implements AdaptedTool {
  private readonly _name: string;
  private readonly _description: string;
  private readonly _parameters: Record<string, unknown>;

  constructor(
    private readonly client: McpClient,
    private readonly toolInfo: ToolInfo,
  ) {
    this._name = toolInfo.name;
    this._description = toolInfo.description ?? `MCP tool: ${toolInfo.name}`;
    this._parameters = (toolInfo.inputSchema as Record<string, unknown>) ?? {
      type: "object",
      properties: {},
    };
  }

  name(): string {
    return this._name;
  }

  description(): string {
    return this._description;
  }

  parameters(): Record<string, unknown> {
    return this._parameters;
  }

  metadata(): ToolMetadata {
    return {
      isReadOnly: false,
      isConcurrencySafe: true,
      isDestructive: false,
      isEnabled: true,
    };
  }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    try {
      const result = await this.client.callTool(this._name, input);
      // MCP tools can return various result types; normalize to string.
      if (typeof result === "string") {
        return result;
      }
      if (result === null || result === undefined) {
        return "";
      }
      // MCP content blocks: [{ type: "text", text: "..." }, ...]
      if (Array.isArray(result)) {
        return result
          .map((item: unknown) => {
            if (typeof item === "object" && item !== null && "text" in item) {
              return (item as { text: string }).text;
            }
            return String(item);
          })
          .join("\n");
      }
      if (typeof result === "object" && result !== null) {
        // Handle { content: [...] } wrapper
        const obj = result as Record<string, unknown>;
        if (Array.isArray(obj.content)) {
          return obj.content
            .map((item: unknown) => {
              if (typeof item === "object" && item !== null && "text" in item) {
                return (item as { text: string }).text;
              }
              return String(item);
            })
            .join("\n");
        }
        return JSON.stringify(result, null, 2);
      }
      return String(result);
    } catch (err) {
      throw new Error(`mcp_tool_error [${this._name}]: ${(err as Error).message}`);
    }
  }
}

// ---------------------------------------------------------------------------
// McpToolManager — manages multiple MCP server connections
// ---------------------------------------------------------------------------

export interface McpServerConfig {
  /** Unique server identifier. */
  id: string;
  /** Command to start the MCP server (stdio transport). */
  command?: string;
  /** Arguments for the command. */
  args?: string[];
  /** Environment variables for the server process. */
  env?: Record<string, string>;
  /** Pre-connected client (for testing or custom transports). */
  client?: McpClient;
}

interface ManagedServer {
  config: McpServerConfig;
  client: McpClient;
  tools: McpToolAdapter[];
  initialized: boolean;
}

/**
 * Manages multiple MCP server connections and provides unified tool access.
 *
 * Usage:
 *   const manager = new McpToolManager();
 *   await manager.addServer({ id: "fs", command: "npx", args: ["-y", "@mcp/fs-server"] });
 *   const tools = manager.getAllTools();
 *   // Register tools with the agent's tool registry
 *   await manager.close();
 */
export class McpToolManager {
  private servers = new Map<string, ManagedServer>();

  /**
   * Add and initialize an MCP server connection.
   */
  async addServer(config: McpServerConfig): Promise<McpToolAdapter[]> {
    if (this.servers.has(config.id)) {
      throw new Error(`MCP server "${config.id}" already registered`);
    }

    let client: McpClient;
    if (config.client) {
      client = config.client;
    } else if (config.command) {
      // Create a stdio transport and client
      const { spawn } = await import("node:child_process");
      const { StdioTransport } = await import("./transport.js");
      const { McpClient: McpClientClass } = await import("./client.js");
      const child = spawn(config.command, config.args ?? [], {
        env: config.env ? { ...process.env, ...config.env } : process.env,
        stdio: ["pipe", "pipe", "pipe"],
      });
      if (!child.stdin || !child.stdout) {
        throw new Error(`MCP server "${config.id}": failed to create stdio pipes`);
      }
      const transport = new StdioTransport(child.stdout as unknown as NodeJS.ReadableStream, child.stdin as unknown as NodeJS.WritableStream);
      client = new McpClientClass(transport);
    } else {
      throw new Error(`MCP server "${config.id}": must provide either client or command`);
    }

    const managed: ManagedServer = {
      config,
      client,
      tools: [],
      initialized: false,
    };

    try {
      await client.initialize();
      managed.initialized = true;

      const toolInfos = await client.listTools();
      managed.tools = toolInfos.map(
        (info) => new McpToolAdapter(client, info),
      );
    } catch (err) {
      // Clean up on failure
      try {
        await client.close();
      } catch {
        // ignore cleanup errors
      }
      throw new Error(
        `failed to initialize MCP server "${config.id}": ${(err as Error).message}`,
      );
    }

    this.servers.set(config.id, managed);
    return managed.tools;
  }

  /**
   * Remove and close a server connection.
   */
  async removeServer(id: string): Promise<void> {
    const server = this.servers.get(id);
    if (!server) return;

    try {
      await server.client.close();
    } catch {
      // ignore close errors
    }
    this.servers.delete(id);
  }

  /**
   * Get all tools from all connected servers.
   */
  getAllTools(): McpToolAdapter[] {
    const tools: McpToolAdapter[] = [];
    for (const server of this.servers.values()) {
      tools.push(...server.tools);
    }
    return tools;
  }

  /**
   * Get tools from a specific server.
   */
  getServerTools(id: string): McpToolAdapter[] {
    const server = this.servers.get(id);
    return server ? [...server.tools] : [];
  }

  /**
   * List connected server IDs.
   */
  listServers(): string[] {
    return Array.from(this.servers.keys());
  }

  /**
   * Refresh tools from a specific server (re-fetches tools/list).
   */
  async refreshServer(id: string): Promise<McpToolAdapter[]> {
    const server = this.servers.get(id);
    if (!server) {
      throw new Error(`MCP server "${id}" not found`);
    }

    const toolInfos = await server.client.listTools();
    server.tools = toolInfos.map(
      (info) => new McpToolAdapter(server.client, info),
    );
    return server.tools;
  }

  /**
   * Close all server connections.
   */
  async close(): Promise<void> {
    const closePromises: Promise<void>[] = [];
    for (const server of this.servers.values()) {
      closePromises.push(
        server.client.close().catch(() => {
          // ignore close errors during shutdown
        }),
      );
    }
    await Promise.all(closePromises);
    this.servers.clear();
  }
}
