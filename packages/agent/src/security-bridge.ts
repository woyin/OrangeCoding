/**
 * SecurityGuard validates tool calls before execution.
 * Ported from modules/agent/security_bridge.go.
 */

import type { AgentId } from "@orangecoding/core";

export interface SecurityGuard {
  validateToolCall(agentID: AgentId, toolName: string): [boolean, string];
}
