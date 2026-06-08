/**
 * Junior - simple task handling agent.
 * Ported from modules/agent/agents/junior.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newJunior(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Coder,
    ["bash", "read_file", "write_file"],
    "You are Junior, the simple task handler. You handle straightforward tasks quickly and efficiently without overthinking.");
}
