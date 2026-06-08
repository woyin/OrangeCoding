/**
 * PlanningWorkflow uses a planner-style agent to decompose a task into steps.
 * Ported from modules/agent/workflows/planning.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { SessionId, AgentId } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import { ToolExecutor, filteredRegistry } from "../executor.js";
import { buildToolDefinitions } from "../tool-defs.js";
import { AgentLoop, defaultLoopConfig } from "../loop.js";

export interface PlanResult {
  steps: string[];
  rawPlan: string;
  durationMs: number;
}

export class PlanningWorkflow {
  private _provider: AiProvider;
  private _registry: ToolRegistry;
  private _workDir: string;

  constructor(provider: AiProvider, registry: ToolRegistry, workDir: string) {
    this._provider = provider;
    this._registry = registry;
    this._workDir = workDir;
  }

  /** Run executes the planning workflow and returns a list of steps. */
  async run(signal: AbortSignal | undefined, task: string): Promise<PlanResult> {
    const sid = SessionId.create();
    const agentCtx = new AgentContext(sid, this._workDir);
    agentCtx.setSystemPrompt("You are a planning agent. Decompose the given task into a numbered list of clear, actionable steps. Output only the steps, one per line.");

    const allowedTools = ["read_file", "find", "grep", "glob"];
    const fRegistry = filteredRegistry(this._registry, allowedTools);
    const executor = new ToolExecutor(fRegistry);
    const toolDefs = buildToolDefinitions(fRegistry);

    const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, defaultLoopConfig(), toolDefs);
    agentCtx.addUserMessage(task);

    const result = await loop.run({}, null);

    // Extract steps from the last assistant message
    const lastAssistant = agentCtx.conversation.lastAssistantMessage();
    if (!lastAssistant) {
      throw new Error("planning workflow: no assistant response");
    }

    const steps = parseSteps(lastAssistant.content);
    return {
      steps,
      rawPlan: lastAssistant.content,
      durationMs: result.durationMs,
    };
  }
}

/** parseSteps extracts numbered steps from plan text. */
function parseSteps(text: string): string[] {
  const steps: string[] = [];
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    // Match lines like "1. step" or "- step"
    if (trimmed.length > 2 && (trimmed[0] === "-" || (trimmed.charCodeAt(0) >= 0x30 && trimmed.charCodeAt(0) <= 0x39 && trimmed.slice(0, 5).includes(".")))) {
      // Strip leading number/bullet and whitespace
      const idx = trimmed.search(/[A-Za-z]/);
      if (idx > 0) {
        steps.push(trimmed.slice(idx));
      } else {
        steps.push(trimmed);
      }
    }
  }
  return steps;
}
