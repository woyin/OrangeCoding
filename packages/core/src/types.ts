/**
 * @module types
 *
 * Core type definitions and branded types for the OrangeCoding system.
 *
 * Provides:
 * - Branded types: AgentId, SessionId (prevent accidental mixing)
 * - Role enum: System, User, Assistant, Tool
 * - AgentStatus enum: lifecycle states
 * - TokenUsage: tracks AI API token consumption
 * - TaskType: task classification categories
 * - TaskStatus: task lifecycle states
 */
import { v4 as uuidv4, validate as uuidValidate, parse as uuidParse } from "uuid";

// ---------------------------------------------------------------------------
// AgentId
// ---------------------------------------------------------------------------

/**
 * Opaque identifier for an agent instance. Wraps a UUIDv4 string and
 * serializes with an `agent-` prefix so it is unambiguous in logs and
 * persisted state. The constructor is private; use {@link AgentId.create}
 * (random) or {@link AgentId.parse} (from string).
 */

export class AgentId {
  private readonly _id: string;

  private constructor(id: string) {
    this._id = id;
  }

  static create(): AgentId {
    return new AgentId(uuidv4());
  }

  static parse(s: string): AgentId {
    if (!s.startsWith("agent-")) {
      throw new Error(`invalid agent ID format: missing 'agent-' prefix in "${s}"`);
    }
    const uuidStr = s.slice(6);
    if (uuidStr.length === 0) {
      throw new Error(`invalid agent ID format: empty UUID in "${s}"`);
    }
    if (!uuidValidate(uuidStr)) {
      throw new Error(`invalid agent ID format: invalid UUID "${uuidStr}"`);
    }
    return new AgentId(uuidStr);
  }

  toString(): string {
    return `agent-${this._id}`;
  }

  toJSON(): string {
    return this.toString();
  }

  equals(other: AgentId): boolean {
    return this._id === other._id;
  }
}

// ---------------------------------------------------------------------------
// SessionId
// ---------------------------------------------------------------------------

/**
 * Opaque identifier for a conversation session. Same pattern as AgentId but
 * uses the `session-` prefix. Equality is by the inner UUID, not the
 * formatted string, so prefixes cannot collide.
 */

export class SessionId {
  private readonly _id: string;

  private constructor(id: string) {
    this._id = id;
  }

  static create(): SessionId {
    return new SessionId(uuidv4());
  }

  static parse(s: string): SessionId {
    if (!s.startsWith("session-")) {
      throw new Error(`invalid session ID format: missing 'session-' prefix in "${s}"`);
    }
    const uuidStr = s.slice(8);
    if (uuidStr.length === 0) {
      throw new Error(`invalid session ID format: empty UUID in "${s}"`);
    }
    if (!uuidValidate(uuidStr)) {
      throw new Error(`invalid session ID format: invalid UUID "${uuidStr}"`);
    }
    return new SessionId(uuidStr);
  }

  toString(): string {
    return `session-${this._id}`;
  }

  toJSON(): string {
    return this.toString();
  }

  equals(other: SessionId): boolean {
    return this._id === other._id;
  }
}

// ---------------------------------------------------------------------------
// ToolName
// ---------------------------------------------------------------------------

/**
 * Lightweight newtype around a tool name string. Provides structural typing
 * (a plain string is not assignable to ToolName) so tool registries and
 * tool-call payloads stay type-safe without runtime overhead.
 */

export class ToolName {
  private readonly _name: string;

  private constructor(name: string) {
    this._name = name;
  }

  static create(name: string): ToolName {
    return new ToolName(name);
  }

  toString(): string {
    return this._name;
  }

  toJSON(): string {
    return this._name;
  }

  equals(other: ToolName): boolean {
    return this._name === other._name;
  }
}

// ---------------------------------------------------------------------------
// TokenUsage
// ---------------------------------------------------------------------------
// Tracks cumulative token consumption for a conversation. The `create`
// factory computes `totalTokens` from prompt+completion so callers cannot
// accidentally desynchronize the three fields.

export interface TokenUsageJSON {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

export class TokenUsage {
  constructor(
    public promptTokens: number,
    public completionTokens: number,
    public totalTokens: number,
  ) {}

