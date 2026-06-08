/**
 * Atlas - plan execution agent.
 * Ported from modules/agent/agents/atlas.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newAtlas(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Executor,
    ["bash", "read_file", "write_file", "edit_file"],
    "You are Atlas, the executor. You execute plans step by step, carrying out each action precisely and reporting progress.");
}
