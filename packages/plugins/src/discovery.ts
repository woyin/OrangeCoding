/**
 * Plugin discovery — scans directories for plugin manifests.
 */

import { readdir, readFile, stat } from "node:fs/promises";
import { join, extname } from "node:path";

import type { PluginManifest } from "./types.js";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MANIFEST_FILE = "plugin.json";

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/**
 * Discover all plugins in a directory.
 *
 * Scans subdirectories for plugin.json files.
 */
export async function discoverPlugins(searchDir: string): Promise<PluginManifest[]> {
  const manifests: PluginManifest[] = [];

  let entries: string[];
  try {
    entries = await readdir(searchDir);
  } catch {
    return [];
  }

  for (const entry of entries) {
    const pluginDir = join(searchDir, entry);
    const manifestPath = join(pluginDir, MANIFEST_FILE);

    let entryStat;
    try {
      entryStat = await stat(pluginDir);
    } catch {
      continue;
    }

    if (!entryStat.isDirectory()) continue;

    try {
      const raw = await readFile(manifestPath, "utf-8");
      const manifest: PluginManifest = JSON.parse(raw);
      manifests.push(manifest);
    } catch {
      continue; // Skip invalid plugin directories
    }
  }

  return manifests;
}

/**
 * Resolve plugin dependencies (topological sort).
 *
 * Returns plugins in dependency order (dependencies before dependents).
 */
export function resolvePluginDependencies(plugins: PluginManifest[]): PluginManifest[] {
  const graph = new Map<string, PluginManifest>();
  const visited = new Set<string>();
  const inProgress = new Set<string>();
  const result: PluginManifest[] = [];

  for (const p of plugins) {
    graph.set(p.name, p);
  }

  function visit(name: string): void {
    if (visited.has(name)) return;
    if (inProgress.has(name)) {
      throw new Error(`circular plugin dependency detected: ${name}`);
    }

    const plugin = graph.get(name);
    if (!plugin) {
      throw new Error(`plugin dependency not found: ${name}`);
    }

    inProgress.add(name);

    for (const depName of Object.keys(plugin.dependencies)) {
      visit(depName);
    }

    inProgress.delete(name);
    visited.add(name);
    result.push(plugin);
  }

  for (const plugin of plugins) {
    if (!visited.has(plugin.name)) {
      visit(plugin.name);
    }
  }

  return result;
}
