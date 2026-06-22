/**
 * ExecutionWorkflow uses an executor-style agent to execute plan steps.
 * Ported from modules/agent/workflows/execution.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { SessionId, AgentId } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import { ToolExecutor, filteredRegistry } from "../executor.js";
import { buildToolDefinitions } from "../tool-defs.js";
import { AgentLoop, defaultLoopConfig } from "../loop.js";

/** Outcome of running a plan: per-step pass/fail counts and cumulative wall time. */
export interface ExecutionResult {
  stepsCompleted: number;
  stepsFailed: number;
  durationMs: number;
}

/**
 * Sequential plan executor. Each step gets its own AgentContext (fresh session)
 * running with a write-capable tool subset (bash + file editors). Steps run in
 * order; a failed step does not abort the remaining steps — it is counted and
 * the workflow continues. Honors abort between steps.
 */
export class ExecutionWorkflow {
  private _provider: AiProvider;
  private _registry: ToolRegistry;
  private _workDir: string;

  constructor(provider: AiProvider, registry: ToolRegistry, workDir: string) {
    this._provider = provider;
    this._registry = registry;
    this._workDir = workDir;
  }

  /**
   * Execute each step in order with a fresh agent context. Returns a tally of
   * completed/failed steps plus total duration. Aborts are checked between
   * steps; a single step's failure is recorded, not thrown.
   */
  async run(signal: AbortSignal | undefined, steps: string[]): Promise<ExecutionResult> {
    const result: ExecutionResult = {
      stepsCompleted: 0,
      stepsFailed: 0,
      durationMs: 0,
    };

    for (let i = 0; i < steps.length; i++) {
      const step = steps[i]!;

      const sid = SessionId.create();
      const agentCtx = new AgentContext(sid, this._workDir);
      agentCtx.setSystemPrompt("You are an executor agent. Execute the given step precisely and report the result.");

      const allowedTools = ["bash", "read_file", "write_file", "edit_file"];
      const fRegistry = filteredRegistry(this._registry, allowedTools);
      const executor = new ToolExecutor(fRegistry);
      const toolDefs = buildToolDefinitions(fRegistry);

      const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, defaultLoopConfig(), toolDefs);
      agentCtx.addUserMessage(`Execute step ${i + 1}: ${step}`);

      const loopResult = await loop.run({}, null);
      result.durationMs += loopResult.durationMs;

      if (loopResult.stopReason === "completed") {
        result.stepsCompleted++;
      } else {
        result.stepsFailed++;
      }

      // Check context cancellation
      if (signal?.aborted) {
        return result;
      }
    }

    return result;
  }
}
