/**
 * @orangecoding/tools - Tool implementations with permission system and security policies.
 *
 * Re-exports all public API from the package.
 */

// Core Tool types
export { ToolError } from "./tool.js";
export type { Tool, ToolMetadata, ToolErrorKind } from "./tool.js";
export { defaultMetadata, readOnlyMetadata, destructiveMetadata } from "./tool.js";

// Registry
export { ToolRegistry } from "./registry.js";

// Default registry
export { createDefaultRegistry } from "./default-registry.js";

// Security
export { PathValidator, SecurityPolicy, DefaultBlockedCommands, lookPath } from "./security.js";

// Permissions
export { PermissionDecision } from "./permissions.js";
export type { PermissionContext } from "./permissions.js";

// Batch execution
export { executeBatch } from "./batch.js";
export type { ExecuteResult } from "./batch.js";

// Concrete tools
export { BashTool } from "./bash-tool.js";
export { ReadFileTool, WriteFileTool, EditFileTool, DeleteFileTool, ListDirectoryTool } from "./file-tools.js";
export { GrepTool, FindTool, GlobTool } from "./search-tools.js";
export { FetchTool, PythonTool, CalcTool, TaskTool, StubTool, newBrowserTool, newSshTool, newLspTool, newWebSearchTool, newNotebookTool } from "./other-tools.js";
