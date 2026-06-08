import type { ProviderConfig } from "./provider.js";
import { providerTimeout } from "./provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent, AiTokenUsage } from "./types.js";
import { newAiParseError, newAiNetworkError, newAiApiError, newAiRateLimitError } from "./error.js";
import { parseSSEStream } from "./stream.js";

// ---------------------------------------------------------------------------
// OpenAI-compatible provider
// ---------------------------------------------------------------------------

const DEFAULT_OPENAI_BASE_URL = "https://api.openai.com/v1";

// ---------------------------------------------------------------------------
// Internal wire types
// ---------------------------------------------------------------------------

interface OpenAIRequest {
  model: string;
  messages: ChatMessage[];
  tools?: ToolDefinition[];
  stream?: boolean;
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  stop?: string[];
  reasoning_effort?: string;
  thinking?: { type: string };
}

interface OpenAIResponse {
  id: string;
  object: string;
  model: string;
  choices: OpenAIChoice[];
  usage: OpenAIUsage;
}

interface OpenAIChoice {
  index: number;
  message: OpenAIMsg;
  finish_reason: string;
}

interface OpenAIMsg {
  role: string;
  content: string;
  tool_calls?: import("./types.js").ToolCall[];
}

interface OpenAIUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

// Streaming wire types
interface OpenAIStreamChunk {
  id: string;
  object: string;
  choices: OpenAIDeltaChoice[];
}

interface OpenAIDeltaChoice {
  index: number;
  delta: OpenAIDelta;
  finish_reason: string | null;
}

interface OpenAIDelta {
  role?: string;
  content?: string;
  tool_calls?: OpenAIToolDelta[];
}

interface OpenAIToolDelta {
  index: number;
  id?: string;
  type?: string;
  function?: { name?: string; arguments?: string };
}

