/**
 * @module anthropic
 *
 * Anthropic Claude provider implementation.
 *
 * Adapts the Anthropic Messages API to the AiProvider interface.
 * Handles:
 * - Claude-specific message format (system prompt separate from messages)
 * - Extended thinking / reasoning tokens
 * - Tool use (function calling) in Claude format
 * - Streaming via Server-Sent Events
 * - Rate limiting and error handling
 */
import type { ProviderConfig } from "./provider.js";
import { providerTimeout } from "./provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent, AiTokenUsage, ToolCall } from "./types.js";
import { newAiParseError, newAiNetworkError, newAiApiError, newAiRateLimitError } from "./error.js";
import { parseSSEStream } from "./stream.js";

/**
 * Anthropic (Claude) Messages API provider.
 *
 * Implements the Anthropic Messages API (POST /v1/messages). Differs from the
 * OpenAI-compatible protocol in three key ways this file adapts for:
 *
 *   1. System prompt is a top-level `system` field, NOT a message in the
 *      `messages` array. extractSystemPrompt() pulls system-role messages out
 *      and joins them into the single `system` string Anthropic expects.
 *
 *   2. Tools are declared with `input_schema` (JSON Schema) rather than the
 *      OpenAI `function.parameters` wrapper. convertTools() reshapes them.
 *
 *   3. Extended thinking: when reasoning_budget_tokens > 0, we send a
 *      `thinking` envelope and must reserve headroom for the thinking budget
 *      inside max_tokens (see ensureAnthropicThinkingRoom).
 *
 * Streaming uses Anthropic's own SSE event types (content_block_delta,
 * content_block_start, message_delta, message_stop), distinct from OpenAI's
 * chunk shape. readStream() maps these onto the shared StreamEvent type.
 */

// ---------------------------------------------------------------------------
// Provider defaults
// ---------------------------------------------------------------------------

/** Default API base when ProviderConfig.baseURL is unset. */
const DEFAULT_ANTHROPIC_BASE_URL = "https://api.anthropic.com/v1";

/** Anthropic API version header (pins the Messages API schema). */
const ANTHROPIC_VERSION = "2023-06-01";

// ---------------------------------------------------------------------------
// Anthropic wire types
// ---------------------------------------------------------------------------

/** Request body for POST /v1/messages. `system` is separate from `messages`. */
interface AnthropicRequest {
  model: string;
  messages: ChatMessage[];
  system?: string;
  max_tokens: number;
  tools?: AnthropicTool[];
  stream?: boolean;
  temperature?: number;
  top_p?: number;
  stop_sequences?: string[];
  thinking?: AnthropicThinking;
}

/** Extended-thinking envelope: type=enabled with a token budget. */
interface AnthropicThinking {
  type: string;
  budget_tokens: number;
}

/** Anthropic tool definition: uses input_schema (JSON Schema), not OpenAI's function wrapper. */
interface AnthropicTool {
  name: string;
  description: string;
  input_schema: unknown;
}

/** Non-streaming Messages response. content[] is a sequence of typed blocks. */
interface AnthropicResponse {
  id: string;
  type: string;
  role: string;
  content: AnthropicContent[];
  model: string;
  stop_reason: string;
  usage: AnthropicUsage;
}

/** One block of a response: "text" or "tool_use" (id/name/input). */
interface AnthropicContent {
  type: string;
  text?: string;
  id?: string;
  name?: string;
  input?: string; // JSON string
}

/** Anthropic usage: input + output tokens (no separate total; we sum them). */
interface AnthropicUsage {
  input_tokens: number;
  output_tokens: number;
}

// Streaming wire types
interface AnthropicStreamEvent {
  type: string;
  index?: number;
  content_block?: AnthropicContentBlock;
  delta?: AnthropicDelta;
  message?: AnthropicMsgDelta;
}

interface AnthropicContentBlock {
  type: string;
  id?: string;
  name?: string;
  input?: string;
  text?: string;
}

