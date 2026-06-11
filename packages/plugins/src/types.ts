/**
 * Core types for the plugins package.
 */

// ---------------------------------------------------------------------------
// Plugin Manifest
// ---------------------------------------------------------------------------

export interface PluginManifest {
  /** Plugin name (kebab-case, unique) */
  name: string;
  /** Semantic version */
  version: string;
  /** Human-readable description */
  description: string;
  /** Entry point module path (e.g., "./dist/index.js") */
  main: string;
  /** Tool names this plugin provides */
  tools: string[];
  /** Required permissions (e.g., "network", "filesystem", "env") */
  permissions: string[];
  /** Plugin dependencies: name -> version range */
  dependencies: Record<string, string>;
}

// ---------------------------------------------------------------------------
// Plugin Instance
// ---------------------------------------------------------------------------

export const PluginStatus = {
  Loaded: "loaded",
  Starting: "starting",
  Running: "running",
  Stopping: "stopping",
  Stopped: "stopped",
  Error: "error",
} as const;

export type PluginStatus = (typeof PluginStatus)[keyof typeof PluginStatus];

export interface PluginInstance {
  /** Plugin manifest */
  manifest: PluginManifest;
  /** Current lifecycle status */
  status: PluginStatus;
  /** Child process (for process-isolated plugins) */
  process?: import("node:child_process").ChildProcess;
  /** MCP client for communication */
  client?: import("@orangecoding/mcp").McpClient;
  /** When the plugin was started */
  startedAt?: Date;
  /** Last error message */
  error?: string;
  /** Number of restarts */
  restartCount: number;
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

export interface HealthStatus {
  /** Plugin name */
  name: string;
  /** Whether the plugin is alive and responding */
  alive: boolean;
  /** Uptime in milliseconds */
  uptimeMs: number;
  /** Last error message */
  lastError: string | null;
  /** Number of tools this plugin provides */
  toolCount: number;
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export interface PluginManagerConfig {
  /** Directories to search for plugins (default: [".claude/plugins"]) */
  searchDirs: string[];
  /** Health check interval in ms (default: 30_000) */
  healthCheckIntervalMs: number;
  /** Max restarts within window before giving up (default: 3) */
  maxRestarts: number;
  /** Restart window in ms (default: 60_000) */
  restartWindowMs: number;
  /** Timeout for plugin startup in ms (default: 30_000) */
  startTimeoutMs: number;
}

export const DEFAULT_PLUGIN_MANAGER_CONFIG: Required<PluginManagerConfig> = {
  searchDirs: [".claude/plugins"],
  healthCheckIntervalMs: 30_000,
  maxRestarts: 3,
  restartWindowMs: 60_000,
  startTimeoutMs: 30_000,
};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export class PluginError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly pluginName: string
  ) {
    super(message);
    this.name = "PluginError";
  }
}

export function newPluginError(code: string, message: string, pluginName: string): PluginError {
  return new PluginError(message, code, pluginName);
}