function throwForStatus(resp: { status: number }, body: string): never {
  if (resp.status === 429) {
    const retryAfterMatch = body.match(/retry[_-]after["\s:]+(\d+)/i);
    const retryAfter = retryAfterMatch ? parseInt(retryAfterMatch[1]!, 10) : 0;
    throw newAiRateLimitError(`rate limited (429): ${body.slice(0, 200)}`, retryAfter);
  }
  throw newAiApiError(`API returned status ${resp.status}: ${body}`, resp.status);
}

// ---------------------------------------------------------------------------
// OpenAIProvider class
// ---------------------------------------------------------------------------

export class OpenAIProvider {
  private config: ProviderConfig;
  private baseURL: string;
  private timeoutMs: number;

  constructor(config: ProviderConfig) {
    this.config = config;
    this.baseURL = config.baseURL || DEFAULT_OPENAI_BASE_URL;
    this.timeoutMs = providerTimeout(config);
  }

  name(): string {
    return "openai";
  }

  // -------------------------------------------------------------------------
  // ChatCompletion (non-streaming)
  // -------------------------------------------------------------------------

  async chatCompletion(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AiResponse> {
    const model = opts.model || this.config.defaultModel;
    const reqBody = this.newOpenAIRequest(model, messages, tools, opts);
    reqBody.stream = false;

    const body = safeMarshal(reqBody);
    const url = `${this.baseURL}/chat/completions`;

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const resp = await fetch(url, {
        method: "POST",
        headers: this.headers(),
        body,
        signal: controller.signal,
      });

      if (!resp.ok) {
        const respBody = await resp.text();
        throwForStatus(resp, respBody);
      }

      const result = (await resp.json()) as OpenAIResponse;
      return this.convertResponse(result);
    } catch (err) {
      if (err instanceof Error && err.name === "AbortError") {
        throw newAiNetworkError(`request timed out after ${this.timeoutMs}ms`);
      }
      throw err;
    } finally {
      clearTimeout(timer);
    }
  }

  // -------------------------------------------------------------------------
  // ChatCompletionStream (streaming)
  // -------------------------------------------------------------------------

  async chatCompletionStream(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AsyncIterable<StreamEvent>> {
    const model = opts.model || this.config.defaultModel;
    const reqBody = this.newOpenAIRequest(model, messages, tools, opts);
    reqBody.stream = true;

    const body = safeMarshal(reqBody);
    const url = `${this.baseURL}/chat/completions`;

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);

    let resp: Response;
    try {
      resp = await fetch(url, {
        method: "POST",
        headers: this.headers(),
        body,
        signal: controller.signal,
      });
    } catch (err) {
      clearTimeout(timer);
      if (err instanceof Error && err.name === "AbortError") {
        throw newAiNetworkError(`request timed out after ${this.timeoutMs}ms`);
      }
      throw err;
    }

    if (!resp.ok) {
      clearTimeout(timer);
      const respBody = await resp.text();
      throwForStatus(resp, respBody);
    }

    return this.readStream(resp, timer);
  }

  // -------------------------------------------------------------------------
  // Internal helpers
  // -------------------------------------------------------------------------

  private headers(): Record<string, string> {
    return {
      "Content-Type": "application/json",
      Authorization: `Bearer ${this.config.apiKey}`,
    };
  }

  private newOpenAIRequest(
    model: string,
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): OpenAIRequest {
    const reqBody: OpenAIRequest = {
      model,
      messages,
      tools: tools.length > 0 ? tools : undefined,
      temperature: opts.temperature,
      max_tokens: opts.max_tokens,
      top_p: opts.top_p,
      stop: opts.stop_sequences,
      reasoning_effort: opts.reasoning_effort,
    };

    // When provider uses thinking format and reasoning effort is specified,
    // swap reasoning_effort for the thinking payload.
    if (
      this.config.extra?.["reasoning_format"] === "thinking" &&
      opts.reasoning_effort &&
      opts.reasoning_effort !== "none"
    ) {
      reqBody.reasoning_effort = undefined;
      reqBody.thinking = { type: "enabled" };
    }

    return reqBody;
  }

  private convertResponse(r: OpenAIResponse): AiResponse {
    const usage: AiTokenUsage = {
      prompt_tokens: r.usage?.prompt_tokens ?? 0,
      completion_tokens: r.usage?.completion_tokens ?? 0,
      total_tokens: r.usage?.total_tokens ?? 0,
    };

    const result: AiResponse = {
      content: "",
      tool_calls: [],
      usage,
      model: r.model ?? "",
      finish_reason: "",
    };

    if (r.choices && r.choices.length > 0) {
      const choice = r.choices[0]!;
      result.content = choice.message?.content ?? "";
      result.finish_reason = choice.finish_reason ?? "";
      result.tool_calls = choice.message?.tool_calls ?? [];
    }

    return result;
  }

  private async *readStream(resp: Response, timer: ReturnType<typeof setTimeout>): AsyncGenerator<StreamEvent> {
    try {
      if (!resp.body) {
        yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
        return;
      }

      const reader = resp.body.getReader();
      const payloads = await parseSSEStream(reader);

      // Accumulate tool call data across chunks
      interface ToolCallAcc {
        id: string;
        name: string;
        arguments: string;
      }
      const toolCalls = new Map<number, ToolCallAcc>();

      for (const payload of payloads) {
        let chunk: OpenAIStreamChunk;
        try {
          chunk = JSON.parse(payload) as OpenAIStreamChunk;
        } catch {
          yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
          return;
        }

        for (const choice of chunk.choices ?? []) {
          // Content delta
          if (choice.delta?.content) {
            yield {
              type: "content_delta",
              content: choice.delta.content,
              tool_call_id: "",
              tool_call_name: "",
              arguments: "",
              usage: null,
            };
          }

          // Tool call deltas - accumulate
          for (const tc of choice.delta?.tool_calls ?? []) {
            let acc = toolCalls.get(tc.index);
            if (!acc) {
              acc = { id: "", name: "", arguments: "" };
              toolCalls.set(tc.index, acc);
            }
            if (tc.id) acc.id += tc.id;
            if (tc.function?.name) acc.name += tc.function.name;
            if (tc.function?.arguments) acc.arguments += tc.function.arguments;
          }

          // Finish
          if (choice.finish_reason != null) {
            // Emit accumulated tool calls
            const sortedKeys = [...toolCalls.keys()].sort((a, b) => a - b);
            for (const key of sortedKeys) {
              const acc = toolCalls.get(key)!;
              yield {
                type: "tool_call_delta",
                content: "",
                tool_call_id: acc.id,
                tool_call_name: acc.name,
                arguments: acc.arguments,
                usage: null,
              };
            }
            yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
            return;
          }
        }
      }

      // Stream ended without explicit finish
      yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
    } finally {
      clearTimeout(timer);
    }
  }
}

/** Creates a new OpenAI-compatible provider with the given config. */
export function newOpenAIProvider(config: ProviderConfig): OpenAIProvider {
  return new OpenAIProvider(config);
}

// ---------------------------------------------------------------------------
// Shared OpenAI-compatible helpers (used by DeepSeek and Qianwen)
// ---------------------------------------------------------------------------

/** Performs a non-streaming OpenAI-compatible request. */
export async function doOpenAIRequest(
  baseURL: string,
  apiKey: string,
  timeoutMs: number,
  reqBody: OpenAIRequest,
): Promise<AiResponse> {
  const body = safeMarshal(reqBody);
  const url = `${baseURL}/chat/completions`;

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const resp = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${apiKey}`,
      },
      body,
      signal: controller.signal,
    });

    if (!resp.ok) {
      const respBody = await resp.text();
      throwForStatus(resp, respBody);
    }

    const result = (await resp.json()) as OpenAIResponse;

    const usage: AiTokenUsage = {
      prompt_tokens: result.usage?.prompt_tokens ?? 0,
      completion_tokens: result.usage?.completion_tokens ?? 0,
      total_tokens: result.usage?.total_tokens ?? 0,
    };

    const aiResp: AiResponse = {
      content: "",
      tool_calls: [],
      usage,
      model: result.model ?? "",
      finish_reason: "",
    };

    if (result.choices && result.choices.length > 0) {
      const choice = result.choices[0]!;
      aiResp.content = choice.message?.content ?? "";
      aiResp.finish_reason = choice.finish_reason ?? "";
      aiResp.tool_calls = choice.message?.tool_calls ?? [];
    }

    return aiResp;
  } catch (err) {
    if (err instanceof Error && err.name === "AbortError") {
      throw newAiNetworkError(`request timed out after ${timeoutMs}ms`);
    }
    throw err;
  } finally {
    clearTimeout(timer);
  }
}

/** Performs a streaming OpenAI-compatible request. */
export async function doOpenAIStreamRequest(
  baseURL: string,
  apiKey: string,
  timeoutMs: number,
  reqBody: OpenAIRequest,
): Promise<AsyncIterable<StreamEvent>> {
  const body = safeMarshal(reqBody);
  const url = `${baseURL}/chat/completions`;

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  let resp: Response;
  try {
    resp = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${apiKey}`,
      },
      body,
      signal: controller.signal,
    });
  } catch (err) {
    clearTimeout(timer);
    if (err instanceof Error && err.name === "AbortError") {
      throw newAiNetworkError(`request timed out after ${timeoutMs}ms`);
    }
    throw err;
  }

  if (!resp.ok) {
    clearTimeout(timer);
    const respBody = await resp.text();
    throwForStatus(resp, respBody);
  }

  return readOpenAIStream(resp, timer);
}

