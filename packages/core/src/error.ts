// ---------------------------------------------------------------------------
// ErrorKind
// ---------------------------------------------------------------------------

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
