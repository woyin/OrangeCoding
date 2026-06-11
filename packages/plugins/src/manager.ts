/**
 * PluginManager — runtime lifecycle manager for plugins.
 *
 * Handles discovery, loading, starting, stopping, health monitoring,
 * and automatic restart with circuit breaker protection.
 */

import { PluginManifest, PluginInstance, PluginStatus, PluginError, newPluginError, DEFAULT_PLUGIN_MANAGER_CONFIG, HealthStatus } from "./types.js";
import type { PluginManagerConfig } from "./types.js";
import { discoverPlugins, resolvePluginDependencies } from "./discovery.js";
import { loadPlugin, initializePlugin, shutdownPlugin } from "./loader.js";
import { healthCheck, CircuitBreaker } from "./health.js";

// ---------------------------------------------------------------------------
// Manager Class
// ---------------------------------------------------------------------------

export class PluginManager {
  private readonly _config: Required<PluginManagerConfig>;
  private readonly _plugins: Map<string, PluginInstance> = new Map();
  private readonly _breaker: CircuitBreaker;
  private _healthTimer: NodeJS.Timeout | null = null;

  // -------------------------------------------------------------------------
  // Callbacks
  // -------------------------------------------------------------------------

  onPluginLoaded: ((instance: PluginInstance) => void) | null = null;
  onPluginStarted: ((instance: PluginInstance) => void) | null = null;
  onPluginStopped: ((instance: PluginInstance) => void) | null = null;
  onPluginError: ((instance: PluginInstance, error: Error) => void) | null = null;

  // -------------------------------------------------------------------------
  // Constructor
  // -------------------------------------------------------------------------

  constructor(config?: Partial<PluginManagerConfig>) {
    this._config = {
      searchDirs: config?.searchDirs ?? DEFAULT_PLUGIN_MANAGER_CONFIG.searchDirs,
      healthCheckIntervalMs: config?.healthCheckIntervalMs ?? DEFAULT_PLUGIN_MANAGER_CONFIG.healthCheckIntervalMs,
      maxRestarts: config?.maxRestarts ?? DEFAULT_PLUGIN_MANAGER_CONFIG.maxRestarts,
      restartWindowMs: config?.restartWindowMs ?? DEFAULT_PLUGIN_MANAGER_CONFIG.restartWindowMs,
      startTimeoutMs: config?.startTimeoutMs ?? DEFAULT_PLUGIN_MANAGER_CONFIG.startTimeoutMs,
    };
    this._breaker = new CircuitBreaker(this._config.maxRestarts, this._config.restartWindowMs);
  }

  // -------------------------------------------------------------------------
  // Discovery
  // -------------------------------------------------------------------------

  /**
   * Discover plugins in the configured search directories.
   */
  async discover(dirs?: string[]): Promise<PluginManifest[]> {
    const searchDirs = dirs ?? this._config.searchDirs;
    const all: PluginManifest[] = [];

    for (const dir of searchDirs) {
      const found = await discoverPlugins(dir);
      all.push(...found);
    }

    return resolvePluginDependencies(all);
  }

  /**
   * Scan a single directory for plugins.
   */
  async scan(searchDir?: string): Promise<PluginManifest[]> {
    const dir = searchDir ?? this._config.searchDirs[0] ?? ".claude/plugins";
    const found = await discoverPlugins(dir);
    return resolvePluginDependencies(found);
  }

  // -------------------------------------------------------------------------
  // Lifecycle
  // -------------------------------------------------------------------------

  /**
   * Load a plugin from its manifest (does not start it).
   *
   * Loads the plugin into _plugins map.
   */
  async load(manifest: PluginManifest): Promise<PluginInstance> {
    if (this._plugins.has(manifest.name)) {
      throw newPluginError("ALREADY_LOADED", `plugin already loaded: ${manifest.name}`, manifest.name);
    }

    const instance: PluginInstance = {
      manifest,
      status: PluginStatus.Loaded,
      restartCount: 0,
    };

    this._plugins.set(manifest.name, instance);
    this.onPluginLoaded?.(instance);

    return instance;
  }

