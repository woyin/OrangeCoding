/**
 * Default registry - creates a ToolRegistry with all built-in tools registered.
 *
 * Ported from modules/tools/default_registry.go.
 */

import { ToolRegistry } from "./registry.js";
import { SecurityPolicy } from "./security.js";
import { BashTool } from "./bash-tool.js";
import { ReadFileTool, WriteFileTool, EditFileTool, DeleteFileTool, ListDirectoryTool } from "./file-tools.js";
import { GrepTool, FindTool, GlobTool } from "./search-tools.js";
import { FetchTool, PythonTool, CalcTool, TaskTool, newBrowserTool, newSshTool, newLspTool, newWebSearchTool, newNotebookTool } from "./other-tools.js";

/**
 * Creates a ToolRegistry with all built-in tools registered.
 */
export function createDefaultRegistry(): ToolRegistry {
  const r = new ToolRegistry();

  // Shell
  r.register(new BashTool(SecurityPolicy.default()));

  // File operations
  r.register(new ReadFileTool());
  r.register(new WriteFileTool());
  r.register(new EditFileTool());
  r.register(new DeleteFileTool());
  r.register(new ListDirectoryTool());

  // Search
  r.register(new GrepTool());
  r.register(new FindTool());
  r.register(new GlobTool());

  // Network
  r.register(new FetchTool());

  // Language runtimes
  r.register(new PythonTool());

  // Utility
  r.register(new CalcTool());
  r.register(new TaskTool());

  // Stubs (not yet implemented)
  r.register(newBrowserTool());
  r.register(newSshTool());
  r.register(newLspTool());
  r.register(newWebSearchTool());
  r.register(newNotebookTool());

  return r;
}
