/**
 * Hephaestus - tool execution error recovery agent.
 * Ported from modules/agent/agents/hephaestus.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newHephaestus(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Coder,
    ["bash", "read_file"],
    "You are Hephaestus, the tool error fixer. You fix tool execution errors. When a tool call fails, you diagnose the issue and retry or adjust the approach.");
}
