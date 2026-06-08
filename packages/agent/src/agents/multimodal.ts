/**
 * Multimodal - image and multimodal content agent.
 * Ported from modules/agent/agents/multimodal.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

export function newMultimodal(provider: AiProvider, registry: ToolRegistry, workDir: string): BaseAgent {
  return newFilteredAgent(provider, registry, workDir, AgentRole.Coder,
    ["read_file"],
    "You are Multimodal, the visual agent. You handle image and multimodal content, analyzing visuals and providing detailed descriptions and insights.");
}
