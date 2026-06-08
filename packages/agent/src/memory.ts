/**
 * MemoryStore provides file-backed key-value storage for agent memory.
 * Ported from modules/agent/memory.go.
 */

import * as fs from "node:fs";
import * as path from "node:path";

export class MemoryStore {
  private _dir: string;

  constructor(dir: string) {
    this._dir = dir;
  }

  /** Write stores the value under the given key. */
  async write(key: string, value: string): Promise<void> {
    await fs.promises.mkdir(this._dir, { recursive: true });
    const filePath = this.keyPath(key);
    await fs.promises.writeFile(filePath, value, "utf-8");
  }

  /** Read retrieves the value for the given key. */
  async read(key: string): Promise<string> {
    try {
      return await fs.promises.readFile(this.keyPath(key), "utf-8");
    } catch (err) {
      throw new Error(`memory store: read: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  /** List returns all stored keys (filenames without the .txt extension). */
  async list(): Promise<string[]> {
    try {
      const entries = await fs.promises.readdir(this._dir);
      return entries
        .filter((e) => !e.startsWith(".") && e.endsWith(".txt"))
        .map((e) => e.slice(0, -4));
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") {
        return [];
      }
      throw new Error(`memory store: list: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  /** Recall returns keys whose names contain the query substring. */
  async recall(query: string): Promise<string[]> {
    const keys = await this.list();
    const lower = query.toLowerCase();
    return keys.filter((k) => k.toLowerCase().includes(lower));
  }

  /** keyPath returns the full file path for a given key, sanitized against path traversal. */
  private keyPath(key: string): string {
    let clean = path.basename(key);
    if (clean === "." || clean === "..") {
      clean = "invalid";
    }
    return path.join(this._dir, `${clean}.txt`);
  }
}
