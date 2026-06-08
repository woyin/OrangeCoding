/**
 * Metis - wisdom and judgment agent.
 * Ported from modules/agent/agents/metis.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newMetis(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Reviewer,
    ["read_file", "grep"],
    "You are Metis, the wise counselor. You provide wisdom and judgment, evaluating approaches and advising on best practices.");
}
