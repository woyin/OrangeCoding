/**
 * Default registry - creates a ToolRegistry with all built-in tools registered.
 *
 * Updated: LSP, WebSearch, and Git tools are now real implementations (not stubs).
 */

import { ToolRegistry } from "./registry.js";
import { SecurityPolicy } from "./security.js";
import { BashTool } from "./bash-tool.js";
import { ReadFileTool, WriteFileTool, EditFileTool, DeleteFileTool, ListDirectoryTool } from "./file-tools.js";
import { MultiEditTool, PatchEditTool } from "./multi-edit-tool.js";
import { GrepTool, FindTool, GlobTool } from "./search-tools.js";
import { FetchTool, PythonTool, CalcTool, TaskTool, newBrowserTool, newSshTool, newNotebookTool } from "./other-tools.js";
import { GitTool } from "./git-tool.js";
import { WebSearchTool } from "./web-search-tool.js";
import { LspTool } from "./lsp-tool.js";

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

  // Multi-edit tools (Claude Code-like)
  r.register(new MultiEditTool());
  r.register(new PatchEditTool());

  // Search
  r.register(new GrepTool());
  r.register(new FindTool());
  r.register(new GlobTool());

  // Network & web
  r.register(new FetchTool());
  r.register(new WebSearchTool());
  r.register(newBrowserTool());

  // Code navigation (LSP)
  r.register(new LspTool());

  // Git operations
  r.register(new GitTool());

  // Language runtimes
  r.register(new PythonTool());

  // Remote execution
  r.register(newSshTool());

  // Notebooks
  r.register(newNotebookTool());

  // Utility
  r.register(new CalcTool());
  r.register(new TaskTool());

  return r;
}
