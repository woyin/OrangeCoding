import { v4 as uuidv4, validate as uuidValidate, parse as uuidParse } from "uuid";

// ---------------------------------------------------------------------------
// AgentId
// ---------------------------------------------------------------------------

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

export interface TokenUsageJSON {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

export class TokenUsage {
  constructor(
    public readonly promptTokens: number,
    public readonly completionTokens: number,
    public readonly totalTokens: number,
  ) {}

  static create(prompt: number, completion: number): TokenUsage {
    return new TokenUsage(prompt, completion, prompt + completion);
  }

  accumulate(other: TokenUsage): void {
    (this as { promptTokens: number }).promptTokens += other.promptTokens;
    (this as { completionTokens: number }).completionTokens += other.completionTokens;
    (this as { totalTokens: number }).totalTokens += other.totalTokens;
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

export const AgentRole = {
  Coder: "coder",
  Reviewer: "reviewer",
  Planner: "planner",
  Executor: "executor",
  Observer: "observer",
} as const;

export type AgentRole = (typeof AgentRole)[keyof typeof AgentRole];

export function parseAgentRole(s: string): AgentRole {
  switch (s) {
    case "coder": return AgentRole.Coder;
    case "reviewer": return AgentRole.Reviewer;
    case "planner": return AgentRole.Planner;
    case "executor": return AgentRole.Executor;
    case "observer": return AgentRole.Observer;
    default: throw new Error(`unknown agent role: "${s}"`);
  }
}

// ---------------------------------------------------------------------------
// AgentStatus enum
// ---------------------------------------------------------------------------

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

export interface AgentCapability {
  name: string;
  description: string;
  supportedTools: ToolName[];
}

export function supportsTool(cap: AgentCapability, name: ToolName): boolean {
  return cap.supportedTools.some((t) => t.equals(name));
}
