/**
 * HarnessEngine owns the state-machine checkpoint for one run.
 * Ported from modules/agent/harness_engine.go.
 */

import { TokenUsage } from "@orangecoding/core";
import type { SessionId } from "@orangecoding/core";
import type { HarnessState, HarnessCheckpoint } from "./harness-state.js";
import { MemoryCheckpointStore, cloneHarnessCheckpoint } from "./harness-state.js";
import type { CheckpointStore } from "./harness-state.js";
import { HarnessState as HS } from "./harness-state.js";

// ---------------------------------------------------------------------------
// HarnessEngineConfig
// ---------------------------------------------------------------------------
// Wires the engine to a run+session identity and an optional checkpoint
// store. When no store is supplied, an in-memory store is used (lost on exit).

export interface HarnessEngineConfig {
  runID: string;
  sessionID: SessionId;
  checkpointStore?: CheckpointStore;
}

// ---------------------------------------------------------------------------
// Allowed transitions
// ---------------------------------------------------------------------------
// The legal state-machine edges. Terminal states (Completed/Stopped/Failed)
// have empty outbound lists. isAllowedTransition is the single validator so
// the graph lives in one place.

const ALLOWED_TRANSITIONS: Map<HarnessState, HarnessState[]> = new Map([
  [HS.Init, [HS.BuildContext, HS.Failed]],
  [HS.BuildContext, [HS.ModelCall, HS.Stopped, HS.Failed]],
  [HS.ModelCall, [HS.GuardrailCheck, HS.Failed]],
  [HS.GuardrailCheck, [HS.ToolDispatch, HS.Completed, HS.Stopped, HS.Failed]],
  [HS.ToolDispatch, [HS.Observe, HS.Failed]],
  [HS.Observe, [HS.MemoryUpdate, HS.Failed]],
  [HS.MemoryUpdate, [HS.Checkpoint, HS.Failed]],
  [HS.Checkpoint, [HS.DecideNext, HS.Failed]],
  [HS.DecideNext, [HS.BuildContext, HS.Completed, HS.Stopped, HS.Failed]],
  [HS.Completed, []],
  [HS.Stopped, []],
  [HS.Failed, []],
]);

/** Return true if `from -> to` is a legal edge (or a self-transition). */
function isAllowedTransition(from: HarnessState, to: HarnessState): boolean {
  if (from === to) return true;
  const allowed = ALLOWED_TRANSITIONS.get(from);
  if (!allowed) return false;
  return allowed.includes(to);
}

// ---------------------------------------------------------------------------
// HarnessEngine
// ---------------------------------------------------------------------------

/**
 * State-machine owner for a single agent run. Tracks one {@link
 * HarnessCheckpoint}, enforces legal transitions, appends a trace entry per
 * move, and persists via the configured {@link CheckpointStore}. All mutators
 * reject on abort so a cancelled run cannot half-commit a transition.
 */

export class HarnessEngine {
  private _config: HarnessEngineConfig;
  private _checkpoint: HarnessCheckpoint;
  private _store: CheckpointStore;

  constructor(config: HarnessEngineConfig) {
    this._config = config;
    this._store = config.checkpointStore ?? new MemoryCheckpointStore();
    this._checkpoint = {} as HarnessCheckpoint;
  }

  /** Initialize a fresh checkpoint in Init and immediately transition to BuildContext. */
  async start(signal: AbortSignal | undefined, task: string): Promise<HarnessCheckpoint> {
    if (!this._config.runID) {
      throw new Error("harness engine: run id is required");
    }
    this._checkpoint = {
      runID: this._config.runID,
      sessionID: this._config.sessionID,
      task,
      state: HS.Init,
      iteration: 0,
      toolCallsMade: 0,
      tokenUsage: new TokenUsage(0, 0, 0),
      updatedAt: new Date(),
    };
    return this.transition(signal, HS.BuildContext, "start");
  }

  /**
   * Move to `next` state. Validates the edge, appends a trace entry tagged
   * with `reason`, persists, and returns a deep clone so callers cannot
   * mutate the engine's internal checkpoint.
   */
  async transition(signal: AbortSignal | undefined, next: HarnessState, reason: string): Promise<HarnessCheckpoint> {
    if (signal?.aborted) throw new Error("aborted");
    if (!this._checkpoint.runID) {
      throw new Error("harness engine: start must be called before transition");
    }
    if (!isAllowedTransition(this._checkpoint.state, next)) {
      throw new Error(`harness engine: illegal transition ${this._checkpoint.state} -> ${next}`);
    }

    const from = this._checkpoint.state;
    this._checkpoint.state = next;
    this._checkpoint.updatedAt = new Date();
    if (!this._checkpoint.trace) this._checkpoint.trace = [];
    this._checkpoint.trace.push({
      from,
      to: next,
      reason,
      createdAt: new Date(),
    });
    if (next === HS.Completed) {
      this._checkpoint.stopReason = "completed";
    }
    await this._store.save(signal, this._checkpoint);
    return cloneHarnessCheckpoint(this._checkpoint);
  }

  /**
   * Apply an in-place mutation to the checkpoint (e.g. bump iteration, record
   * token usage) without a state change. Still persists and returns a clone.
   */
  async update(
    signal: AbortSignal | undefined,
    mutate: (cp: HarnessCheckpoint) => void,
  ): Promise<HarnessCheckpoint> {
    if (signal?.aborted) throw new Error("aborted");
    if (!this._checkpoint.runID) {
      throw new Error("harness engine: start must be called before update");
    }
    mutate(this._checkpoint);
    this._checkpoint.updatedAt = new Date();
    await this._store.save(signal, this._checkpoint);
    return cloneHarnessCheckpoint(this._checkpoint);
  }
}
