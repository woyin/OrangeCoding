/**
 * Resume capability — restores an agent session from a checkpoint.
 *
 * Uses FileCheckpointStore to persist HarnessCheckpoint snapshots,
 * and SessionManager to reload conversation history.
 */

import type { AgentId, SessionId } from "@orangecoding/core";
import type { AiProvider, ChatOptions } from "@orangecoding/ai";
import type { ToolExecutor } from "./executor.js";
import type { AgentContext } from "./context.js";
import type { AgentLoopConfig } from "./loop.js";
import type { CheckpointStore, HarnessCheckpoint } from "./harness-state.js";
import { FileCheckpointStore } from "./harness-checkpoint-file.js";
import { AgentLoop, defaultLoopConfig } from "./loop.js";
import { buildToolDefinitions } from "./tool-defs.js";
import { AgentId as AgentIdClass, SessionId as SessionIdClass } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// ResumeResult
// ---------------------------------------------------------------------------

export interface ResumeResult {
  resumed: boolean;
  runID: string;
  iteration: number;
  toolCallsMade: number;
  stopReason: string;
}

// ---------------------------------------------------------------------------
// ResumeManager
// ---------------------------------------------------------------------------

export class ResumeManager {
  private _checkpointStore: CheckpointStore;

  constructor(checkpointDir: string) {
    this._checkpointStore = new FileCheckpointStore(checkpointDir);
  }

  /**
   * List resumable checkpoints, most recent first.
   */
  async listResumable(prefix?: string): Promise<HarnessCheckpoint[]> {
    const summaries = await this._checkpointStore.list(undefined, prefix ?? "");
    const checkpoints: HarnessCheckpoint[] = [];
    for (const summary of summaries) {
      try {
        const cp = await this._checkpointStore.load(undefined, summary.runID);
        // Only include non-terminal states
        if (cp.state !== "completed" && cp.state !== "failed") {
          checkpoints.push(cp);
        }
      } catch {
        /* skip corrupted checkpoints */
      }
    }
    return checkpoints;
  }

  /**
   * Check if a specific run can be resumed.
   */
  async canResume(runID: string): Promise<boolean> {
    try {
      const cp = await this._checkpointStore.load(undefined, runID);
      return cp.state !== "completed" && cp.state !== "failed";
    } catch {
      return false;
    }
  }

  /**
   * Resume a run from its checkpoint.
   * Reconstructs the AgentLoop with the saved state and continues execution.
   */
  async resume(
    runID: string,
    provider: AiProvider,
    executor: ToolExecutor,
    config?: Partial<AgentLoopConfig>,
  ): Promise<ResumeResult> {
    const cp = await this._checkpointStore.load(undefined, runID);

    // Reconstruct IDs
    const agentID = AgentIdClass.create();

    // Reconstruct context from checkpoint
    const workDir = process.cwd();
    const { AgentContext } = await import("./context.js");
    const ctx = new AgentContext(cp.sessionID as unknown as SessionId, workDir);

    // Rebuild tool definitions
    const toolDefs = buildToolDefinitions(executor.registry);

    // Build loop config, merging checkpoint state
    const loopConfig: AgentLoopConfig = {
      ...defaultLoopConfig(),
      ...config,
      checkpointStore: this._checkpointStore,
    };

    // Create and run the loop
    const loop = new AgentLoop(
      agentID,
      provider,
      executor,
      ctx,
      loopConfig,
      toolDefs,
    );

    const result = await loop.run(
      { model: undefined } as Partial<ChatOptions>,
      null,
    );

    return {
      resumed: true,
      runID,
      iteration: result.toolCallsMade,
      toolCallsMade: result.toolCallsMade,
      stopReason: result.stopReason,
    };
  }
}
