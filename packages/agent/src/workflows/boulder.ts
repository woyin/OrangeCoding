/**
 * BoulderRecovery detects a stuck agent, resets the context, and retries.
 * Ported from modules/agent/workflows/boulder.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { SessionId, AgentId } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import { ToolExecutor } from "../executor.js";
import { buildToolDefinitions } from "../tool-defs.js";
import { AgentLoop, defaultLoopConfig } from "../loop.js";

/** Result of a boulder-recovery run: attempt count, outcome, and last error. */
export interface BoulderResult {
  attempts: number;
  success: boolean;
  durationMs: number;
  finalError: string;
}

/**
 * Stuck-agent recovery strategy. When an agent fails or loops, BoulderRecovery
 * discards the contaminated context and retries the task from scratch (fresh
 * session + system prompt) up to `_maxRetries` times. The fresh-context reset
 * is the key trick: it breaks tool-call loops that persist within a single
 * conversation. Throws after all attempts are exhausted.
 */
export class BoulderRecovery {
  private _provider: AiProvider;
  private _registry: ToolRegistry;
  private _workDir: string;
  private _maxRetries: number;

  constructor(provider: AiProvider, registry: ToolRegistry, workDir: string, maxRetries: number) {
    this._provider = provider;
    this._registry = registry;
    this._workDir = workDir;
    this._maxRetries = maxRetries;
  }

  /**
   * Retry loop: each attempt spins up a brand-new AgentContext so prior
   * tool-call history cannot trap the agent in the same loop. Returns on the
   * first successful completion; throws after `_maxRetries` failures. Abort
   * is honored between attempts.
   */
  async run(signal: AbortSignal | undefined, task: string): Promise<BoulderResult> {
    const result: BoulderResult = {
      attempts: 0,
      success: false,
      durationMs: 0,
      finalError: "",
    };

    for (let attempt = 0; attempt < this._maxRetries; attempt++) {
      result.attempts = attempt + 1;

      // Fresh context for each attempt
      const sid = SessionId.create();
      const agentCtx = new AgentContext(sid, this._workDir);
      agentCtx.setSystemPrompt("You are a resilient agent. Complete the task without getting stuck in loops.");
      agentCtx.addUserMessage(task);

      const executor = new ToolExecutor(this._registry);
      const toolDefs = buildToolDefinitions(this._registry);
      const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, defaultLoopConfig(), toolDefs);

      const loopResult = await loop.run({}, null);
      result.durationMs += loopResult.durationMs;

      if (loopResult.stopReason === "completed") {
        result.success = true;
        return result;
      }

      result.finalError = `stop reason: ${loopResult.stopReason}`;

      // Check context cancellation
      if (signal?.aborted) {
        return result;
      }
    }

    throw new Error(`boulder recovery: all ${this._maxRetries} attempts exhausted, last error: ${result.finalError}`);
  }
}