async function* readOpenAIStream(resp: Response, timer: ReturnType<typeof setTimeout>): AsyncGenerator<StreamEvent> {
  try {
    if (!resp.body) {
      yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
      return;
    }

    const reader = resp.body.getReader();
    const payloads = await parseSSEStream(reader);

    interface ToolCallAcc {
      id: string;
      name: string;
      arguments: string;
    }
    const toolCalls = new Map<number, ToolCallAcc>();

    for (const payload of payloads) {
      let chunk: OpenAIStreamChunk;
      try {
        chunk = JSON.parse(payload) as OpenAIStreamChunk;
      } catch {
        yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
        return;
      }

      for (const choice of chunk.choices ?? []) {
        if (choice.delta?.content) {
          yield {
            type: "content_delta",
            content: choice.delta.content,
            tool_call_id: "",
            tool_call_name: "",
            arguments: "",
            usage: null,
          };
        }

        for (const tc of choice.delta?.tool_calls ?? []) {
          let acc = toolCalls.get(tc.index);
          if (!acc) {
            acc = { id: "", name: "", arguments: "" };
            toolCalls.set(tc.index, acc);
          }
          if (tc.id) acc.id += tc.id;
          if (tc.function?.name) acc.name += tc.function.name;
          if (tc.function?.arguments) acc.arguments += tc.function.arguments;
        }

        if (choice.finish_reason != null) {
          const sortedKeys = [...toolCalls.keys()].sort((a, b) => a - b);
          for (const key of sortedKeys) {
            const acc = toolCalls.get(key)!;
            yield {
              type: "tool_call_delta",
              content: "",
              tool_call_id: acc.id,
              tool_call_name: acc.name,
              arguments: acc.arguments,
              usage: null,
            };
          }
          yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
          return;
        }
      }
    }

    yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
  } finally {
    clearTimeout(timer);
  }
}

function safeMarshal(obj: unknown): string {
  try {
    return JSON.stringify(obj);
  } catch (err) {
    throw newAiParseError(`failed to marshal request: ${err instanceof Error ? err.message : String(err)}`);
  }
}
