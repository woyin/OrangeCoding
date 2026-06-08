/**
 * ToolRegistry stores and retrieves tools by name.
 * Mirrors Go's `tools.ToolRegistry`.
 *
 * Ported from modules/tools/registry.go.
 */

import type { Tool } from "./tool.js";

export class ToolRegistry {
  private _tools = new Map<string, Tool>();

  /** Register adds a tool to the registry. If a tool with the same name already exists, it is replaced. */
  register(t: Tool): void {
    this._tools.set(t.name(), t);
  }

  /** Get retrieves a tool by name. Returns the tool and true if found, or undefined and false. */
  get(name: string): [Tool, true] | [undefined, false] {
    const t = this._tools.get(name);
    if (t !== undefined) {
      return [t, true];
    }
    return [undefined, false];
  }

  /** List returns all registered tools. */
  list(): Tool[] {
    return Array.from(this._tools.values());
  }
}
