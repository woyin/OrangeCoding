/**
 * Tests for the plugin-sdk package — protocol types and error codes.
 */

import { ErrorCode } from "../protocol.js";
import type {
  JsonRpcRequest,
  JsonRpcResponse,
  JsonRpcError,
  InitializeParams,
  InitializeResult,
  ToolDefinition,
  ToolCallParams,
  ToolCallResult,
} from "../protocol.js";

// ---------------------------------------------------------------------------
// ErrorCode constants
// ---------------------------------------------------------------------------

describe("ErrorCode", () => {
  it("has standard JSON-RPC 2.0 error codes", () => {
    expect(ErrorCode.ParseError).toBe(-32700);
    expect(ErrorCode.InvalidRequest).toBe(-32600);
    expect(ErrorCode.MethodNotFound).toBe(-32601);
    expect(ErrorCode.InvalidParams).toBe(-32602);
    expect(ErrorCode.InternalError).toBe(-32603);
  });
});

// ---------------------------------------------------------------------------
// JSON-RPC type shapes
// ---------------------------------------------------------------------------

describe("JsonRpcRequest", () => {
  it("can be constructed with required fields", () => {
    const req: JsonRpcRequest = {
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
    };
    expect(req.jsonrpc).toBe("2.0");
    expect(req.id).toBe(1);
    expect(req.method).toBe("initialize");
  });

  it("supports optional params", () => {
    const req: JsonRpcRequest = {
      jsonrpc: "2.0",
      id: 2,
      method: "tools/call",
      params: { name: "hello", input: { name: "world" } },
    };
    expect(req.params).toBeDefined();
  });
});

describe("JsonRpcResponse", () => {
  it("can be constructed with result", () => {
    const resp: JsonRpcResponse = {
      jsonrpc: "2.0",
      id: 1,
      result: { name: "plugin", version: "1.0.0" },
    };
    expect(resp.result).toBeDefined();
    expect(resp.error).toBeUndefined();
  });

  it("can be constructed with error", () => {
    const resp: JsonRpcResponse = {
      jsonrpc: "2.0",
      id: 1,
      error: { code: ErrorCode.MethodNotFound, message: "not found" },
    };
    expect(resp.error).toBeDefined();
    expect(resp.error!.code).toBe(-32601);
  });
});

describe("JsonRpcError", () => {
  it("supports optional data field", () => {
    const err: JsonRpcError = {
      code: ErrorCode.InternalError,
      message: "internal error",
      data: { stack: "..." },
    };
    expect(err.data).toBeDefined();
  });
});

// ---------------------------------------------------------------------------
// Plugin protocol types
// ---------------------------------------------------------------------------

describe("InitializeParams", () => {
  it("has required fields", () => {
    const params: InitializeParams = {
      name: "host",
      version: "1.0.0",
      workDir: "/workspace",
    };
    expect(params.workDir).toBe("/workspace");
  });
});

describe("InitializeResult", () => {
  it("has required fields", () => {
    const result: InitializeResult = {
      name: "my-plugin",
      version: "1.0.0",
      description: "A test plugin",
      tools: [],
    };
    expect(result.name).toBe("my-plugin");
    expect(result.tools).toHaveLength(0);
  });
});

describe("ToolDefinition", () => {
  it("has required and optional fields", () => {
    const def: ToolDefinition = {
      name: "hello",
      description: "Say hello",
      parameters: {
        type: "object",
        properties: { name: { type: "string" } },
      },
      readOnly: true,
      destructive: false,
    };
    expect(def.name).toBe("hello");
    expect(def.readOnly).toBe(true);
    expect(def.destructive).toBe(false);
  });
});

describe("ToolCallParams", () => {
  it("has name and input", () => {
    const params: ToolCallParams = {
      name: "hello",
      input: { name: "world" },
    };
    expect(params.name).toBe("hello");
    expect(params.input).toEqual({ name: "world" });
  });
});

describe("ToolCallResult", () => {
  it("has content field", () => {
    const result: ToolCallResult = {
      content: "Hello, world!",
    };
    expect(result.content).toBe("Hello, world!");
    expect(result.isError).toBeUndefined();
  });

  it("supports isError flag", () => {
    const result: ToolCallResult = {
      content: "Error: something failed",
      isError: true,
    };
    expect(result.isError).toBe(true);
  });
});
