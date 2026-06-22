/**
 * Tests for the plugins package — CircuitBreaker, plugin dependency resolution,
 * PluginManager lifecycle, and types.
 */

import { CircuitBreaker } from "../health.js";
import { resolvePluginDependencies } from "../discovery.js";
import { PluginManager } from "../manager.js";
import { PluginStatus, PluginError, newPluginError, DEFAULT_PLUGIN_MANAGER_CONFIG } from "../types.js";
import type { PluginManifest } from "../types.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeManifest(overrides: Partial<PluginManifest> = {}): PluginManifest {
  return {
    name: "test-plugin",
    version: "1.0.0",
    description: "A test plugin",
    main: "./dist/index.js",
    tools: ["test-tool"],
    permissions: [],
    dependencies: {},
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// CircuitBreaker
// ---------------------------------------------------------------------------

describe("CircuitBreaker", () => {
  it("starts with no failures and not tripped", () => {
    const cb = new CircuitBreaker(3, 60_000);
    expect(cb.isTripped("plugin-a")).toBe(false);
  });

  it("trips after maxFailures consecutive failures", () => {
    const cb = new CircuitBreaker(3, 60_000);
    cb.recordFailure("plugin-a");
    expect(cb.isTripped("plugin-a")).toBe(false);
    cb.recordFailure("plugin-a");
    expect(cb.isTripped("plugin-a")).toBe(false);
    cb.recordFailure("plugin-a");
    expect(cb.isTripped("plugin-a")).toBe(true);
  });

  it("recordFailure returns true when circuit trips", () => {
    const cb = new CircuitBreaker(2, 60_000);
    expect(cb.recordFailure("p")).toBe(false);
    expect(cb.recordFailure("p")).toBe(true);
  });

  it("recordSuccess resets failure count", () => {
    const cb = new CircuitBreaker(3, 60_000);
    cb.recordFailure("p");
    cb.recordFailure("p");
    cb.recordSuccess("p");
    expect(cb.isTripped("p")).toBe(false);

    // Can fail again without tripping immediately
    cb.recordFailure("p");
    cb.recordFailure("p");
    expect(cb.isTripped("p")).toBe(false);
  });

  it("auto-resets after the window expires", () => {
    const cb = new CircuitBreaker(2, 100); // 100ms window
    cb.recordFailure("p");
    cb.recordFailure("p");
    expect(cb.isTripped("p")).toBe(true);

    // Wait for window to expire
    return new Promise<void>((resolve) => {
      setTimeout(() => {
        expect(cb.isTripped("p")).toBe(false);
        resolve();
      }, 150);
    });
  });

  it("resets all state", () => {
    const cb = new CircuitBreaker(1, 60_000);
    cb.recordFailure("a");
    cb.recordFailure("b");
    expect(cb.isTripped("a")).toBe(true);
    expect(cb.isTripped("b")).toBe(true);

    cb.reset();
    expect(cb.isTripped("a")).toBe(false);
    expect(cb.isTripped("b")).toBe(false);
  });

  it("handles different plugins independently", () => {
    const cb = new CircuitBreaker(2, 60_000);
    cb.recordFailure("a");
    cb.recordFailure("a");
    cb.recordFailure("b");

    expect(cb.isTripped("a")).toBe(true);
    expect(cb.isTripped("b")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// resolvePluginDependencies — topological sort
// ---------------------------------------------------------------------------

describe("resolvePluginDependencies", () => {
  it("returns plugins in dependency order", () => {
    const plugins = [
      makeManifest({ name: "c", dependencies: { a: "^1.0.0" } }),
      makeManifest({ name: "a", dependencies: {} }),
      makeManifest({ name: "b", dependencies: { a: "^1.0.0" } }),
    ];

    const result = resolvePluginDependencies(plugins);
    const names = result.map((p) => p.name);

    // "a" must come before "b" and "c"
    expect(names.indexOf("a")).toBeLessThan(names.indexOf("b"));
    expect(names.indexOf("a")).toBeLessThan(names.indexOf("c"));
  });

  it("handles plugins with no dependencies", () => {
    const plugins = [
      makeManifest({ name: "a" }),
      makeManifest({ name: "b" }),
      makeManifest({ name: "c" }),
    ];

    const result = resolvePluginDependencies(plugins);
    expect(result).toHaveLength(3);
  });

  it("handles chain dependencies: a -> b -> c", () => {
    const plugins = [
      makeManifest({ name: "c", dependencies: { b: "^1.0.0" } }),
      makeManifest({ name: "b", dependencies: { a: "^1.0.0" } }),
      makeManifest({ name: "a", dependencies: {} }),
    ];

    const result = resolvePluginDependencies(plugins);
    const names = result.map((p) => p.name);

    expect(names.indexOf("a")).toBeLessThan(names.indexOf("b"));
    expect(names.indexOf("b")).toBeLessThan(names.indexOf("c"));
  });

  it("throws on circular dependencies", () => {
    const plugins = [
      makeManifest({ name: "a", dependencies: { b: "^1.0.0" } }),
      makeManifest({ name: "b", dependencies: { a: "^1.0.0" } }),
    ];

    expect(() => resolvePluginDependencies(plugins)).toThrow("circular");
  });

  it("throws on missing dependency", () => {
    const plugins = [
      makeManifest({ name: "a", dependencies: { missing: "^1.0.0" } }),
    ];

    expect(() => resolvePluginDependencies(plugins)).toThrow("not found");
  });

  it("returns empty array for empty input", () => {
    expect(resolvePluginDependencies([])).toEqual([]);
  });

  it("deduplicates plugins visited via multiple paths", () => {
    const plugins = [
      makeManifest({ name: "a" }),
      makeManifest({ name: "b", dependencies: { a: "^1.0.0" } }),
      makeManifest({ name: "c", dependencies: { a: "^1.0.0", b: "^1.0.0" } }),
    ];

    const result = resolvePluginDependencies(plugins);
    // "a" should appear exactly once
    expect(result.filter((p) => p.name === "a")).toHaveLength(1);
    expect(result).toHaveLength(3);
  });
});

// ---------------------------------------------------------------------------
// PluginManager — lifecycle (without real plugin processes)
// ---------------------------------------------------------------------------

describe("PluginManager — lifecycle", () => {
  it("loads a plugin from manifest", async () => {
    const manager = new PluginManager();
    const manifest = makeManifest({ name: "my-plugin" });

    const instance = await manager.load(manifest);
    expect(instance.status).toBe(PluginStatus.Loaded);
    expect(instance.manifest.name).toBe("my-plugin");
  });

  it("throws when loading a duplicate plugin", async () => {
    const manager = new PluginManager();
    const manifest = makeManifest({ name: "dup" });

    await manager.load(manifest);
    await expect(manager.load(manifest)).rejects.toThrow("already loaded");
  });

  it("gets a loaded plugin by name", async () => {
    const manager = new PluginManager();
    await manager.load(makeManifest({ name: "p1" }));

    const instance = manager.get("p1");
    expect(instance).toBeDefined();
    expect(instance!.manifest.name).toBe("p1");
  });

  it("returns undefined for non-existent plugin", () => {
    const manager = new PluginManager();
    expect(manager.get("nope")).toBeUndefined();
  });

  it("lists all loaded plugins", async () => {
    const manager = new PluginManager();
    await manager.load(makeManifest({ name: "p1" }));
    await manager.load(makeManifest({ name: "p2" }));
    await manager.load(makeManifest({ name: "p3" }));

    expect(manager.list()).toHaveLength(3);
  });

  it("calls onPluginLoaded callback", async () => {
    const manager = new PluginManager();
    let loadedName = "";
    manager.onPluginLoaded = (inst) => { loadedName = inst.manifest.name; };

    await manager.load(makeManifest({ name: "cb-test" }));
    expect(loadedName).toBe("cb-test");
  });

  it("healthCheck returns not alive for non-existent plugin", async () => {
    const manager = new PluginManager();
    const status = await manager.healthCheck("nope");
    expect(status.alive).toBe(false);
    expect(status.lastError).toBe("plugin not found");
  });

  it("healthCheck returns not alive for loaded (not running) plugin", async () => {
    const manager = new PluginManager();
    await manager.load(makeManifest({ name: "p1" }));

    const status = await manager.healthCheck("p1");
    expect(status.alive).toBe(false);
  });

  it("startHealthMonitor and stopHealthMonitor do not throw", () => {
    const manager = new PluginManager({ healthCheckIntervalMs: 100 });
    manager.startHealthMonitor();
    manager.stopHealthMonitor();
  });

  it("shutdownAll clears all plugins", async () => {
    const manager = new PluginManager();
    await manager.load(makeManifest({ name: "p1" }));
    await manager.load(makeManifest({ name: "p2" }));

    await manager.shutdownAll();
    expect(manager.list()).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// Types — error constructors and constants
// ---------------------------------------------------------------------------

describe("Plugin types", () => {
  it("PluginStatus has all expected values", () => {
    expect(PluginStatus.Loaded).toBe("loaded");
    expect(PluginStatus.Starting).toBe("starting");
    expect(PluginStatus.Running).toBe("running");
    expect(PluginStatus.Stopping).toBe("stopping");
    expect(PluginStatus.Stopped).toBe("stopped");
    expect(PluginStatus.Error).toBe("error");
  });

  it("newPluginError creates a PluginError with code and pluginName", () => {
    const err = newPluginError("TEST", "test message", "my-plugin");
    expect(err).toBeInstanceOf(PluginError);
    expect(err.code).toBe("TEST");
    expect(err.pluginName).toBe("my-plugin");
    expect(err.message).toBe("test message");
    expect(err.name).toBe("PluginError");
  });

  it("DEFAULT_PLUGIN_MANAGER_CONFIG has sensible defaults", () => {
    expect(DEFAULT_PLUGIN_MANAGER_CONFIG.searchDirs).toEqual([".claude/plugins"]);
    expect(DEFAULT_PLUGIN_MANAGER_CONFIG.healthCheckIntervalMs).toBe(30_000);
    expect(DEFAULT_PLUGIN_MANAGER_CONFIG.maxRestarts).toBe(3);
    expect(DEFAULT_PLUGIN_MANAGER_CONFIG.restartWindowMs).toBe(60_000);
    expect(DEFAULT_PLUGIN_MANAGER_CONFIG.startTimeoutMs).toBe(30_000);
  });
});
