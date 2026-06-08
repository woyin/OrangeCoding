/**
 * Shared helper for creating filtered agents.
 * Ported from modules/agent/agents/helper.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import type { AgentRole } from "@orangecoding/core";
import { SessionId, AgentId, AgentRole as AgentRoleEnum } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import { ToolExecutor, filteredRegistry } from "../executor.js";
import { buildToolDefinitions } from "../tool-defs.js";
import { AgentLoop, defaultLoopConfig } from "../loop.js";
import { BaseAgent } from "./base.js";

export function newFilteredAgent(
  provider: AiProvider,
  registry: ToolRegistry,
  workDir: string,
  role: AgentRole,
  allowedTools: string[],
  systemPrompt: string,
): BaseAgent {
  const sid = SessionId.create();
  const agentCtx = new AgentContext(sid, workDir);
  agentCtx.setSystemPrompt(systemPrompt);

  const fRegistry = filteredRegistry(registry, allowedTools);
  const executor = new ToolExecutor(fRegistry);
  const toolDefs = buildToolDefinitions(fRegistry);
  const loop = new AgentLoop(AgentId.create(), provider, executor, agentCtx, defaultLoopConfig(), toolDefs);

  return new BaseAgent(role, loop);
}
