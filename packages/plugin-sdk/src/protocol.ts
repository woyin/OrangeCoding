/**
 * OrangeCoding Plugin Protocol — JSON-RPC 2.0 over stdin/stdout.
 *
 * Communication flow:
 *   Host → Plugin stdin:  JSON-RPC request + newline
 *   Plugin → Host stdout: JSON-RPC response + newline
 */

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

export interface JsonRpcRequest {
  jsonrpc: "2.0";
  id: number;
  method: string;
  params?: unknown;
}

export interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: number;
  result?: unknown;
  error?: JsonRpcError;
}

export interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

// Standard JSON-RPC error codes
export const ErrorCode = {
  ParseError: -32700,
  InvalidRequest: -32600,
  MethodNotFound: -32601,
  InvalidParams: -32602,
  InternalError: -32603,
} as const;

// ---------------------------------------------------------------------------
// Plugin protocol types
// ---------------------------------------------------------------------------

export interface InitializeParams {
  name: string;
  version: string;
  workDir: string;
}

export interface InitializeResult {
  name: string;
  version: string;
  description: string;
  tools: ToolDefinition[];
}

export interface ToolDefinition {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
  readOnly?: boolean;
  destructive?: boolean;
}

export interface ToolCallParams {
  name: string;
  input: Record<string, unknown>;
}

export interface ToolCallResult {
  content: string;
  isError?: boolean;
}
