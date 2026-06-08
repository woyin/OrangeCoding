import type { ProviderConfig } from "./provider.js";
import { providerTimeout } from "./provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent, AiTokenUsage, ToolCall } from "./types.js";
import { newAiParseError, newAiNetworkError, newAiApiError, newAiRateLimitError } from "./error.js";
import { parseSSEStream } from "./stream.js";

// ---------------------------------------------------------------------------
// Anthropic / Claude provider
// ---------------------------------------------------------------------------

const DEFAULT_ANTHROPIC_BASE_URL = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION = "2023-06-01";

// ---------------------------------------------------------------------------
// Anthropic wire types
// ---------------------------------------------------------------------------

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

interface AnthropicThinking {
  type: string;
  budget_tokens: number;
}

interface AnthropicTool {
  name: string;
  description: string;
  input_schema: unknown;
}

interface AnthropicResponse {
  id: string;
  type: string;
  role: string;
  content: AnthropicContent[];
  model: string;
  stop_reason: string;
  usage: AnthropicUsage;
}

interface AnthropicContent {
  type: string;
  text?: string;
  id?: string;
  name?: string;
  input?: string; // JSON string
}

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

  async chatCompletion(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AiResponse> {
    const model = opts.model || this.config.defaultModel;

    const { systemPrompt, filtered } = this.extractSystemPrompt(messages);

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
  // ChatCompletionStream (streaming)
  // -------------------------------------------------------------------------

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

  private headers(): Record<string, string> {
    return {
      "Content-Type": "application/json",
      "x-api-key": this.config.apiKey,
      "anthropic-version": ANTHROPIC_VERSION,
    };
  }

  /** Removes system messages from the list and joins them into a single string. */
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

  /** Converts ToolDefinition array to Anthropic's tool format. */
  private convertTools(tools: ToolDefinition[]): AnthropicTool[] {
    return tools.map((t) => ({
      name: t.function.name,
      description: t.function.description,
      input_schema: t.function.parameters,
    }));
  }

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

function ensureAnthropicThinkingRoom(maxTokens: number, thinkingBudget?: number): number {
  if (!thinkingBudget || thinkingBudget === 0) {
    return maxTokens;
  }
  if (maxTokens > thinkingBudget) {
    return maxTokens;
  }
  return thinkingBudget + 1024;
}

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
