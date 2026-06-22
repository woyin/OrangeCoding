/**
 * OpenAI-compatible chat-completions provider.
 *
 * Implements the OpenAI Chat Completions API contract
 * (POST /v1/chat/completions) and is reused, via the `doOpenAIRequest` /
 * `doOpenAIStreamRequest` helpers, by DeepSeek and Qianfan/Wenxin providers
 * that speak the same wire protocol.
 *
 * Two transport modes:
 *   - chatCompletion:       single non-streaming request/response
 *   - chatCompletionStream: server-sent-events (SSE) async iterable
 *
 * Streaming tool-call assembly: OpenAI streams tool calls as incremental
 * deltas keyed by `index` (the call's position in the response), with the
 * `id`, function `name`, and `arguments` arriving across many chunks. We
 * accumulate these in a Map<number, ToolCallAcc> and emit one
 * `tool_call_delta` StreamEvent per tool call at finish_reason, preserving
 * index order.
 */

import type { ProviderConfig } from "./provider.js";
import { providerTimeout } from "./provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent, AiTokenUsage } from "./types.js";
import { newAiParseError, newAiNetworkError, newAiApiError, newAiRateLimitError } from "./error.js";
import { parseSSEStream } from "./stream.js";

// ---------------------------------------------------------------------------
// Provider defaults
// ---------------------------------------------------------------------------

/** Default API base when ProviderConfig.baseURL is unset. */
const DEFAULT_OPENAI_BASE_URL = "https://api.openai.com/v1";

// ---------------------------------------------------------------------------
// Wire types (subset of the OpenAI Chat Completions schema we read/write)
// ---------------------------------------------------------------------------

/** Request body for POST /chat/completions. */
interface OpenAIRequest {
  model: string;
  messages: ChatMessage[];
  tools?: ToolDefinition[];
  stream?: boolean;
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  stop?: string[];
  /** "low" | "medium" | "high" for o-series reasoning models. */
  reasoning_effort?: string;
  /** Alternate reasoning envelope used by some Anthropic-compatible gateways. */
  thinking?: { type: string };
}

/** Non-streaming response: choices[0] holds the generated message. */
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

// Streaming wire types.
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

/** A single streamed delta. Fields are optional; only present fields apply. */
interface OpenAIDelta {
  role?: string;
  content?: string;
  tool_calls?: OpenAIToolDelta[];
}

/**
 * Incremental tool-call delta. `index` identifies which tool call this updates
 * (a model may stream several tool calls interleaved); the id/name/arguments
 * fields arrive piecemeal and must be concatenated per index.
 */
interface OpenAIToolDelta {
  index: number;
  id?: string;
  type?: string;
  function?: { name?: string; arguments?: string };
}

// ---------------------------------------------------------------------------
// Shared HTTP helpers
// ---------------------------------------------------------------------------

/**
 * Maps a non-2xx HTTP response to the appropriate typed error:
 *   - 429  -> AiRateLimitError (with parsed retry-after, if present)
 *   - else -> AiApiError
 * Always throws; the return type `never` encodes that.
 */
