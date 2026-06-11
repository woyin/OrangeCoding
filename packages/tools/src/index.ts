/**
 * @orangecoding/tools - Tool implementations with permission system and security policies.
 *
 * Re-exports all public API from the package.
 */

// Core Tool types
export { ToolError, ToolBudgetTracker } from "./tool.js";
export type { Tool, ToolMetadata, ToolErrorKind, BudgetCheckResult } from "./tool.js";
export { defaultMetadata, readOnlyMetadata, destructiveMetadata, withBudget } from "./tool.js";

// Registry
export { ToolRegistry } from "./registry.js";

// Default registry
export { createDefaultRegistry } from "./default-registry.js";

// Security
export { PathValidator, SecurityPolicy, DefaultBlockedCommands, lookPath } from "./security.js";

// Permissions
export { PermissionDecision } from "./permissions.js";
export type { PermissionContext } from "./permissions.js";

// Sandbox
export { SandboxPermissionManager, strictSandbox, devSandbox } from "./sandbox.js";
export type { PermissionRule, PermissionAction, SandboxConfig, PermissionCheckResult } from "./sandbox.js";

// Batch execution
export { executeBatch } from "./batch.js";
export type { ExecuteResult } from "./batch.js";

// Approval
export { AutoApproveHandler, AutoDenyHandler, CLIApprovalHandler } from "./approval.js";
export type { ApprovalHandler, ApprovalRequest, ApprovalResult } from "./approval.js";

// Concrete tools
export { BashTool } from "./bash-tool.js";
export { ReadFileTool, WriteFileTool, EditFileTool, DeleteFileTool, ListDirectoryTool, computeContentHash } from "./file-tools.js";
export { MultiEditTool, PatchEditTool } from "./multi-edit-tool.js";
export { GrepTool, FindTool, GlobTool } from "./search-tools.js";
export { FetchTool, PythonTool, CalcTool, TaskTool, BrowserTool, SshTool, NotebookTool } from "./other-tools.js";
