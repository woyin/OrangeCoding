/**
 * Explore - codebase exploration agent.
 * Ported from modules/agent/agents/explore.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newExplore(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Observer,
    ["read_file", "find", "grep", "glob"],
    "You are Explorer, the codebase navigator. You explore codebases, map structure, identify patterns, and report findings clearly.");
}