function throwForStatus(resp: { status: number }, body: string): never {
  if (resp.status === 429) {
    // Retry-After may arrive as a header (handled by callers) or, for some
    // OpenAI-compatible gateways, embedded in the JSON body.
    const retryAfterMatch = body.match(/retry[_-]after["\s:]+(\d+)/i);
    const retryAfter = retryAfterMatch ? parseInt(retryAfterMatch[1]!, 10) : 0;
    throw newAiRateLimitError(`rate limited (429): ${body.slice(0, 200)}`, retryAfter);
  }
  throw newAiApiError(`API returned status ${resp.status}: ${body}`, resp.status);
}

/**
 * JSON-stringify with a typed error wrapper. Guards against circular-reference
 * or BigInt failures surfacing as an opaque TypeError.
 */
function safeMarshal(obj: unknown): string {
  try {
    return JSON.stringify(obj);
  } catch (err) {
    throw newAiParseError(`failed to marshal request: ${err instanceof Error ? err.message : String(err)}`);
  }
}

/** Standard request headers for the OpenAI-compatible API. */
function openAIHeaders(apiKey: string): Record<string, string> {
  return {
    "Content-Type": "application/json",
    Authorization: `Bearer ${apiKey}`,
  };
}

// ---------------------------------------------------------------------------
// Shared response/stream converters (used by OpenAIProvider + standalone helpers)
// ---------------------------------------------------------------------------

/**
 * Accumulator for a single streamed tool call, keyed by delta.index. Fields are
 * concatenated across deltas because OpenAI splits a tool call across many
 * SSE chunks (id in the first, arguments split across all).
 */
interface ToolCallAcc {
  id: string;
  name: string;
  arguments: string;
}

/**
 * Converts a non-streaming OpenAIResponse into the provider-agnostic AiResponse.
 * Centralized here so OpenAIProvider and doOpenAIRequest share one code path.
 */
function openAIResponseToAiResponse(r: OpenAIResponse): AiResponse {
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

  // choices is always present in a successful response; we read [0] because
  // n defaults to 1 and we never request n>1.
  if (r.choices && r.choices.length > 0) {
    const choice = r.choices[0]!;
    result.content = choice.message?.content ?? "";
    result.finish_reason = choice.finish_reason ?? "";
    result.tool_calls = choice.message?.tool_calls ?? [];
  }

  return result;
}

/**
 * 读取 OpenAI 兼容 SSE 流并产出 StreamEvent。
 *
 * - 文本增量立即转成 `content_delta`（低延迟流式）
 * - 工具调用按 delta.index 聚合到 Map（O(1) 插入）
 * - finish_reason 时按 index 升序 flush 出每个 `tool_call_delta`，最后发 `done`
 *
 * 该生成器被 OpenAIProvider 与 readOpenAIStream 共用，集中维护单一代码路径。
 *
 * Reads an OpenAI-compatible SSE stream and yields StreamEvents.
 */
async function* readOpenAIStream(resp: Response, timer: ReturnType<typeof setTimeout>): AsyncGenerator<StreamEvent> {
  // Sentinel object emitted for terminal/empty-stream cases; reused to avoid
  // allocating distinct objects for the common "stream done" path.
  const DONE: StreamEvent = { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };

  try {
    if (!resp.body) {
      yield DONE;
      return;
    }

    const reader = resp.body.getReader();
    const payloads = await parseSSEStream(reader);

    // Map<deltaIndex, ToolCallAcc> — O(1) insert per delta, sorted emit at end.
    const toolCalls = new Map<number, ToolCallAcc>();

    for (const payload of payloads) {
      let chunk: OpenAIStreamChunk;
      try {
        chunk = JSON.parse(payload) as OpenAIStreamChunk;
      } catch {
        // Malformed SSE payload — abort the stream cleanly rather than throw,
        // matching OpenAI SDK behavior on partial/truncated events.
        yield DONE;
        return;
      }

      for (const choice of chunk.choices ?? []) {
        // Text content delta: forward immediately for low-latency streaming.
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

        // Tool-call deltas: accumulate by index. id/name typically arrive once
        // (first chunk for that index); arguments stream across many chunks.
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

        // finish_reason marks end-of-stream for this choice. Flush accumulated
        // tool calls in index order, then the terminal done event.
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
          yield DONE;
          return;
        }
      }
    }

    // Stream ended without an explicit finish_reason — still emit done so the
    // consumer's loop terminates.
    yield DONE;
  } finally {
    clearTimeout(timer);
  }
}

// ---------------------------------------------------------------------------
// OpenAIProvider class
// ---------------------------------------------------------------------------

/**
 * OpenAIProvider 实现 OpenAI Chat Completions 协议的 provider。
 *
 * 兼容任何遵循该协议的端点（通过 ProviderConfig.baseURL 指向 DeepSeek、
 * 本地推理服务等）。同时提供非流式 ({@link chatCompletion}) 与流式
 * ({@link chatCompletionStream}) 两种调用方式；流式响应通过 SSE 增量解析，
 * 工具调用按 delta.index 聚合后按顺序输出。
 *
 * OpenAIProvider 实现 provider 接口，对接 OpenAI Chat Completions API
 * （以及任何兼容端点，通过 ProviderConfig.baseURL）。
 */
export class OpenAIProvider {
  private config: ProviderConfig;
  private baseURL: string;
  private timeoutMs: number;

  constructor(config: ProviderConfig) {
    this.config = config;
    this.baseURL = config.baseURL || DEFAULT_OPENAI_BASE_URL;
    this.timeoutMs = providerTimeout(config);
  }

  /** Provider identifier used in logs/metrics. */
  name(): string {
    return "openai";
  }

  // -------------------------------------------------------------------------
  // ChatCompletion (non-streaming)
  // -------------------------------------------------------------------------

  /**
   * Sends a non-streaming chat-completion request and returns the full
   * generated message (content + tool calls + usage) in one AiResponse.
   */
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
        headers: openAIHeaders(this.config.apiKey),
        body,
        signal: controller.signal,
      });

      if (!resp.ok) {
        const respBody = await resp.text();
        throwForStatus(resp, respBody);
      }

      const result = (await resp.json()) as OpenAIResponse;
      return openAIResponseToAiResponse(result);
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
  // ChatCompletionStream (streaming via SSE)
  // -------------------------------------------------------------------------

  /**
   * Sends a streaming chat-completion request and returns an AsyncIterable of
   * StreamEvent (content_delta / tool_call_delta / done). The HTTP timeout is
   * held open for the lifetime of the stream and cleared on completion.
   */
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
        headers: openAIHeaders(this.config.apiKey),
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

    return readOpenAIStream(resp, timer);
  }

  // -------------------------------------------------------------------------
  // Internal: request building
  // -------------------------------------------------------------------------

  /**
   * Builds the OpenAI request body from provider options. Omits undefined
   * fields so the wire payload stays minimal (OpenAI rejects unknown nulls).
   *
   * Handles the reasoning_format toggle: when the provider is configured with
   * `reasoning_format=thinking` and a reasoning_effort is set, we swap the
   * OpenAI `reasoning_effort` field for the alternate `thinking` envelope
   * used by some Anthropic-compatible gateways.
   */
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
}