  static create(prompt: number, completion: number): TokenUsage {
    return new TokenUsage(prompt, completion, prompt + completion);
  }

  accumulate(other: TokenUsage): void {
    this.promptTokens += other.promptTokens;
    this.completionTokens += other.completionTokens;
    this.totalTokens += other.totalTokens;
  }

  isEmpty(): boolean {
    return this.totalTokens === 0;
  }

  toJSON(): TokenUsageJSON {
    return {
      prompt_tokens: this.promptTokens,
      completion_tokens: this.completionTokens,
      total_tokens: this.totalTokens,
    };
  }
}

// ---------------------------------------------------------------------------
// AgentRole enum
// ---------------------------------------------------------------------------
// String-enum (not a TS `enum`) so the values survive JSON serialization
// unchanged and are tree-shakeable. `parseAgentRole` is the trusted boundary
// that maps arbitrary input strings to the known set, throwing on unknowns.

export const AgentRole = {
  Coder: "coder",
  Reviewer: "reviewer",
  Planner: "planner",
  Executor: "executor",
  Observer: "observer",
  Explorer: "explorer",
  Builder: "builder",
  Analyst: "analyst",
} as const;

export type AgentRole = (typeof AgentRole)[keyof typeof AgentRole];

export function parseAgentRole(s: string): AgentRole {
  switch (s) {
    case "coder": return AgentRole.Coder;
    case "reviewer": return AgentRole.Reviewer;
    case "planner": return AgentRole.Planner;
    case "executor": return AgentRole.Executor;
    case "observer": return AgentRole.Observer;
    case "explorer": return AgentRole.Explorer;
    case "builder": return AgentRole.Builder;
    case "analyst": return AgentRole.Analyst;
    default: throw new Error(`unknown agent role: "${s}"`);
  }
}

// ---------------------------------------------------------------------------
// AgentStatus enum
// ---------------------------------------------------------------------------
// Lifecycle states for an agent. `isTerminalStatus` and `isActiveStatus`
// are the predicates callers should use instead of comparing to literals,
// so future status additions only need to update one place.

export const AgentStatus = {
  Idle: "idle",
  Running: "running",
  Waiting: "waiting",
  Completed: "completed",
  Failed: "failed",
} as const;

export type AgentStatus = (typeof AgentStatus)[keyof typeof AgentStatus];

export function parseAgentStatus(s: string): AgentStatus {
  switch (s) {
    case "idle": return AgentStatus.Idle;
    case "running": return AgentStatus.Running;
    case "waiting": return AgentStatus.Waiting;
    case "completed": return AgentStatus.Completed;
    case "failed": return AgentStatus.Failed;
    default: throw new Error(`unknown agent status: "${s}"`);
  }
}

export function isTerminalStatus(s: AgentStatus): boolean {
  return s === AgentStatus.Completed || s === AgentStatus.Failed;
}

export function isActiveStatus(s: AgentStatus): boolean {
  return s === AgentStatus.Running || s === AgentStatus.Waiting;
}

// ---------------------------------------------------------------------------
// Role enum (message role)
// ---------------------------------------------------------------------------
// Mirrors the OpenAI/Anthropic chat-message roles. `parseRole` is the
// single point that validates strings coming off disk or the wire.

export const Role = {
  System: "system",
  User: "user",
  Assistant: "assistant",
  Tool: "tool",
} as const;

export type Role = (typeof Role)[keyof typeof Role];

export function parseRole(s: string): Role {
  switch (s) {
    case "system": return Role.System;
    case "user": return Role.User;
    case "assistant": return Role.Assistant;
    case "tool": return Role.Tool;
    default: throw new Error(`unknown role: "${s}"`);
  }
}

// ---------------------------------------------------------------------------
// AgentCapability
// ---------------------------------------------------------------------------

/**
 * Declares what an agent can do: a human-readable name/description plus the
 * subset of tools it supports. {@link supportsTool} is the O(n) membership
 * check used by dispatchers; n is small so a Set is not warranted.
 */

export interface AgentCapability {
  name: string;
  description: string;
  supportedTools: ToolName[];
}

export function supportsTool(cap: AgentCapability, name: ToolName): boolean {
  return cap.supportedTools.some((t) => t.equals(name));
}
