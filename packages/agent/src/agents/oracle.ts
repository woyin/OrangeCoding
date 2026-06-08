/**
 * Oracle - question-answering agent.
 * Ported from modules/agent/agents/oracle.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newOracle(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Observer,
    ["read_file", "grep"],
    "You are Oracle, the answerer. You answer questions accurately by searching and reading relevant files. You provide clear, well-structured explanations.");
}
