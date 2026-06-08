import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent } from "./types.js";

// ---------------------------------------------------------------------------
// AiProvider interface
// ---------------------------------------------------------------------------

/**
 * Interface that all AI provider adapters must implement.
 */
export interface AiProvider {
  /** Returns the provider's display name. */
  name(): string;

  /** Sends a non-streaming chat completion request. */
  chatCompletion(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AiResponse>;

  /** Sends a streaming chat completion request and returns an async iterable of events. */
  chatCompletionStream(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AsyncIterable<StreamEvent>>;
}

// ---------------------------------------------------------------------------
// ProviderConfig
// ---------------------------------------------------------------------------

/**
 * Configuration for creating an AI provider.
 */
export interface ProviderConfig {
  apiKey: string;
  apiSecret: string;
  baseURL: string;
  defaultModel: string;
  timeoutSecs: number;
  extra: Record<string, string>;
}

const DEFAULT_TIMEOUT_MS = 120_000;

/** Returns the configured timeout or the default of 120 seconds. */
export function providerTimeout(config: ProviderConfig): number {
  return config.timeoutSecs > 0 ? config.timeoutSecs * 1000 : DEFAULT_TIMEOUT_MS;
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_MOONSHOT_BASE_URL = "https://api.moonshot.ai/v1";
const DEFAULT_BIGMODEL_BASE_URL = "https://api.z.ai/api/paas/v4";

const DEFAULT_OPENAI_MODEL = "gpt-5.1";
const DEFAULT_ANTHROPIC_MODEL = "claude-opus-4-7";
const DEFAULT_KIMI_MODEL = "kimi-k2.6";
const DEFAULT_GLM_MODEL = "glm-5.1";

function usesThinkingReasoningFormat(name: string): boolean {
  switch (name) {
    case "kimi":
    case "moonshot":
    case "glm":
    case "bigmodel":
    case "zhipu":
      return true;
    default:
      return false;
  }
}

/** Returns the model used when a compatible provider has no explicit model. */
export function defaultModelForProvider(name: string): string {
  switch (name) {
    case "openai":
    case "gpt":
      return DEFAULT_OPENAI_MODEL;
    case "anthropic":
    case "claude":
    case "opus":
      return DEFAULT_ANTHROPIC_MODEL;
    case "kimi":
    case "moonshot":
      return DEFAULT_KIMI_MODEL;
    case "glm":
    case "bigmodel":
    case "zhipu":
      return DEFAULT_GLM_MODEL;
    default:
      return "";
  }
}

/** Returns a compatibility endpoint for provider aliases. */
export function defaultBaseURLForProvider(name: string): string {
  switch (name) {
    case "kimi":
    case "moonshot":
      return DEFAULT_MOONSHOT_BASE_URL;
    case "glm":
    case "bigmodel":
    case "zhipu":
      return DEFAULT_BIGMODEL_BASE_URL;
    default:
      return "";
  }
}

/** Fills compatibility defaults for common model families. */
export function normalizeProviderConfig(name: string, config: ProviderConfig): ProviderConfig {
  const normalized = name.toLowerCase().trim();
  if (config.defaultModel === "") {
    config.defaultModel = defaultModelForProvider(normalized);
  }
  if (config.baseURL === "") {
    config.baseURL = defaultBaseURLForProvider(normalized);
  }
  if (usesThinkingReasoningFormat(normalized)) {
    if (!config.extra) {
      config.extra = {};
    }
    if (!config.extra["reasoning_format"]) {
      config.extra["reasoning_format"] = "thinking";
    }
  }
  return config;
}

// ---------------------------------------------------------------------------
// ProviderFactory
// ---------------------------------------------------------------------------

/**
 * Creates AiProvider instances by name.
 */
export class ProviderFactory {
  /**
   * Creates an AiProvider for the given name using the provided config.
   *
   * Supported names (case-insensitive):
   *   - "openai", "gpt", "zai", "z.ai", "zen", "opencode-zen", "kimi", "moonshot", "glm", "bigmodel", "zhipu" -> OpenAI-compatible
   *   - "anthropic", "claude", "opus" -> Anthropic-compatible
   *   - "deepseek" -> DeepSeek
   *   - "qianwen", "tongyi", "dashscope" -> Qianwen
   *   - "wenxin", "ernie", "baidu" -> Wenxin
   */
  createProvider(name: string, config: ProviderConfig): AiProvider {
    const normalized = name.toLowerCase().trim();
    const normalizedConfig = normalizeProviderConfig(normalized, { ...config });

    switch (normalized) {
      case "openai":
      case "gpt":
      case "zai":
      case "z.ai":
      case "zen":
      case "opencode-zen":
      case "kimi":
      case "moonshot":
      case "glm":
      case "bigmodel":
      case "zhipu":
        return newOpenAIProvider(normalizedConfig);
      case "anthropic":
      case "claude":
      case "opus":
        return newAnthropicProvider(normalizedConfig);
      case "deepseek":
        return newDeepSeekProvider(normalizedConfig);
      case "qianwen":
      case "tongyi":
      case "dashscope":
        return newQianwenProvider(normalizedConfig);
      case "wenxin":
      case "ernie":
      case "baidu":
        return newWenxinProvider(normalizedConfig);
      default:
        throw newAiUnsupportedProviderError(
          `unsupported provider: "${name}"`,
        );
    }
  }
}

// Import convenience constructors — these are used in the factory method above
// which is defined in the same file to avoid circular deps.
// The actual implementations are imported from their respective modules.
import { newOpenAIProvider } from "./openai.js";
import { newAnthropicProvider } from "./anthropic.js";
import { newDeepSeekProvider } from "./deepseek.js";
import { newQianwenProvider } from "./qianwen.js";
import { newWenxinProvider } from "./wenxin.js";
import { newAiUnsupportedProviderError } from "./error.js";
