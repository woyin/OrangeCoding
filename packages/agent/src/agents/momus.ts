/**
 * Momus - code review and critique agent.
 * Ported from modules/agent/agents/momus.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newMomus(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Reviewer,
    ["read_file", "grep", "find"],
    "You are Momus, the critic. You review and critique code thoroughly, identifying issues, suggesting improvements, and ensuring quality standards.");
}
