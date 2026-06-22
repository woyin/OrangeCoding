/**
 * @module harness-handoff
 *
 * Agent handoff protocol for transitioning between agent instances.
 *
 * When an agent session needs to be transferred to a different agent
 * (e.g., model upgrade, specialized agent, or session resumption),
 * the handoff module manages:
 * - State serialization and transfer
 * - Context preservation
 * - Conversation history migration
 * - Graceful transition with minimal disruption
 */

import type { AgentId, Message } from "@orangecoding/core";
import type { ChatOptions } from "@orangecoding/ai";
import type { ReasoningEffort } from "./harness-profile.js";

// ---------------------------------------------------------------------------
// HandoffRequest
// ---------------------------------------------------------------------------

export interface HandoffRequest {
  fromAgentID: AgentId;
  toAgentID: AgentId;
  task: string;
  conversation: Message[];
  toolCallsMade: number;
  memoryKeys: string[];
  metadata: Record<string, string>;
}

// ---------------------------------------------------------------------------
// HandoffResult
// ---------------------------------------------------------------------------

export interface HandoffResult {
  fromAgentID: AgentId;
  toAgentID: AgentId;
  completed: boolean;
  toolCallsMade: number;
  summary: string;
  error: string;
}

// ---------------------------------------------------------------------------
// HandoffHandler
// ---------------------------------------------------------------------------

export interface HandoffHandler {
  canHandoff(signal: AbortSignal | undefined, req: HandoffRequest): Promise<[boolean, Error | null]>;
  executeHandoff(signal: AbortSignal | undefined, req: HandoffRequest): Promise<[HandoffResult, Error | null]>;
}

// ---------------------------------------------------------------------------
// ToolUseBudget
// ---------------------------------------------------------------------------

export class ToolUseBudget {
  maxUses: Map<string, number>;
  private _counts: Map<string, number>;

  constructor() {
    this.maxUses = new Map();
    this._counts = new Map();
  }

  /** SetMaxUses sets the maximum allowed calls for a tool. */
  setMaxUses(toolName: string, max: number): void {
    this.maxUses.set(toolName, max);
  }

  /** RecordCall records that a tool was called. Returns true if the call is allowed. */
  recordCall(toolName: string): boolean {
    const max = this.maxUses.get(toolName);
    if (max === undefined || max === 0) {
      this._counts.set(toolName, (this._counts.get(toolName) ?? 0) + 1);
      return true;
    }
    const current = this._counts.get(toolName) ?? 0;
    if (current >= max) return false;
    this._counts.set(toolName, current + 1);
    return true;
  }

  /** Remaining returns how many more calls are allowed for a tool. */
  remaining(toolName: string): number {
    const max = this.maxUses.get(toolName);
    if (max === undefined || max === 0) return Number.MAX_SAFE_INTEGER;
    const used = this._counts.get(toolName) ?? 0;
    if (used >= max) return 0;
    return max - used;
  }

  /** Used returns how many times a tool has been called. */
  used(toolName: string): number {
    return this._counts.get(toolName) ?? 0;
  }
}

// ---------------------------------------------------------------------------
// AgentModelSettings
// ---------------------------------------------------------------------------

export interface AgentModelSettings {
  model?: string;
  temperature?: number;
  topP?: number;
  maxTokens?: number;
  reasoningEffort?: ReasoningEffort;
  reasoningBudget?: number;
}

/** ApplyToChatOptions applies agent-specific model settings to ChatOptions. */
export function applyModelSettingsToChatOptions(settings: AgentModelSettings, opts: ChatOptions): ChatOptions {
  if (settings.model) opts.model = settings.model;
  if (settings.temperature !== undefined) opts.temperature = settings.temperature;
  if (settings.topP !== undefined) opts.top_p = settings.topP;
  if (settings.maxTokens !== undefined) opts.max_tokens = settings.maxTokens;
  if (settings.reasoningEffort) opts.reasoning_effort = settings.reasoningEffort;
  if (settings.reasoningBudget !== undefined) opts.reasoning_budget_tokens = settings.reasoningBudget;
  return opts;
}

// ---------------------------------------------------------------------------
// OrchestratorTask
// ---------------------------------------------------------------------------

export interface OrchestratorTask {
  id: string;
  agentID: AgentId;
  description: string;
  scope: string[]; // file paths or directories
  dependsOn: string[]; // task IDs this depends on
  priority: number;
}

// ---------------------------------------------------------------------------
// OrchestratorResult
// ---------------------------------------------------------------------------

export interface OrchestratorResult {
  taskID: string;
  success: boolean;
  summary: string;
  error: string;
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

export class Orchestrator {
  private _tasks: Map<string, OrchestratorTask>;
  private _results: Map<string, OrchestratorResult>;

  constructor() {
    this._tasks = new Map();
    this._results = new Map();
  }

  /** AddTask registers a task for orchestration. */
  addTask(task: OrchestratorTask): void {
    if (!task.id) throw new Error("orchestrator: task ID is required");
    this._tasks.set(task.id, task);
  }

  /** RecordResult records the outcome of a task. */
  recordResult(result: OrchestratorResult): void {
    this._results.set(result.taskID, result);
  }

  /** ReadyTasks returns tasks whose dependencies are all completed successfully. */
  readyTasks(): OrchestratorTask[] {
    const ready: OrchestratorTask[] = [];
    for (const task of this._tasks.values()) {
      if (this._results.has(task.id)) continue;
      let allDepsMet = true;
      for (const depID of task.dependsOn) {
        const result = this._results.get(depID);
        if (!result || !result.success) {
          allDepsMet = false;
          break;
        }
      }
      if (allDepsMet) ready.push(task);
    }
    return ready;
  }

  /** AllCompleted returns true if all tasks have results. */
  get allCompleted(): boolean {
    for (const task of this._tasks.keys()) {
      if (!this._results.has(task)) return false;
    }
    return true;
  }

  /** Summary returns a summary of all task results. */
  summary(): string {
    const total = this._tasks.size;
    let completed = 0;
    let failed = 0;
    for (const r of this._results.values()) {
      if (r.success) completed++;
      else failed++;
    }
    return `tasks: ${total} total, ${completed} completed, ${failed} failed`;
  }
}
