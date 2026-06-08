/**
 * UltraWork runs an AgentLoop autonomously with a step budget.
 * Ported from modules/agent/workflows/ultra_work.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { SessionId, AgentId } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import { ToolExecutor } from "../executor.js";
import { buildToolDefinitions } from "../tool-defs.js";
import { AgentLoop, defaultLoopConfig, type AgentLoopResult } from "../loop.js";

export class UltraWork {
  private _loop: AgentLoop;
  private _stepBudget: number;

  constructor(provider: AiProvider, registry: ToolRegistry, workDir: string, stepBudget: number) {
    const sid = SessionId.create();
    const agentCtx = new AgentContext(sid, workDir);
    agentCtx.setSystemPrompt("You are an autonomous agent working within a step budget. Complete the task efficiently.");

    const executor = new ToolExecutor(registry);
    const toolDefs = buildToolDefinitions(registry);
    const config = defaultLoopConfig();
    config.maxIterations = stepBudget;
    config.timeoutMs = 600_000; // 10 minutes
    config.autoApproveTools = true;
    const loop = new AgentLoop(AgentId.create(), provider, executor, agentCtx, config, toolDefs);

    this._loop = loop;
    this._stepBudget = stepBudget;
  }

  /** Run executes the workflow with the given task. */
  async run(signal: AbortSignal | undefined, task: string): Promise<AgentLoopResult> {
    this._loop.context.addUserMessage(task);
    const result = await this._loop.run({}, null);
    if (result.stopReason !== "completed") {
      throw new Error(`ultra work failed: stop reason ${result.stopReason}`);
    }
    return result;
  }
}
