/**
 * GuardrailPipeline runs guardrails in order and stops on deny.
 * Ported from modules/agent/harness_guardrail.go.
 */

import type { ToolCall } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// GuardrailPhase
// ---------------------------------------------------------------------------

export type GuardrailPhase = "pre_model" | "pre_tool" | "post_tool" | "final_output";

// ---------------------------------------------------------------------------
// GuardrailDecision
// ---------------------------------------------------------------------------

export type GuardrailDecision = "allow" | "deny" | "warn";

// ---------------------------------------------------------------------------
// GuardrailContext
// ---------------------------------------------------------------------------

export interface GuardrailContext {
  phase: GuardrailPhase;
  toolCall?: ToolCall;
  output: string;
  recentToolKeys: string[];
  tokenEstimate: number;
  maxTokens: number;
}

// ---------------------------------------------------------------------------
// GuardrailResult
// ---------------------------------------------------------------------------

export interface GuardrailResult {
  decision: GuardrailDecision;
  reason: string;
  name: string;
}

export interface DefaultGuardrailPipelineConfig {
  repeatedToolLimit?: number;
  maxTokens?: number;
  maxOutputLength?: number;
  llmGuardrails?: LLMGuardrailConfig[];
}

/** Helper to create an allow result. */
function allowResult(name: string): GuardrailResult {
  return { decision: "allow", reason: "", name };
}

// ---------------------------------------------------------------------------
// Guardrail interface
// ---------------------------------------------------------------------------

export interface Guardrail {
  name(): string;
  check(signal: AbortSignal | undefined, input: GuardrailContext): GuardrailResult | Promise<GuardrailResult>;
}

// ---------------------------------------------------------------------------
// GuardrailPipeline
// ---------------------------------------------------------------------------

export class GuardrailPipeline {
  private _guardrails: Guardrail[];

  constructor(guardrails: Guardrail[]) {
    this._guardrails = guardrails;
  }

  /** Check runs the guardrail pipeline. Supports async guardrails. */
  async check(signal: AbortSignal | undefined, input: GuardrailContext): Promise<GuardrailResult> {
    if (signal?.aborted) {
      return { decision: "deny", reason: "aborted", name: "context" };
    }
    for (const guardrail of this._guardrails) {
      const result = await guardrail.check(signal, input);
      if (!result.name) result.name = guardrail.name();
      if (result.decision === "deny" || result.decision === "warn") {
        return result;
      }
    }
    return { decision: "allow", reason: "", name: "pipeline" };
  }
}

export function defaultGuardrailPipeline(config: DefaultGuardrailPipelineConfig = {}): GuardrailPipeline {
  const guardrails: Guardrail[] = [
    new TokenBudgetGuardrail(config.maxTokens),
    new OutputLengthGuardrail(config.maxOutputLength),
    new DangerousToolGuardrail(),
    new RepeatedToolGuardrail(config.repeatedToolLimit ?? 3),
  ];

  for (const llmConfig of config.llmGuardrails ?? []) {
    guardrails.push(new LLMGuardrail(llmConfig));
  }

  return new GuardrailPipeline(guardrails);
}

// ---------------------------------------------------------------------------
// GuardrailLogEntry
// ---------------------------------------------------------------------------

export interface GuardrailLogEntry {
  name: string;
  decision: GuardrailDecision;
  reason: string;
  phase: GuardrailPhase;
  timestamp: Date;
}

// ---------------------------------------------------------------------------
// GuardrailLogger
// ---------------------------------------------------------------------------

const MAX_GUARDRAIL_LOG_ENTRIES = 1000;

export class GuardrailLogger {
  private _entries: GuardrailLogEntry[];

  constructor() {
    this._entries = [];
  }

  /** Log appends a guardrail log entry. Evicts oldest if over limit. */
  log(entry: GuardrailLogEntry): void {
    this._entries.push(entry);
    if (this._entries.length > MAX_GUARDRAIL_LOG_ENTRIES) {
      this._entries.splice(0, this._entries.length - MAX_GUARDRAIL_LOG_ENTRIES);
    }
  }

  /** Recent returns the last n log entries (or all if n exceeds length). */
  recent(n: number): GuardrailLogEntry[] {
    if (n >= this._entries.length) {
      return [...this._entries];
    }
    return this._entries.slice(this._entries.length - n);
  }

  /** Warnings returns only entries with warn decision. */
  warnings(): GuardrailLogEntry[] {
    return this._entries.filter((e) => e.decision === "warn");
  }

  /** Len returns the total number of logged entries. */
  get length(): number {
    return this._entries.length;
  }
}

// ---------------------------------------------------------------------------
// TokenBudgetGuardrail
// ---------------------------------------------------------------------------

export class TokenBudgetGuardrail implements Guardrail {
  private _maxTokens: number;

  constructor(maxTokens?: number) {
    this._maxTokens = maxTokens ?? 0;
  }

  name(): string { return "token_budget"; }

  check(_signal: AbortSignal | undefined, input: GuardrailContext): GuardrailResult {
    if (input.phase !== "pre_model" && input.phase !== "final_output") {
      return allowResult(this.name());
    }
    const maxTokens = input.maxTokens > 0 ? input.maxTokens : this._maxTokens;
    if (maxTokens > 0 && input.tokenEstimate > maxTokens) {
      return {
        decision: "warn",
        reason: "approaching token budget",
        name: this.name(),
      };
    }
    return allowResult(this.name());
  }
}