  /**
   * Start a loaded plugin (spawn process, initialize MCP).
   */
  async start(name: string): Promise<void> {
    const instance = this._plugins.get(name);
    if (!instance) {
      throw newPluginError("NOT_FOUND", `plugin not found: ${name}`, name);
    }

    if (instance.status === PluginStatus.Running) return;

    try {
      const loaded = await loadPlugin(instance.manifest, this._config.startTimeoutMs);

      // Copy the runtime fields
      instance.process = loaded.process;
      instance.client = loaded.client;
      instance.status = PluginStatus.Starting;

      await initializePlugin(instance, this._config.startTimeoutMs);

      instance.restartCount = 0;
      this._breaker.recordSuccess(name);
      this.onPluginStarted?.(instance);
    } catch (err) {
      instance.status = PluginStatus.Error;
      instance.error = (err as Error).message;
      this._breaker.recordFailure(name);
      this.onPluginError?.(instance, err as Error);
      throw err;
    }
  }

  /**
   * Stop a running plugin.
   */
  async stop(name: string): Promise<void> {
    const instance = this._plugins.get(name);
    if (!instance) return;

    await shutdownPlugin(instance);
    this.onPluginStopped?.(instance);
  }

  /**
   * Restart a plugin.
   */
  async restart(name: string): Promise<void> {
    await this.stop(name);

    const instance = this._plugins.get(name);
    if (instance) {
      instance.restartCount++;
    }

    await this.start(name);
  }

  // -------------------------------------------------------------------------
  // Status
  // -------------------------------------------------------------------------

  /**
   * Get a plugin instance.
   */
  get(name: string): PluginInstance | undefined {
    return this._plugins.get(name);
  }

  /**
   * List all loaded plugins.
   */
  list(): PluginInstance[] {
    return [...this._plugins.values()];
  }

  /**
   * Perform a health check on a specific plugin.
   */
  async healthCheck(name: string): Promise<HealthStatus> {
    const instance = this._plugins.get(name);
    if (!instance) {
      return {
        name,
        alive: false,
        uptimeMs: 0,
        lastError: "plugin not found",
        toolCount: 0,
      };
    }

    const status = await healthCheck(instance);

    if (status.alive) {
      this._breaker.recordSuccess(name);
    } else {
      if (this._breaker.isTripped(name)) {
        // Circuit breaker tripped — try restart
        this.restart(name).catch(() => {
          // Restart failure is logged via onPluginError
        });
      }
    }

    return status;
  }

  /**
   * Start health monitoring for all running plugins.
   */
  startHealthMonitor(): void {
    if (this._healthTimer) return;

    this._healthTimer = setInterval(async () => {
      for (const instance of this._plugins.values()) {
        if (instance.status === PluginStatus.Running) {
          try {
            await this.healthCheck(instance.manifest.name);
          } catch (err) {
            this.onPluginError?.(instance, err as Error);
          }
        }
      }
    }, this._config.healthCheckIntervalMs);
  }

  /**
   * Stop health monitoring.
   */
  stopHealthMonitor(): void {
    if (this._healthTimer) {
      clearInterval(this._healthTimer);
      this._healthTimer = null;
    }
  }

  // -------------------------------------------------------------------------
  // Cleanup
  // -------------------------------------------------------------------------

  /**
   * Shutdown all plugins.
   */
  async shutdownAll(): Promise<void> {
    this.stopHealthMonitor();

    const shutdowns: Promise<void>[] = [];
    for (const instance of this._plugins.values()) {
      if (instance.status === PluginStatus.Running || instance.status === PluginStatus.Starting) {
        shutdowns.push(shutdownPlugin(instance).catch(() => {
          // Ignore shutdown errors
        }));
      }
    }

    await Promise.all(shutdowns);
    this._plugins.clear();
  }
}
