/**
 * Prometheus - planning agent.
 * Ported from modules/agent/agents/prometheus.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newPrometheus(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Planner,
    ["read_file", "find", "grep", "glob"],
    "You are Prometheus, the planner. You decompose tasks into clear, actionable plans. You analyze requirements and create step-by-step execution strategies.");
}
