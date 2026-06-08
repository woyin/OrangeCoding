// ---------------------------------------------------------------------------
// AiErrorKind enum
// ---------------------------------------------------------------------------

/**
 * Classifies the category of an AI-related error.
 */
export enum AiErrorKind {
  Network = "network",
  Api = "api",
  Auth = "auth",
  Parse = "parse",
  Stream = "stream",
  Config = "config",
  UnsupportedProvider = "unsupported-provider",
  RateLimit = "rate-limit",
  Timeout = "timeout",
}

// ---------------------------------------------------------------------------
// AiError
// ---------------------------------------------------------------------------

/**
 * Error type for AI provider operations.
 */
export class AiError extends Error {
  public readonly kind: AiErrorKind;
  public readonly statusCode: number;
  public readonly retryAfter: number;

  constructor(kind: AiErrorKind, message: string, statusCode = 0, retryAfter = 0) {
    super(`ai: ${kind}: ${message}`);
    this.name = "AiError";
    this.kind = kind;
    this.statusCode = statusCode;
    this.retryAfter = retryAfter;
  }

  /** Returns true for error kinds that may succeed on retry. */
  isRetryable(): boolean {
    return (
      this.kind === AiErrorKind.Network ||
      this.kind === AiErrorKind.RateLimit ||
      this.kind === AiErrorKind.Timeout
    );
  }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

/** Creates a network error. */
export function newAiNetworkError(msg: string): AiError {
  return new AiError(AiErrorKind.Network, msg);
}

/** Creates an API error with a status code. */
export function newAiApiError(msg: string, statusCode: number): AiError {
  return new AiError(AiErrorKind.Api, msg, statusCode);
}

/** Creates an authentication error. */
export function newAiAuthError(msg: string): AiError {
  return new AiError(AiErrorKind.Auth, msg);
}

/** Creates a response parsing error. */
export function newAiParseError(msg: string): AiError {
  return new AiError(AiErrorKind.Parse, msg);
}

/** Creates a streaming error. */
export function newAiStreamError(msg: string): AiError {
  return new AiError(AiErrorKind.Stream, msg);
}

/** Creates a configuration error. */
export function newAiConfigError(msg: string): AiError {
  return new AiError(AiErrorKind.Config, msg);
}

/** Creates an unsupported provider error. */
export function newAiUnsupportedProviderError(msg: string): AiError {
  return new AiError(AiErrorKind.UnsupportedProvider, msg);
}

/** Creates a rate limit error with retry-after duration. */
export function newAiRateLimitError(msg: string, retryAfter: number): AiError {
  return new AiError(AiErrorKind.RateLimit, msg, 0, retryAfter);
}

/** Creates a timeout error. */
export function newAiTimeoutError(msg: string): AiError {
  return new AiError(AiErrorKind.Timeout, msg);
}
