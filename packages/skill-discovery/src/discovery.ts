/**
 * Skill file discovery — scans directories for SKILL.md files.
 */

import { readdir, readFile, stat } from "node:fs/promises";
import { join, extname } from "node:path";
import { parseSkillMd } from "./parser.js";
import type { SkillFile, SkillDiscoveryConfig } from "./types.js";
import { DEFAULT_DISCOVERY_CONFIG } from "./types.js";

// ---------------------------------------------------------------------------
// Discoverer Class
// ---------------------------------------------------------------------------

export class SkillDiscoverer {
  private readonly _config: Required<SkillDiscoveryConfig>;

  constructor(config?: Partial<SkillDiscoveryConfig>) {
    this._config = {
      searchDirs: config?.searchDirs ?? DEFAULT_DISCOVERY_CONFIG.searchDirs,
      pattern: config?.pattern ?? DEFAULT_DISCOVERY_CONFIG.pattern,
    };
  }

  /**
   * Discover all skill files across configured search directories.
   */
  async discover(): Promise<SkillFile[]> {
    const results: SkillFile[] = [];

    for (const dir of this._config.searchDirs) {
      const files = await _scanDir(dir, this._config.pattern);
      for (const filePath of files) {
        const skill = await this.discoverOne(filePath);
        if (skill) {
          results.push(skill);
        }
      }
    }

    return results;
  }

  /**
   * Parse a single skill file.
   */
  async discoverOne(path: string): Promise<SkillFile | null> {
    try {
      const content = await readFile(path, "utf-8");
      return parseSkillMd(content, path);
    } catch {
      return null;
    }
  }

  /**
   * Watch for file changes in search directories.
   * Returns a dispose function to stop watching.
   */
  watch(callback: (skills: SkillFile[]) => void): () => void {
    // Polling-based watch — simple and reliable
    let disposed = false;
    let timeoutId: NodeJS.Timeout | null = null;

    const poll = async (): Promise<void> => {
      if (disposed) return;

      try {
        const skills = await this.discover();
        callback(skills);
      } catch {
        // Ignore discovery errors during watch
      }

      if (!disposed) {
        timeoutId = setTimeout(poll, 5_000); // poll every 5 seconds
      }
    };

    poll();

    return () => {
      disposed = true;
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    };
  }
}

// ---------------------------------------------------------------------------
// Internal Helpers
// ---------------------------------------------------------------------------

/**
 * Recursively scan a directory for files matching a pattern.
 */
async function _scanDir(dir: string, pattern: string): Promise<string[]> {
  const results: string[] = [];

  // Map pattern to extension filter
  const ext = pattern.includes("*.md") ? ".md" : null;

  async function walk(current: string): Promise<void> {
    let entries;
    try {
      entries = await readdir(current, { withFileTypes: true });
    } catch {
      return; // Skip unreadable directories
    }

    for (const entry of entries) {
      const fullPath = join(current, entry.name);

      if (entry.isDirectory()) {
        await walk(fullPath);
      } else if (entry.isFile()) {
        if (ext === null || extname(entry.name) === ext) {
          results.push(fullPath);
        }
      }
    }
  }

  await walk(dir);
  return results;
}
