/**
 * @orangecoding/plugin-sdk — TypeScript SDK for writing OrangeCoding tool plugins.
 *
 * Quick start:
 *   import { createPlugin } from "@orangecoding/plugin-sdk";
 *
 *   createPlugin({
 *     name: "my-plugin",
 *     version: "1.0.0",
 *     tools: [{
 *       name: "hello",
 *       description: "Say hello",
 *       parameters: { type: "object", properties: { name: { type: "string" } } },
 *       execute: async (input) => `Hello, ${input.name}!`,
 *     }],
 *   });
 */

export { createPlugin } from "./plugin.js";
export type { ToolHandler, PluginConfig } from "./plugin.js";

export type {
  JsonRpcRequest,
  JsonRpcResponse,
  JsonRpcError,
  InitializeParams,
  InitializeResult,
  ToolDefinition,
  ToolCallParams,
  ToolCallResult,
} from "./protocol.js";

export { ErrorCode } from "./protocol.js";
