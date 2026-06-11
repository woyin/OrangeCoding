// Types
export type {
  ChatMessage,
  ToolCall,
  FunctionCall,
  ToolDefinition,
  FunctionDefinition,
  ToolParameter,
  ChatOptions,
  AiResponse,
  AiTokenUsage,
  StreamEvent,
} from "./types.js";

export {
  systemMsg,
  userMsg,
  assistantMsg,
  toolResultMsg,
  assistantMsgWithTools,
  toAiToolCall,
  toCoreToolCall,
  toAiToolCalls,
  toCoreToolCalls,
} from "./types.js";

// Error
export {
  AiErrorKind,
  AiError,
  newAiNetworkError,
  newAiApiError,
  newAiAuthError,
  newAiParseError,
  newAiStreamError,
  newAiConfigError,
  newAiUnsupportedProviderError,
  newAiRateLimitError,
  newAiTimeoutError,
} from "./error.js";

// Stream
export { parseSSEStream } from "./stream.js";

// Provider
export type { AiProvider, ProviderConfig } from "./provider.js";

export {
  ProviderFactory,
  providerTimeout,
  defaultModelForProvider,
  defaultBaseURLForProvider,
  normalizeProviderConfig,
} from "./provider.js";

// OpenAI provider
export { OpenAIProvider, newOpenAIProvider } from "./openai.js";

// Anthropic provider
export { AnthropicProvider, newAnthropicProvider } from "./anthropic.js";

// DeepSeek provider
export { DeepSeekProvider, newDeepSeekProvider } from "./deepseek.js";

// Qianwen provider
export { QianwenProvider, newQianwenProvider } from "./qianwen.js";

// Wenxin provider
export { WenxinProvider, newWenxinProvider } from "./wenxin.js";

// Router
export { ModelCategory, ModelRouter, createOmORouter } from "./router.js";
export type { RoutingRule } from "./router.js";

// Fallback
export { FallbackChain } from "./fallback.js";

// Rate limit handling
export { RateLimitHandler, withRateLimitRetry } from "./rate-limit.js";
export type { RateLimitPolicy, RetryDecision } from "./rate-limit.js";
