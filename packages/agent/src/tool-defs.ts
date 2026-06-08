/**
 * BuildToolDefinitions converts registered tools into provider-facing schemas.
 * Ported from modules/agent/tool_defs.go.
 */

import type { ToolDefinition, ToolParameter } from "@orangecoding/ai";
import { ToolRegistry } from "@orangecoding/tools";

/** BuildToolDefinitions converts registered tools into provider-facing schemas. */
export function buildToolDefinitions(registry: ToolRegistry): ToolDefinition[] {
  const defs: ToolDefinition[] = [];
  for (const t of registry.list()) {
    const params = toolParameters(t.parameters());
    defs.push({
      type: "function",
      function: {
        name: t.name(),
        description: t.description(),
        parameters: params,
      },
    });
  }
  return defs;
}

function toolParameters(raw: Record<string, unknown>): ToolParameter {
  if (!raw || !raw.type || typeof raw.type !== "string") {
    return { type: "object", properties: {} };
  }
  const params = raw as unknown as ToolParameter;
  if (!params.properties) {
    params.properties = {};
  }
  return params;
}