interface AnthropicDelta {
  type?: string;
  text?: string;
  partial_json?: string;
  stop_reason?: string;
}

interface AnthropicMsgDelta {
  stop_reason?: string;
  usage?: AnthropicUsage;
}

// ---------------------------------------------------------------------------
// AnthropicProvider class
// ---------------------------------------------------------------------------

/**
 * AnthropicProvider implements the provider interface against the Anthropic
 * Messages API. Handles system-prompt extraction, tool reshaping, extended
 * thinking, and Anthropic-format SSE streaming.
 */
export class AnthropicProvider {
  private config: ProviderConfig;
  private baseURL: string;
  private timeoutMs: number;

  constructor(config: ProviderConfig) {
    this.config = config;
    this.baseURL = config.baseURL || DEFAULT_ANTHROPIC_BASE_URL;
    this.timeoutMs = providerTimeout(config);
  }

  name(): string {
    return "anthropic";
  }

  // -------------------------------------------------------------------------
  // ChatCompletion (non-streaming)
  // -------------------------------------------------------------------------

  /**
   * Sends a non-streaming Messages request and returns the full AiResponse.
   * Extracts the system prompt, reshapes tools, reserves thinking headroom,
   * then parses the content blocks (text + tool_use) into AiResponse.
   */
  async chatCompletion(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AiResponse> {
    const model = opts.model || this.config.defaultModel;

    // Anthropic requires system content as a top-level field, not a message.
    const { systemPrompt, filtered } = this.extractSystemPrompt(messages);

    // Reserve headroom for the thinking budget if extended thinking is on.
    let maxTokens = opts.max_tokens ?? 4096;
    maxTokens = ensureAnthropicThinkingRoom(maxTokens, opts.reasoning_budget_tokens);

    const reqBody: AnthropicRequest = {
      model,
      messages: filtered,
      system: systemPrompt,
      max_tokens: maxTokens,
      temperature: opts.temperature,
      top_p: opts.top_p,
      stop_sequences: opts.stop_sequences,
    };

    if (opts.reasoning_budget_tokens && opts.reasoning_budget_tokens > 0) {
      reqBody.thinking = { type: "enabled", budget_tokens: opts.reasoning_budget_tokens };
    }

    if (tools.length > 0) {
      reqBody.tools = this.convertTools(tools);
    }

    const body = safeMarshal(reqBody);
    const url = `${this.baseURL}/messages`;

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
        if (resp.status === 429) {
          const retryAfter = parseInt(resp.headers.get("retry-after") ?? "0", 10);
          throw newAiRateLimitError(`rate limited (429): ${respBody.slice(0, 200)}`, retryAfter);
        }
        throw newAiApiError(
          `API returned status ${resp.status}: ${respBody}`,
          resp.status,
        );
      }

      const result = (await resp.json()) as AnthropicResponse;
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
  // ChatCompletionStream (streaming via Anthropic SSE)
  // -------------------------------------------------------------------------

  /**
   * Sends a streaming Messages request. Returns an AsyncIterable<StreamEvent>
   * that yields content_delta, tool_call_delta, usage, and done events as
   * Anthropic SSE events arrive. The HTTP timeout covers the whole stream.
   */
  async chatCompletionStream(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AsyncIterable<StreamEvent>> {
    const model = opts.model || this.config.defaultModel;
    const { systemPrompt, filtered } = this.extractSystemPrompt(messages);

    let maxTokens = opts.max_tokens ?? 4096;
    maxTokens = ensureAnthropicThinkingRoom(maxTokens, opts.reasoning_budget_tokens);

    const reqBody: AnthropicRequest = {
      model,
      messages: filtered,
      system: systemPrompt,
      max_tokens: maxTokens,
      stream: true,
      temperature: opts.temperature,
      top_p: opts.top_p,
      stop_sequences: opts.stop_sequences,
    };

    if (opts.reasoning_budget_tokens && opts.reasoning_budget_tokens > 0) {
      reqBody.thinking = { type: "enabled", budget_tokens: opts.reasoning_budget_tokens };
    }

    if (tools.length > 0) {
      reqBody.tools = this.convertTools(tools);
    }

    const body = safeMarshal(reqBody);
    const url = `${this.baseURL}/messages`;

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
      if (resp.status === 429) {
        const retryAfter = parseInt(resp.headers.get("retry-after") ?? "0", 10);
        throw newAiRateLimitError(`rate limited (429): ${respBody.slice(0, 200)}`, retryAfter);
      }
      throw newAiApiError(
        `API returned status ${resp.status}: ${respBody}`,
        resp.status,
      );
    }

    return this.readStream(resp, timer);
  }

  // -------------------------------------------------------------------------
  // Internal helpers
  // -------------------------------------------------------------------------

  /** Anthropic auth headers: x-api-key + the pinned API version. */
  private headers(): Record<string, string> {
    return {
      "Content-Type": "application/json",
      "x-api-key": this.config.apiKey,
      "anthropic-version": ANTHROPIC_VERSION,
    };
  }

  /**
   * Removes system-role messages from the list and joins them into one string.
   * Anthropic's API takes `system` as a top-level field rather than a message,
   * so we must split it out of the OpenAI-style messages array before sending.
   * Non-system messages are returned in original order.
   */
  private extractSystemPrompt(
    messages: ChatMessage[],
  ): { systemPrompt: string; filtered: ChatMessage[] } {
    const systemParts: string[] = [];
    const filtered: ChatMessage[] = [];
    for (const msg of messages) {
      if (msg.role === "system") {
        systemParts.push(msg.content ?? "");
      } else {
        filtered.push(msg);
      }
    }
    return { systemPrompt: systemParts.join("\n"), filtered };
  }

  /**
   * Converts provider ToolDefinitions to Anthropic's tool format. Anthropic
   * uses `input_schema` (a raw JSON Schema) instead of OpenAI's
   * `{ function: { parameters } }` wrapper.
   */
  private convertTools(tools: ToolDefinition[]): AnthropicTool[] {
    return tools.map((t) => ({
      name: t.function.name,
      description: t.function.description,
      input_schema: t.function.parameters,
    }));
  }

  /**
   * Parses an Anthropic Messages response into the provider-agnostic AiResponse.
   * Walks content blocks: "text" blocks concatenate into `content`; "tool_use"
   * blocks become ToolCall entries (input is JSON-stringified for the wire).
   * Usage is remapped from input/output_tokens to the shared AiTokenUsage.
   */
  private convertResponse(r: AnthropicResponse): AiResponse {
    const usage: AiTokenUsage = {
      prompt_tokens: r.usage?.input_tokens ?? 0,
      completion_tokens: r.usage?.output_tokens ?? 0,
      total_tokens: (r.usage?.input_tokens ?? 0) + (r.usage?.output_tokens ?? 0),
    };

    const result: AiResponse = {
      content: "",
      tool_calls: [],
      usage,
      model: r.model ?? "",
      finish_reason: r.stop_reason ?? "",
    };

    const contentParts: string[] = [];
    for (const block of r.content ?? []) {
      switch (block.type) {
        case "text":
          contentParts.push(block.text ?? "");
          break;
        case "tool_use": {
          let args = "{}";
          if (block.input && typeof block.input === "string" && block.input.length > 0) {
            args = block.input;
          } else if (block.input && typeof block.input === "object") {
            args = JSON.stringify(block.input);
          }
          result.tool_calls.push({
            id: block.id ?? "",
            type: "function",
            function: {
              name: block.name ?? "",
              arguments: args,
            },
          } satisfies ToolCall);
          break;
        }
      }
    }
    result.content = contentParts.join("");

    return result;
  }

  /**
   * Reads Anthropic's SSE event stream and yields StreamEvents.
   *
   * Event types handled:
   *   - content_block_delta / text_delta:    -> content_delta (streamed text)
   *   - content_block_delta / input_json_delta: -> tool_call_delta (args chunk)
   *   - content_block_start / tool_use:      -> tool_call_delta (id + name)
   *   - message_delta (stop_reason set):     -> usage + done (terminal)
   *   - message_stop:                        -> done (terminal)
   *
   * Unlike OpenAI, tool-call arguments stream as partial_json deltas rather
   * than being accumulated server-side; we forward each fragment directly.
   */
  private async *readStream(resp: Response, timer: ReturnType<typeof setTimeout>): AsyncGenerator<StreamEvent> {
    try {
      if (!resp.body) {
        yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
        return;
      }

      const reader = resp.body.getReader();
      const payloads = await parseSSEStream(reader);

      for (const payload of payloads) {
        let evt: AnthropicStreamEvent;
        try {
          evt = JSON.parse(payload) as AnthropicStreamEvent;
        } catch {
          continue;
        }

        switch (evt.type) {
          case "content_block_delta":
            if (evt.delta) {
              switch (evt.delta.type) {
                case "text_delta":
                  yield {
                    type: "content_delta",
                    content: evt.delta.text ?? "",
                    tool_call_id: "",
                    tool_call_name: "",
                    arguments: "",
                    usage: null,
                  };
                  break;
                case "input_json_delta":
                  // Tool call arguments being streamed
                  yield {
                    type: "tool_call_delta",
                    content: "",
                    tool_call_id: "",
                    tool_call_name: "",
                    arguments: evt.delta.partial_json ?? "",
                    usage: null,
                  };
                  break;
              }
            }
            break;

          case "content_block_start":
            if (evt.content_block?.type === "tool_use") {
              yield {
                type: "tool_call_delta",
                content: "",
                tool_call_id: evt.content_block.id ?? "",
                tool_call_name: evt.content_block.name ?? "",
                arguments: "",
                usage: null,
              };
            }
            break;

          case "message_delta":
            if (evt.delta?.stop_reason) {
              const usage: AiTokenUsage = {
                prompt_tokens: evt.message?.usage?.input_tokens ?? 0,
                completion_tokens: evt.message?.usage?.output_tokens ?? 0,
                total_tokens:
                  (evt.message?.usage?.input_tokens ?? 0) + (evt.message?.usage?.output_tokens ?? 0),
              };
              yield {
                type: "usage",
                content: "",
                tool_call_id: "",
                tool_call_name: "",
                arguments: "",
                usage,
              };
              yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
              return;
            }
            break;

          case "message_stop":
            yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
            return;
        }
      }

      yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
    } finally {
      clearTimeout(timer);
    }
  }
}

/**
 * Ensures max_tokens leaves room for the extended-thinking budget.
 * Anthropic requires max_tokens > thinking.budget_tokens (the output budget
 * must exceed the thinking budget). If the caller's max_tokens is too small,
 * we bump it to budget + 1024. No-op when thinking is disabled (budget=0).
 */
function ensureAnthropicThinkingRoom(maxTokens: number, thinkingBudget?: number): number {
  if (!thinkingBudget || thinkingBudget === 0) {
    return maxTokens;
  }
  if (maxTokens > thinkingBudget) {
    return maxTokens;
  }
  return thinkingBudget + 1024;
}

/** JSON-stringify with a typed error wrapper (guards against circular refs). */
function safeMarshal(obj: unknown): string {
  try {
    return JSON.stringify(obj);
  } catch (err) {
    throw newAiParseError(`failed to marshal request: ${err instanceof Error ? err.message : String(err)}`);
  }
}

/** Creates a new Anthropic provider with the given config. */
export function newAnthropicProvider(config: ProviderConfig): AnthropicProvider {
  return new AnthropicProvider(config);
}