// ---------------------------------------------------------------------------
// OutputLengthGuardrail
// ---------------------------------------------------------------------------

export class OutputLengthGuardrail implements Guardrail {
  private _maxLength: number;

  constructor(maxLength?: number) {
    this._maxLength = maxLength ?? 50000;
  }

  name(): string { return "output_length"; }

  check(_signal: AbortSignal | undefined, input: GuardrailContext): GuardrailResult {
    if (input.phase !== "final_output") {
      return allowResult(this.name());
    }
    if (input.output.length > this._maxLength) {
      return {
        decision: "warn",
        reason: "output exceeds recommended length",
        name: this.name(),
      };
    }
    return allowResult(this.name());
  }
}

// ---------------------------------------------------------------------------
// LLMGuardrailConfig
// ---------------------------------------------------------------------------

export interface LLMGuardrailConfig {
  phase: GuardrailPhase;
  prompt: string;
  provider: ((signal: AbortSignal | undefined, prompt: string, content: string) => Promise<[boolean, Error | null]>) | null;
}

// ---------------------------------------------------------------------------
// LLMGuardrail
// ---------------------------------------------------------------------------

export class LLMGuardrail implements Guardrail {
  private _config: LLMGuardrailConfig;

  constructor(config: LLMGuardrailConfig) {
    this._config = config;
  }

  name(): string { return "llm_guardrail"; }

  async check(signal: AbortSignal | undefined, input: GuardrailContext): Promise<GuardrailResult> {
    if (input.phase !== this._config.phase) {
      return allowResult(this.name());
    }
    if (!this._config.provider) {
      return allowResult(this.name());
    }
    let content = input.output;
    if (!content && input.toolCall) {
      content = JSON.stringify(input.toolCall.arguments);
    }
    if (!content) {
      return allowResult(this.name());
    }
    const [safe, err] = await this._config.provider(signal, this._config.prompt, content);
    if (err) {
      return {
        decision: "deny",
        reason: "llm guardrail evaluation failed: " + err.message,
        name: this.name(),
      };
    }
    if (!safe) {
      return {
        decision: "deny",
        reason: "llm guardrail rejected content",
        name: this.name(),
      };
    }
    return allowResult(this.name());
  }
}

// ---------------------------------------------------------------------------
// DangerousToolGuardrail
// ---------------------------------------------------------------------------

// Comprehensive list of dangerous command patterns
const BLOCKED_COMMANDS = [
  // Direct deletion
  "rm -rf /", "rm -rf /*", "rm -rf ~", "rm -rf .",
  // Filesystem destruction
  "mkfs", "dd if=", "> /dev/sda", "> /dev/nvme",
  // Fork bomb
  ":(){:|:&};:",
  // Remote execution
  "curl | sh", "curl | bash", "wget | sh", "wget | bash",
  "curl | python", "wget | python",
  // Permission escalation
  "chmod -R 777 /", "chmod 777 /", "chown -R",
  // System control
  "shutdown", "reboot", "halt", "poweroff",
  // Process killing
  "kill -9 -1", "killall -9",
];

export class DangerousToolGuardrail implements Guardrail {
  name(): string { return "dangerous_tool"; }

  check(_signal: AbortSignal | undefined, input: GuardrailContext): GuardrailResult {
    if (input.phase !== "pre_tool" || !input.toolCall) {
      return allowResult(this.name());
    }
    if (input.toolCall.function_name !== "bash") {
      return allowResult(this.name());
    }
    const command = extractCommandArgument(input.toolCall.arguments);
    const lower = command.toLowerCase();
    for (const pattern of BLOCKED_COMMANDS) {
      if (lower.includes(pattern)) {
        return { decision: "deny", reason: "dangerous shell command", name: this.name() };
      }
    }
    return allowResult(this.name());
  }
}

// ---------------------------------------------------------------------------
// RepeatedToolGuardrail
// ---------------------------------------------------------------------------

export class RepeatedToolGuardrail implements Guardrail {
  private _limit: number;

  constructor(limit?: number) {
    this._limit = limit && limit > 0 ? limit : 3;
  }

  name(): string { return "repeated_tool"; }

  check(_signal: AbortSignal | undefined, input: GuardrailContext): GuardrailResult {
    if (input.phase !== "pre_tool" || !input.toolCall) {
      return allowResult(this.name());
    }
    const key = toolCallKey(input.toolCall);
    let count = 0;
    for (const recent of input.recentToolKeys) {
      if (recent === key) count++;
    }
    if (count >= this._limit) {
      return { decision: "deny", reason: "repeated identical tool call", name: this.name() };
    }
    return allowResult(this.name());
  }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/** ToolCallKey returns a stable key for loop detection. */
export function toolCallKey(call: ToolCall): string {
  return call.function_name + ":" + JSON.stringify(call.arguments);
}

function extractCommandArgument(raw: unknown): string {
  if (typeof raw === "object" && raw !== null) {
    const obj = raw as Record<string, unknown>;
    if (typeof obj.command === "string") return obj.command;
  }
  return typeof raw === "string" ? raw : JSON.stringify(raw);
}
