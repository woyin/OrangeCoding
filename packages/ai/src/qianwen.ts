/**
 * @module qianwen
 *
 * Alibaba Qianwen (Tongyi) provider implementation.
 *
 * Adapts the Qianwen/DashScope API to the AiProvider interface.
 * Supports both chat completion and multi-modal capabilities.
 */
import type { ProviderConfig } from "./provider.js";
import { providerTimeout } from "./provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent } from "./types.js";
import { doOpenAIRequest, doOpenAIStreamRequest } from "./openai.js";

// ---------------------------------------------------------------------------
// Qianwen / Tongyi / DashScope provider (OpenAI-compatible)
// ---------------------------------------------------------------------------

const DEFAULT_QIANWEN_BASE_URL = "https://dashscope.aliyuncs.com/compatible-mode/v1";

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
// QianwenProvider class
// ---------------------------------------------------------------------------

/**
 * QianwenProvider implements the AI provider interface for Alibaba Qianwen /
 * Tongyi / DashScope. The DashScope compatible-mode endpoint is OpenAI-
 * compatible, so requests are delegated to the shared OpenAI helpers.
 */
export class QianwenProvider {
  private config: ProviderConfig;
  private baseURL: string;
  private timeoutMs: number;

  constructor(config: ProviderConfig) {
    this.config = config;
    this.baseURL = config.baseURL || DEFAULT_QIANWEN_BASE_URL;
    this.timeoutMs = providerTimeout(config);
  }

  name(): string {
    return "qianwen";
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

/** Creates a new Qianwen provider with the given config. */
export function newQianwenProvider(config: ProviderConfig): QianwenProvider {
  return new QianwenProvider(config);
}
