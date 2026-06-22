/**
 * @module error
 *
 * Error types and constructors for the OrangeCoding core module.
 *
 * Provides a structured error hierarchy:
 * - IOError: file system and I/O errors
 * - ConfigError: configuration validation errors
 * - AgentError: agent lifecycle errors
 * - NetworkError: HTTP and WebSocket errors
 *
 * All errors include a code string for programmatic matching
 * and optional detail fields for debugging.
 */
// ---------------------------------------------------------------------------
// ErrorKind
// ---------------------------------------------------------------------------
// Categorical tags for OrangeError. The kind drives retry decisions
 // (isRetryable) and routing in CLI output. String-enum form keeps the values
 // stable across serialization.

export const ErrorKind = {
  Config: "config",
  IO: "io",
  Network: "network",
  Provider: "provider",
  Agent: "agent",
  Tool: "tool",
  Protocol: "protocol",
  Serialization: "serialization",
  Auth: "auth",
  Internal: "internal",
} as const;

export type ErrorKind = (typeof ErrorKind)[keyof typeof ErrorKind];

// ---------------------------------------------------------------------------
// OrangeError
// ---------------------------------------------------------------------------

/**
 * Base error type for the whole monorepo. Carries a categorical {@link
 * ErrorKind} and an optional wrapped cause. `isRetryable` returns true only
 * for transient categories (Network, Provider) so retry loops can gate on it.
 */

export class OrangeError extends Error {
  public readonly kind: ErrorKind;
  private readonly _cause?: Error;

  constructor(kind: ErrorKind, message: string, cause?: Error) {
    const msg = cause ? `${kind}: ${message}: ${cause.message}` : `${kind}: ${message}`;
    super(msg);
    this.name = "OrangeError";
    this.kind = kind;
    this._cause = cause;
  }

  override get cause(): Error | undefined {
    return this._cause;
  }

  isRetryable(): boolean {
    return this.kind === ErrorKind.Network || this.kind === ErrorKind.Provider;
  }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------
// One factory per ErrorKind so call sites read as intent and the kind tag is
// never misspelled. Each is a thin wrapper around the OrangeError constructor.

/** Wrap an existing Error as an OrangeError of `kind`, preserving it as `.cause`. */
export function wrapError(cause: Error, kind: ErrorKind, message: string): OrangeError {
  return new OrangeError(kind, message, cause);
}

export function newConfigError(msg: string): OrangeError {
  return new OrangeError(ErrorKind.Config, msg);
}

export function newIOError(msg: string): OrangeError {
  return new OrangeError(ErrorKind.IO, msg);
}

export function newNetworkError(msg: string): OrangeError {
  return new OrangeError(ErrorKind.Network, msg);
}

export function newProviderError(msg: string): OrangeError {
  return new OrangeError(ErrorKind.Provider, msg);
}

export function newProtocolError(msg: string): OrangeError {
  return new OrangeError(ErrorKind.Protocol, msg);
}

export function newSerializationError(msg: string): OrangeError {
  return new OrangeError(ErrorKind.Serialization, msg);
}

export function newAuthError(msg: string): OrangeError {
  return new OrangeError(ErrorKind.Auth, msg);
}

export function newInternalError(msg: string): OrangeError {
  return new OrangeError(ErrorKind.Internal, msg);
}

export function newToolError(toolName: string, msg: string): OrangeError {
  return new OrangeError(ErrorKind.Tool, `[${toolName}] ${msg}`);
}

export function newAgentError(agentId: string, msg: string): OrangeError {
  return new OrangeError(ErrorKind.Agent, `[${agentId}] ${msg}`);
}
