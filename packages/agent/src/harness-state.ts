/**
 * HarnessState is the explicit state-machine phase for a harness run.
 * Ported from modules/agent/harness_state.go.
 */

import type { SessionId, TokenUsage } from "@orangecoding/core";
import type { StopReason } from "./harness-profile.js";

// ---------------------------------------------------------------------------
// HarnessState
// ---------------------------------------------------------------------------

export type HarnessState =
  | "init"
  | "build_context"
  | "model_call"
  | "guardrail_check"
  | "tool_dispatch"
  | "observe"
  | "memory_update"
  | "checkpoint"
  | "decide_next"
  | "completed"
  | "stopped"
  | "failed";

export const HarnessState = {
  Init: "init" as HarnessState,
  BuildContext: "build_context" as HarnessState,
  ModelCall: "model_call" as HarnessState,
  GuardrailCheck: "guardrail_check" as HarnessState,
  ToolDispatch: "tool_dispatch" as HarnessState,
  Observe: "observe" as HarnessState,
  MemoryUpdate: "memory_update" as HarnessState,
  Checkpoint: "checkpoint" as HarnessState,
  DecideNext: "decide_next" as HarnessState,
  Completed: "completed" as HarnessState,
  Stopped: "stopped" as HarnessState,
  Failed: "failed" as HarnessState,
};

// ---------------------------------------------------------------------------
// HarnessTraceEvent
// ---------------------------------------------------------------------------

export interface HarnessTraceEvent {
  from: HarnessState;
  to: HarnessState;
  reason?: string;
  metadata?: Record<string, string>;
  createdAt: Date;
}

// ---------------------------------------------------------------------------
// ContextBlock (forward declaration for checkpoint)
// ---------------------------------------------------------------------------

export type ContextBlockKind = "system" | "harness" | "task" | "memory" | "conversation" | "tool_result";

export interface ContextBlock {
  kind: ContextBlockKind;
  content: string;
  stable: boolean;
  priority: number;
  tokenEstimate: number;
}

// ---------------------------------------------------------------------------
// HarnessCheckpoint
// ---------------------------------------------------------------------------

export interface HarnessCheckpoint {
  runID: string;
  sessionID: SessionId;
  task: string;
  state: HarnessState;
  iteration: number;
  toolCallsMade: number;
  tokenUsage: TokenUsage;
  stopReason?: StopReason;
  contextBlocks?: ContextBlock[];
  memoryKeys?: string[];
  recentToolKeys?: string[];
  trace?: HarnessTraceEvent[];
  updatedAt: Date;
  lastErrorMessage?: string;
}

// ---------------------------------------------------------------------------
// CheckpointSummary
// ---------------------------------------------------------------------------

export interface CheckpointSummary {
  runID: string;
  sessionID: SessionId;
  task: string;
  state: HarnessState;
  stopReason?: StopReason;
  iteration: number;
  toolCallsMade: number;
  updatedAt: Date;
}

/** Creates a lightweight summary from a checkpoint. */
export function checkpointSummary(cp: HarnessCheckpoint): CheckpointSummary {
  return {
    runID: cp.runID,
    sessionID: cp.sessionID,
    task: cp.task,
    state: cp.state,
    stopReason: cp.stopReason,
    iteration: cp.iteration,
    toolCallsMade: cp.toolCallsMade,
    updatedAt: cp.updatedAt,
  };
}

// ---------------------------------------------------------------------------
// CheckpointStore interface
// ---------------------------------------------------------------------------

export interface CheckpointStore {
  save(signal: AbortSignal | undefined, cp: HarnessCheckpoint): Promise<void>;
  load(signal: AbortSignal | undefined, runID: string): Promise<HarnessCheckpoint>;
  list(signal: AbortSignal | undefined, prefix: string): Promise<CheckpointSummary[]>;
  delete(signal: AbortSignal | undefined, runID: string): Promise<void>;
}

// ---------------------------------------------------------------------------
// MemoryCheckpointStore
// ---------------------------------------------------------------------------

export class MemoryCheckpointStore implements CheckpointStore {
  private _checkpoints: Map<string, HarnessCheckpoint>;

  constructor() {
    this._checkpoints = new Map();
  }

  async save(_signal: AbortSignal | undefined, cp: HarnessCheckpoint): Promise<void> {
    cp.updatedAt = new Date();
    this._checkpoints.set(cp.runID, cloneHarnessCheckpoint(cp));
  }

  async load(_signal: AbortSignal | undefined, runID: string): Promise<HarnessCheckpoint> {
    const cp = this._checkpoints.get(runID);
    if (!cp) throw new Error(`checkpoint "${runID}" not found`);
    // Return the stored reference — callers should not mutate.
    // If mutation is needed, the caller should clone.
    return cp;
  }

  async list(_signal: AbortSignal | undefined, prefix: string): Promise<CheckpointSummary[]> {
    const summaries: CheckpointSummary[] = [];
    for (const cp of this._checkpoints.values()) {
      if (prefix && !cp.runID.startsWith(prefix)) continue;
      summaries.push(checkpointSummary(cp));
    }
    return summaries;
  }

  async delete(_signal: AbortSignal | undefined, runID: string): Promise<void> {
    if (!this._checkpoints.has(runID)) {
      throw new Error(`checkpoint "${runID}" not found`);
    }
    this._checkpoints.delete(runID);
  }
}

export function cloneHarnessCheckpoint(cp: HarnessCheckpoint): HarnessCheckpoint {
  return {
    ...cp,
    contextBlocks: cp.contextBlocks ? [...cp.contextBlocks] : undefined,
    memoryKeys: cp.memoryKeys ? [...cp.memoryKeys] : undefined,
    recentToolKeys: cp.recentToolKeys ? [...cp.recentToolKeys] : undefined,
    trace: cp.trace ? cp.trace.map((e) => ({ ...e })) : undefined,
  };
}
