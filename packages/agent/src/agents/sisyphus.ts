/**
 * Sisyphus - primary general-purpose coding agent.
 * Ported from modules/agent/agents/sisyphus.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentId, SessionId, AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import { ToolExecutor } from "../executor.js";
import { buildToolDefinitions } from "../tool-defs.js";
import { AgentLoop, defaultLoopConfig } from "../loop.js";
import { BaseAgent } from "./base.js";

export function newSisyphus(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  const sid = SessionId.create();
  const agentCtx = new AgentContext(sid, workDir);
  agentCtx.setSystemPrompt("You are Sisyphus, a general-purpose coding agent. You write, debug, review, and refactor code. You are thorough, methodical, and never give up on a task.");

  const executor = new ToolExecutor(registry);
  const toolDefs = buildToolDefinitions(registry);
  const loop = new AgentLoop(AgentId.create(), provider, executor, agentCtx, defaultLoopConfig(), toolDefs);

  return new BaseAgent(AgentRole.Coder, loop);
}
