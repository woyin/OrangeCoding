/**
 * @module agent-helpers
 *
 * Shared helper functions for agent implementations.
 *
 * Provides factory functions for creating agents with specific tool restrictions.
 * A "filtered agent" is a child agent that can only use a subset of the
 * available tools, useful for delegating specialized subtasks.
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

/**
 * Creates a new agent with a restricted tool set.
 *
 * @param provider - AI provider for model calls
 * @param registry - full tool registry (will be filtered)
 * @param workDir - working directory for the agent
 * @param role - agent role (executor, reviewer, etc.)
 * @param allowedTools - tool names the agent is allowed to use
 * @param systemPrompt - system prompt for the agent
 * @returns a configured BaseAgent instance
 */
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
