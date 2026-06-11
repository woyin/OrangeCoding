/**
 * Tests for McpServer with prompt, resource, and notification support.
 */

import { McpServer } from "../server.js";
import type { Transport } from "../transport.js";
import type { Response } from "../jsonrpc.js";
import { JSONRPC_VERSION } from "../jsonrpc.js";

// ---------------------------------------------------------------------------
// Mock transport
// ---------------------------------------------------------------------------

function createMockTransport(): {
  transport: Transport;
  pushMessage: (msg: unknown) => void;
  getSentMessages: () => string[];
  close: () => void;
} {
  const sentMessages: string[] = [];
  const pendingReceives: Array<(data: Uint8Array) => void> = [];
  let closed = false;

  const transport: Transport = {
    async send(data: Uint8Array): Promise<void> {
      sentMessages.push(new TextDecoder().decode(data));
    },
    async receive(): Promise<Uint8Array> {
      return new Promise((resolve) => {
        if (closed) {
          throw new Error("transport closed");
        }
        pendingReceives.push(resolve);
      });
    },
    async close(): Promise<void> {
      closed = true;
    },
  };

  const pushMessage = (msg: unknown) => {
    const data = new TextEncoder().encode(JSON.stringify(msg));
    const resolver = pendingReceives.shift();
    if (resolver) {
      resolver(data);
    }
  };

  return {
    transport,
    pushMessage,
    getSentMessages: () => sentMessages,
    close: () => {
      closed = true;
      // Reject pending receives
      for (const r of pendingReceives) {
        // Resolver will throw because we mark as closed
      }
      pendingReceives.length = 0;
    },
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("McpServer", () => {
  test("handles initialize request", async () => {
    const { transport, pushMessage, getSentMessages } = createMockTransport();
    const server = new McpServer(transport);
    server.setServerInfo("test-server", "1.0.0");

    // Start server in background
    const servePromise = server.serve();

    // Push initialize request
    pushMessage({
      jsonrpc: JSONRPC_VERSION,
      id: 1,
      method: "initialize",
      params: { clientInfo: { name: "test", version: "1.0.0" } },
    });

    // Wait for response
    await new Promise((r) => setTimeout(r, 50));

    const sent = getSentMessages();
    expect(sent.length).toBeGreaterThanOrEqual(1);

    const response = JSON.parse(sent[0]!) as Response;
    expect(response.id).toBe(1);
    expect(response.result).toBeDefined();
    const result = response.result as { serverInfo: { name: string; version: string } };
    expect(result.serverInfo.name).toBe("test-server");

    // Clean up
    pushMessage({ jsonrpc: JSONRPC_VERSION, method: "cancel" }); // trigger end
  });

  test("registers and lists tools", async () => {
    const { transport, pushMessage, getSentMessages } = createMockTransport();
    const server = new McpServer(transport);

    server.registerTool(
      { name: "test_tool", description: "A test tool" },
      async () => "tool result",
    );

    const servePromise = server.serve();

    // Initialize first
    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 1, method: "initialize", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    // Then list tools
    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 2, method: "tools/list", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    const sent = getSentMessages();
    // Find the tools/list response (should be last or second to last)
    const toolsResponse = sent
      .map((s) => JSON.parse(s) as Response)
      .find((r) => r.id === 2);

    expect(toolsResponse).toBeDefined();
    const result = toolsResponse!.result as { tools: Array<{ name: string }> };
    expect(result.tools.length).toBe(1);
    expect(result.tools[0]!.name).toBe("test_tool");
  });

  test("registers and lists prompts", async () => {
    const { transport, pushMessage, getSentMessages } = createMockTransport();
    const server = new McpServer(transport);

    server.registerPrompt(
      { name: "greeting", description: "A greeting prompt" },
      async () => ({
        messages: [{ role: "user" as const, content: { type: "text" as const, text: "Hello!" } }],
      }),
    );

    const servePromise = server.serve();

    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 1, method: "initialize", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 2, method: "prompts/list", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    const sent = getSentMessages();
    const promptsResponse = sent
      .map((s) => JSON.parse(s) as Response)
      .find((r) => r.id === 2);

    expect(promptsResponse).toBeDefined();
    const result = promptsResponse!.result as { prompts: Array<{ name: string }> };
    expect(result.prompts.length).toBe(1);
    expect(result.prompts[0]!.name).toBe("greeting");
  });

  test("gets a prompt with arguments", async () => {
    const { transport, pushMessage, getSentMessages } = createMockTransport();
    const server = new McpServer(transport);

    server.registerPrompt(
      {
        name: "code_review",
        description: "Code review prompt",
        arguments: [{ name: "language", description: "Programming language", required: true }],
      },
      async (args) => ({
        messages: [{
          role: "user" as const,
          content: { type: "text" as const, text: `Review this ${args?.language ?? "unknown"} code.` },
        }],
      }),
    );

    const servePromise = server.serve();

    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 1, method: "initialize", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    pushMessage({
      jsonrpc: JSONRPC_VERSION,
      id: 2,
      method: "prompts/get",
      params: { name: "code_review", arguments: { language: "TypeScript" } },
    });
    await new Promise((r) => setTimeout(r, 50));

    const sent = getSentMessages();
    const promptResponse = sent
      .map((s) => JSON.parse(s) as Response)
      .find((r) => r.id === 2);

    expect(promptResponse).toBeDefined();
    const result = promptResponse!.result as { messages: Array<{ role: string; content: { text: string } }> };
    expect(result.messages[0]!.content.text).toContain("TypeScript");
  });

  test("registers and lists resources", async () => {
    const { transport, pushMessage, getSentMessages } = createMockTransport();
    const server = new McpServer(transport);

    server.registerResource(
      { uri: "file:///test.txt", name: "test.txt", mimeType: "text/plain" },
      async () => ({
        contents: [{ uri: "file:///test.txt", mimeType: "text/plain", text: "Hello World" }],
      }),
    );

    const servePromise = server.serve();

    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 1, method: "initialize", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 2, method: "resources/list", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    const sent = getSentMessages();
    const resourcesResponse = sent
      .map((s) => JSON.parse(s) as Response)
      .find((r) => r.id === 2);

    expect(resourcesResponse).toBeDefined();
    const result = resourcesResponse!.result as { resources: Array<{ uri: string; name: string }> };
    expect(result.resources.length).toBe(1);
    expect(result.resources[0]!.uri).toBe("file:///test.txt");
  });

  test("reads a resource", async () => {
    const { transport, pushMessage, getSentMessages } = createMockTransport();
    const server = new McpServer(transport);

    server.registerResource(
      { uri: "file:///data.json", name: "data.json", mimeType: "application/json" },
      async () => ({
        contents: [{ uri: "file:///data.json", mimeType: "application/json", text: '{"key": "value"}' }],
      }),
    );

    const servePromise = server.serve();

    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 1, method: "initialize", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    pushMessage({
      jsonrpc: JSONRPC_VERSION,
      id: 2,
      method: "resources/read",
      params: { uri: "file:///data.json" },
    });
    await new Promise((r) => setTimeout(r, 50));

    const sent = getSentMessages();
    const readResponse = sent
      .map((s) => JSON.parse(s) as Response)
      .find((r) => r.id === 2);

    expect(readResponse).toBeDefined();
    const result = readResponse!.result as { contents: Array<{ text: string }> };
    expect(result.contents[0]!.text).toBe('{"key": "value"}');
  });

  test("handles notifications", async () => {
    const { transport, pushMessage } = createMockTransport();
    const server = new McpServer(transport);

    let notificationReceived = false;
    let receivedMethod = "";
    let receivedParams: unknown = null;

    server.onNotification("test/event", async (method, params) => {
      notificationReceived = true;
      receivedMethod = method;
      receivedParams = params;
    });

    const servePromise = server.serve();

    // Push a notification (no ID)
    pushMessage({
      jsonrpc: JSONRPC_VERSION,
      method: "test/event",
      params: { data: "hello" },
    });

    await new Promise((r) => setTimeout(r, 50));

    expect(notificationReceived).toBe(true);
    expect(receivedMethod).toBe("test/event");
    expect((receivedParams as { data: string }).data).toBe("hello");
  });

  test("returns method not found for unknown methods", async () => {
    const { transport, pushMessage, getSentMessages } = createMockTransport();
    const server = new McpServer(transport);

    const servePromise = server.serve();

    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 1, method: "initialize", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    pushMessage({ jsonrpc: JSONRPC_VERSION, id: 2, method: "unknown/method", params: {} });
    await new Promise((r) => setTimeout(r, 50));

    const sent = getSentMessages();
    const errorResponse = sent
      .map((s) => JSON.parse(s) as Response)
      .find((r) => r.id === 2);

    expect(errorResponse).toBeDefined();
    expect(errorResponse!.error).toBeDefined();
    expect(errorResponse!.error!.message).toContain("method not found");
  });
});
