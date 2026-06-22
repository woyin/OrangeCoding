/**
 * @module deepseek
 *
 * DeepSeek provider implementation.
 *
 * Adapts the DeepSeek API (OpenAI-compatible) with DeepSeek-specific
 * features like deep thinking mode and code generation optimizations.
 */
import type { ProviderConfig } from "./provider.js";
import { providerTimeout } from "./provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent } from "./types.js";
import { doOpenAIRequest, doOpenAIStreamRequest } from "./openai.js";

// ---------------------------------------------------------------------------
// DeepSeek provider (OpenAI-compatible)
// ---------------------------------------------------------------------------

const DEFAULT_DEEPSEEK_BASE_URL = "https://api.deepseek.com/v1";

// Re-export the OpenAI wire type for the shared helper
interface OpenAIRequest {
  model: string;
  messages: ChatMessage[];
  tools?: ToolDefinition[];
  stream?: boolean;
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  stop?: string[];
}

// ---------------------------------------------------------------------------
// DeepSeekProvider class
// ---------------------------------------------------------------------------

/**
 * DeepSeekProvider implements the AI provider interface for the DeepSeek API.
 * DeepSeek is OpenAI-compatible, so chat and streaming requests are delegated
 * to the shared OpenAI request helpers with the DeepSeek base URL.
 */
export class DeepSeekProvider {
  private config: ProviderConfig;
  private baseURL: string;
  private timeoutMs: number;

  constructor(config: ProviderConfig) {
    this.config = config;
    this.baseURL = config.baseURL || DEFAULT_DEEPSEEK_BASE_URL;
    this.timeoutMs = providerTimeout(config);
  }

  name(): string {
    return "deepseek";
  }

  // -------------------------------------------------------------------------
  // ChatCompletion (non-streaming) — delegates to OpenAI-compatible format
  // -------------------------------------------------------------------------

  async chatCompletion(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AiResponse> {
    const model = opts.model || this.config.defaultModel;

    const reqBody: OpenAIRequest = {
      model,
      messages,
      tools: tools.length > 0 ? tools : undefined,
      temperature: opts.temperature,
      max_tokens: opts.max_tokens,
      top_p: opts.top_p,
      stop: opts.stop_sequences,
    };

    return doOpenAIRequest(this.baseURL, this.config.apiKey, this.timeoutMs, reqBody);
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

    const reqBody: OpenAIRequest = {
      model,
      messages,
      tools: tools.length > 0 ? tools : undefined,
      stream: true,
      temperature: opts.temperature,
      max_tokens: opts.max_tokens,
      top_p: opts.top_p,
      stop: opts.stop_sequences,
    };

    return doOpenAIStreamRequest(this.baseURL, this.config.apiKey, this.timeoutMs, reqBody);
  }
}

/** Creates a new DeepSeek provider with the given config. */
export function newDeepSeekProvider(config: ProviderConfig): DeepSeekProvider {
  return new DeepSeekProvider(config);
}
