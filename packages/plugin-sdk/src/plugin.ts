/**
 * Plugin runtime — handles JSON-RPC communication and dispatches to tool implementations.
 */

import * as readline from "node:readline";
import type {
  JsonRpcRequest,
  JsonRpcResponse,
  InitializeParams,
  InitializeResult,
  ToolCallParams,
  ToolCallResult,
  ToolDefinition,
} from "./protocol.js";
import { ErrorCode } from "./protocol.js";

// ---------------------------------------------------------------------------
// ToolHandler — what plugin authors implement
// ---------------------------------------------------------------------------

export interface ToolHandler {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
  readOnly?: boolean;
  destructive?: boolean;
  execute(input: Record<string, unknown>): Promise<string> | string;
}

// ---------------------------------------------------------------------------
// PluginConfig — plugin metadata
// ---------------------------------------------------------------------------

export interface PluginConfig {
  name: string;
  version: string;
  description?: string;
  tools: ToolHandler[];
}

// ---------------------------------------------------------------------------
// Plugin runtime
// ---------------------------------------------------------------------------

/**
 * Create and run a plugin. This is the main entry point for plugin authors.
 *
 * @example
 * ```ts
 * import { createPlugin } from "@orangecoding/plugin-sdk";
 *
 * createPlugin({
 *   name: "my-plugin",
 *   version: "1.0.0",
 *   description: "My custom tools",
 *   tools: [{
 *     name: "hello",
 *     description: "Say hello",
 *     parameters: { type: "object", properties: { name: { type: "string" } } },
 *     execute: async (input) => `Hello, ${input.name}!`,
 *   }],
 * });
 * ```
 */
export function createPlugin(config: PluginConfig): void {
  const toolMap = new Map<string, ToolHandler>();
  for (const tool of config.tools) {
    toolMap.set(tool.name, tool);
  }

  const rl = readline.createInterface({ input: process.stdin });
  let initialized = false;

  rl.on("line", async (line: string) => {
    if (!line.trim()) return;

    let request: JsonRpcRequest;
    try {
      request = JSON.parse(line) as JsonRpcRequest;
    } catch {
      writeResponse({ jsonrpc: "2.0", id: 0, error: { code: ErrorCode.ParseError, message: "parse error" } });
      return;
    }

    try {
      const result = await handleRequest(request, config, toolMap, () => { initialized = true; });
      writeResponse({ jsonrpc: "2.0", id: request.id, result });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      writeResponse({
        jsonrpc: "2.0",
        id: request.id,
        error: { code: ErrorCode.InternalError, message },
      });
    }
  });

  // Graceful shutdown on SIGTERM/SIGINT
  const shutdown = () => {
    process.exit(0);
  };
  process.on("SIGTERM", shutdown);
  process.on("SIGINT", shutdown);
}

async function handleRequest(
  req: JsonRpcRequest,
  config: PluginConfig,
  toolMap: Map<string, ToolHandler>,
  markInitialized: () => void,
): Promise<unknown> {
  switch (req.method) {
    case "initialize": {
      markInitialized();
      const params = req.params as InitializeParams;
      const result: InitializeResult = {
        name: config.name,
        version: config.version,
        description: config.description ?? "",
        tools: config.tools.map((t) => ({
          name: t.name,
          description: t.description,
          parameters: t.parameters,
          readOnly: t.readOnly ?? false,
          destructive: t.destructive ?? false,
        })),
      };
      // Store workDir for tools that need it
      process.env["PLUGIN_WORK_DIR"] = params.workDir;
      return result;
    }

    case "tools/call": {
      const params = req.params as ToolCallParams;
      const tool = toolMap.get(params.name);
      if (!tool) {
        throw new Error(`tool not found: ${params.name}`);
      }
      try {
        const content = await tool.execute(params.input);
        const result: ToolCallResult = { content };
        return result;
      } catch (err) {
        const content = err instanceof Error ? err.message : String(err);
        const result: ToolCallResult = { content, isError: true };
        return result;
      }
    }

    case "shutdown": {
      process.exit(0);
    }

    default:
      throw new Error(`unknown method: ${req.method}`);
  }
}

function writeResponse(resp: JsonRpcResponse): void {
  process.stdout.write(JSON.stringify(resp) + "\n");
}
