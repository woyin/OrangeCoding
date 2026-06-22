/** JSON-RPC 2.0 协议类型：请求 / 响应 / 通知 / 标准错误码。 */

export const JSONRPC_VERSION = "2.0";

/** Represents a JSON-RPC 2.0 request. */
export interface Request {
  jsonrpc: string;
  id: unknown;
  method: string;
  params?: unknown;
}

/** Represents a JSON-RPC 2.0 response (success or error). */
export interface Response {
  jsonrpc: string;
  id: unknown;
  result?: unknown;
  error?: ResponseError;
}

/** Represents the error object in a JSON-RPC 2.0 error response. */
export interface ResponseError {
  code: number;
  message: string;
  data?: unknown;
}

/** Represents a JSON-RPC 2.0 notification (no ID, no response expected). */
export interface Notification {
  jsonrpc: string;
  method: string;
  params?: unknown;
}

/** Standard JSON-RPC 2.0 error codes. */
/** Standard JSON-RPC 2.0 error codes as defined in the specification. */
export const ErrorCode = {
  ParseError: -32700,
  InvalidRequest: -32600,
  MethodNotFound: -32601,
  InvalidParams: -32602,
  InternalError: -32603,
} as const;
export type ErrorCode = (typeof ErrorCode)[keyof typeof ErrorCode];

export interface ToolInfo {
  name: string;
  description?: string;
  inputSchema?: unknown;
}
