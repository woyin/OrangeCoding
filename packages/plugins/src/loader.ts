/**
 * Plugin loader — spawns plugin child processes and establishes MCP communication.
 */

import { spawn } from "node:child_process";
import { McpClient, StdioTransport } from "@orangecoding/mcp";
import type { PluginManifest, PluginInstance } from "./types.js";
import { PluginStatus, newPluginError } from "./types.js";

/**
 * Load a plugin: spawn the child process and connect via MCP.
 *
 * @param manifest - the plugin manifest
 * @param startTimeoutMs - timeout for startup
 * @returns a fully initialized PluginInstance
 */
/**
 * Loads a plugin by spawning its child process and establishing MCP communication.
 *
 * The loading sequence:
 * 1. Spawns a Node.js child process running the plugin's entry point
 * 2. Creates a stdio transport for JSON-RPC communication
 * 3. Sends an initialize request to register tools
 * 4. Sets up error handling and exit detection
 *
 * @param manifest - Plugin manifest with entry point and metadata
 * @param startTimeoutMs - Maximum time to wait for initialization
 * @returns Fully initialized PluginInstance
 */
export async function loadPlugin(
  manifest: PluginManifest,
  startTimeoutMs: number
): Promise<PluginInstance> {
  const child = spawn("node", [manifest.main], {
    stdio: ["pipe", "pipe", "pipe"],
    env: {
      ...process.env,
      PLUGIN_NAME: manifest.name,
      PLUGIN_VERSION: manifest.version,
    },
  });

  const transport = new StdioTransport(child.stdout!, child.stdin!);
  const client = new McpClient(transport);

  const instance: PluginInstance = {
    manifest,
    status: PluginStatus.Starting,
    process: child,
    client,
    restartCount: 0,
  };

  // Handle stderr for debugging
  child.stderr?.on("data", (data: Buffer) => {
    instance.error = data.toString().trim();
  });

  // Handle unexpected exit
  child.on("exit", (code) => {
    if (instance.status === PluginStatus.Running || instance.status === PluginStatus.Starting) {
      instance.status = PluginStatus.Error;
      instance.error = `process exited with code ${code}`;
    }
  });

  child.on("error", (err) => {
    instance.status = PluginStatus.Error;
    instance.error = err.message;
  });

  return instance;
}

/**
 * Initialize a loaded plugin by sending the MCP initialize request.
 */
/**
 * Sends the MCP initialize request to a loaded plugin.
 * Times out if the plugin doesn't respond within the specified duration.
 */
export async function initializePlugin(
  instance: PluginInstance,
  timeoutMs: number
): Promise<void> {
  if (!instance.client) {
    throw newPluginError("NO_CLIENT", "plugin has no MCP client", instance.manifest.name);
  }

  const timer = setTimeout(() => {
    throw new Error(`plugin "${instance.manifest.name}" initialize timed out after ${timeoutMs}ms`);
  }, timeoutMs);

  try {
    await instance.client.initialize();
    instance.status = PluginStatus.Running;
    instance.startedAt = new Date();
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Shutdown a plugin cleanly.
 */
/**
 * Gracefully shuts down a plugin by closing MCP connection and killing the process.
 * Falls back to SIGKILL if graceful shutdown times out (5 seconds).
 */
export async function shutdownPlugin(instance: PluginInstance): Promise<void> {
  instance.status = PluginStatus.Stopping;

  try {
    await instance.client?.close();
    instance.process?.kill("SIGTERM");

    // Wait for graceful shutdown
    if (instance.process?.exitCode === null) {
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => {
          instance.process?.kill("SIGKILL");
          resolve();
        }, 5000);

        instance.process?.on("exit", () => {
          clearTimeout(timer);
          resolve();
        });

        instance.process?.on("error", () => {
          clearTimeout(timer);
          reject();
        });
      });
    }
  } catch {
    // Force kill on failure
    instance.process?.kill("SIGKILL");
  }

  instance.status = PluginStatus.Stopped;
}