/** 按给定配置创建一个 OpenAI 兼容 provider 实例。 */
export function newOpenAIProvider(config: ProviderConfig): OpenAIProvider {
  return new OpenAIProvider(config);
}

// ---------------------------------------------------------------------------
// Shared OpenAI-compatible helpers (used by DeepSeek and Qianfan/Wenxin)
// ---------------------------------------------------------------------------

/**
 * Performs a non-streaming OpenAI-compatible request. Reused by providers that
 * share the OpenAI wire format but differ in base URL / auth (DeepSeek, Wenxin).
 */
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
      headers: openAIHeaders(apiKey),
      body,
      signal: controller.signal,
    });

    if (!resp.ok) {
      const respBody = await resp.text();
      throwForStatus(resp, respBody);
    }

    const result = (await resp.json()) as OpenAIResponse;
    return openAIResponseToAiResponse(result);
  } catch (err) {
    if (err instanceof Error && err.name === "AbortError") {
      throw newAiNetworkError(`request timed out after ${timeoutMs}ms`);
    }
    throw err;
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Performs a streaming OpenAI-compatible request. Returns the SSE stream as an
 * AsyncIterable<StreamEvent>. Used by DeepSeek and Wenxin streaming providers.
 */
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
      headers: openAIHeaders(apiKey),
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
