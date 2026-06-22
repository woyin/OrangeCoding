/**
 * @module mcp
 *
 * Model Context Protocol (MCP) implementation for OrangeCoding.
 *
 * MCP enables AI models to interact with external tools, resources,
 * and prompts through a standardized protocol. This module provides:
 * - McpServer: hosts tools/resources/prompts for AI consumption
 * - McpClient: connects to MCP servers and invokes their capabilities
 * - Transport layer: stdio and socket-based communication
 */
// JSON-RPC 2.0 protocol types
export {
  JSONRPC_VERSION,
  ErrorCode,
} from "./jsonrpc.js";
export type {
  Request,
  Response,
  ResponseError,
  Notification,
  ErrorCode as ErrorCodeType,
} from "./jsonrpc.js";

// Transport
export { StdioTransport, SSETransport, StreamableHTTPTransport } from "./transport.js";
export type { Transport } from "./transport.js";

// Client
export { McpClient } from "./client.js";
export type {
  ServerInfo,
  PromptInfo,
  PromptResult,
  ResourceInfo,
  ResourceResult,
} from "./client.js";

// ToolInfo (shared type, defined in jsonrpc.js)
export type { ToolInfo } from "./jsonrpc.js";

// Server
export { McpServer } from "./server.js";
export type { ToolHandler, PromptHandler, ResourceHandler, NotificationHandler } from "./server.js";

// Adapter (MCP → native Tool bridge)
export { McpToolAdapter, McpToolManager } from "./adapter.js";
export type { AdaptedTool, McpServerConfig } from "./adapter.js";
