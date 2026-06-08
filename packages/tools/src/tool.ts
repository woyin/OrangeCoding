/**
 * Core Tool interface and supporting types.
 *
 * Ported from modules/tools/tool.go.
 */

// ---------------------------------------------------------------------------
// ToolMetadata
// ---------------------------------------------------------------------------

export interface ToolMetadata {
  readonly isReadOnly: boolean;
  readonly isConcurrencySafe: boolean;
  readonly isDestructive: boolean;
  readonly isEnabled: boolean;
}

/** Returns metadata with only isEnabled set to true. */
export function defaultMetadata(): ToolMetadata {
  return { isReadOnly: false, isConcurrencySafe: false, isDestructive: false, isEnabled: true };
}

/** Returns metadata for read-only, concurrency-safe tools. */
export function readOnlyMetadata(): ToolMetadata {
  return { isReadOnly: true, isConcurrencySafe: true, isDestructive: false, isEnabled: true };
}

/** Returns metadata for tools that modify the filesystem or state. */
export function destructiveMetadata(): ToolMetadata {
  return { isReadOnly: false, isConcurrencySafe: false, isDestructive: true, isEnabled: true };
}

// ---------------------------------------------------------------------------
// ToolError
// ---------------------------------------------------------------------------

/**
 * Structured error kind names, mirroring Go's ToolError.Kind values.
 */
export type ToolErrorKind =
  | "invalid_params"
  | "execution_error"
  | "security_violation"
  | "not_found";

/**
 * A structured error returned by tool execution.
 * Mirrors Go's `tools.ToolError`.
 */
export class ToolError extends Error {
  public readonly kind: ToolErrorKind;

  constructor(kind: ToolErrorKind, message: string) {
    super(`${kind}: ${message}`);
    this.name = "ToolError";
    this.kind = kind;
  }
}

// ---------------------------------------------------------------------------
// Tool interface
// ---------------------------------------------------------------------------

/**
 * The interface that every tool must implement.
 * Mirrors Go's `tools.Tool`.
 */
export interface Tool {
  /** Unique tool identifier (e.g. "bash", "read_file"). */
  name(): string;

  /** Human-readable description of what the tool does. */
  description(): string;

  /** JSON Schema describing the tool's input parameters. */
  parameters(): Record<string, unknown>;

  /** Execute the tool with the given JSON-compatible input and return a string result. */
  execute(ctx: unknown, input: unknown): Promise<string>;

  /** Metadata about the tool's behaviour. */
  metadata(): ToolMetadata;
}
