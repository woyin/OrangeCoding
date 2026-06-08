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

export interface ExecutionResult {
  stepsCompleted: number;
  stepsFailed: number;
  durationMs: number;
}

export class ExecutionWorkflow {
  private _provider: AiProvider;
  private _registry: ToolRegistry;
  private _workDir: string;

  constructor(provider: AiProvider, registry: ToolRegistry, workDir: string) {
    this._provider = provider;
    this._registry = registry;
    this._workDir = workDir;
  }

  /** Run executes the given plan steps sequentially. */
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
