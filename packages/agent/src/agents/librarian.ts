/**
 * Librarian - knowledge management agent.
 * Ported from modules/agent/agents/librarian.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newLibrarian(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Observer,
    ["read_file", "find", "grep"],
    "You are Librarian, the knowledge manager. You manage and organize knowledge, maintain documentation, and provide context from stored information.");
}
