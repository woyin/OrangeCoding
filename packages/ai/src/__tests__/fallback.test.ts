import { jest } from "@jest/globals";
import { FallbackChain } from "../fallback.js";
import type { AiProvider } from "../provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent } from "../types.js";

class MockProvider implements AiProvider {
  constructor(
    private readonly _name: string,
    private readonly _shouldFail: boolean = false,
    private readonly _response: string = "ok",
  ) {}

  name(): string { return this._name; }

  async chatCompletion(
    _messages: ChatMessage[],
    _tools: ToolDefinition[],
    _opts: ChatOptions,
  ): Promise<AiResponse> {
    if (this._shouldFail) {
      throw new Error(`${this._name} failed`);
    }
    return { content: this._response, usage: { prompt_tokens: 10, completion_tokens: 5 } };
  }

  async chatCompletionStream(
    _messages: ChatMessage[],
    _tools: ToolDefinition[],
    _opts: ChatOptions,
  ): Promise<AsyncIterable<StreamEvent>> {
    if (this._shouldFail) {
      throw new Error(`${this._name} stream failed`);
    }
    const content = this._response;
    return {
      async *[Symbol.asyncIterator]() {
        yield { type: "content_delta" as const, content };
        yield { type: "usage" as const, usage: { prompt_tokens: 10, completion_tokens: 5 } };
      },
    };
  }
}

describe("FallbackChain", () => {
  it("returns the name of all providers", () => {
    const chain = new FallbackChain(
      [new MockProvider("openai"), new MockProvider("anthropic")],
      5000,
    );
    expect(chain.name()).toBe("fallback[openai, anthropic]");
  });

  it("uses the first provider when it succeeds", async () => {
    const chain = new FallbackChain(
      [new MockProvider("openai", false, "from-openai"), new MockProvider("anthropic", false, "from-anthropic")],
      5000,
    );
    const result = await chain.chatCompletion([], [], {} as ChatOptions);
    expect(result.content).toBe("from-openai");
  });

  it("falls back to the second provider when the first fails", async () => {
    const chain = new FallbackChain(
      [new MockProvider("openai", true), new MockProvider("anthropic", false, "from-anthropic")],
      5000,
    );
    const result = await chain.chatCompletion([], [], {} as ChatOptions);
    expect(result.content).toBe("from-anthropic");
  });

  it("skips providers that are on cooldown", async () => {
    const chain = new FallbackChain(
      [new MockProvider("openai", true), new MockProvider("anthropic", false, "from-anthropic")],
      5000,
    );
    // First call: openai fails, anthropic succeeds
    await chain.chatCompletion([], [], {} as ChatOptions);

    // openai should now be on cooldown
    expect(chain.isCoolingDown(0)).toBe(true);

    // Second call: openai is skipped (cooldown), anthropic succeeds
    const result = await chain.chatCompletion([], [], {} as ChatOptions);
    expect(result.content).toBe("from-anthropic");
  });

  it("throws when all providers fail", async () => {
    const chain = new FallbackChain(
      [new MockProvider("openai", true), new MockProvider("anthropic", true)],
      5000,
    );
    await expect(chain.chatCompletion([], [], {} as ChatOptions)).rejects.toThrow();
  });

  it("falls back for streaming calls too", async () => {
    const chain = new FallbackChain(
      [new MockProvider("openai", true), new MockProvider("anthropic", false, "streamed")],
      5000,
    );
    const stream = await chain.chatCompletionStream([], [], {} as ChatOptions);
    const events: StreamEvent[] = [];
    for await (const event of stream) {
      events.push(event);
    }
    expect(events.length).toBeGreaterThan(0);
    expect(events[0]!.type).toBe("content_delta");
  });
});
