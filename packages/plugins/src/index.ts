/**
 * @orangecoding/plugins — Plugin runtime manager.
 *
 * Re-exports all public API from the package.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
export {
  PluginStatus,
  DEFAULT_PLUGIN_MANAGER_CONFIG,
  PluginError,
  newPluginError,
} from "./types.js";
export type {
  PluginManifest,
  PluginInstance,
  PluginStatus as PluginStatusType,
  HealthStatus,
  PluginManagerConfig,
} from "./types.js";

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------
export { discoverPlugins, resolvePluginDependencies } from "./discovery.js";

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------
export { loadPlugin, initializePlugin, shutdownPlugin } from "./loader.js";

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------
export { healthCheck, CircuitBreaker } from "./health.js";
export type { CircuitBreakerState } from "./health.js";

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------
export { PluginManager } from "./manager.js";
