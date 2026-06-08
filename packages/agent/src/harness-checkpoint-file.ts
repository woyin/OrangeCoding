/**
 * FileCheckpointStore persists harness checkpoints as JSON files.
 * Ported from modules/agent/harness_checkpoint_file.go.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import type { HarnessCheckpoint, CheckpointSummary } from "./harness-state.js";
import { checkpointSummary, cloneHarnessCheckpoint } from "./harness-state.js";
import type { CheckpointStore } from "./harness-state.js";

export class FileCheckpointStore implements CheckpointStore {
  private _dir: string;
  private _ttlMs: number; // 0 = no TTL

  constructor(dir: string, ttlMs?: number) {
    this._dir = dir;
    this._ttlMs = ttlMs ?? 0;
  }

  /** Save writes a checkpoint atomically using write-to-temp + rename. */
  async save(_signal: AbortSignal | undefined, cp: HarnessCheckpoint): Promise<void> {
    if (!cp.runID) throw new Error("file checkpoint store: run id is required");
    await fs.promises.mkdir(this._dir, { recursive: true });

    const cloned = cloneHarnessCheckpoint(cp);
    cloned.updatedAt = new Date();
    const data = JSON.stringify(cloned, null, 2);

    // Atomic write via temp file + rename
    const tmpPath = this.pathFor(cp.runID) + ".tmp";
    await fs.promises.writeFile(tmpPath, data, "utf-8");
    try {
      await fs.promises.rename(tmpPath, this.pathFor(cp.runID));
    } catch (err) {
      await fs.promises.unlink(tmpPath).catch(() => {});
      throw new Error(`file checkpoint store: rename: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  /** Load reads a checkpoint by run ID. */
  async load(_signal: AbortSignal | undefined, runID: string): Promise<HarnessCheckpoint> {
    try {
      const data = await fs.promises.readFile(this.pathFor(runID), "utf-8");
      const cp = JSON.parse(data) as HarnessCheckpoint;
      return cloneHarnessCheckpoint(cp);
    } catch (err) {
      throw new Error(`file checkpoint store: read: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  /** List returns summaries for checkpoints matching the given prefix. */
  async list(_signal: AbortSignal | undefined, prefix: string): Promise<CheckpointSummary[]> {
    let entries: fs.Dirent[];
    try {
      entries = await fs.promises.readdir(this._dir, { withFileTypes: true });
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw new Error(`file checkpoint store: read dir: ${err instanceof Error ? err.message : String(err)}`);
    }

    const summaries: CheckpointSummary[] = [];
    for (const entry of entries) {
      if (entry.isDirectory() || !entry.name.endsWith(".json")) continue;
      const runID = entry.name.slice(0, -5); // remove .json
      if (prefix && !runID.startsWith(prefix)) continue;

      try {
        const data = await fs.promises.readFile(this.pathFor(runID), "utf-8");
        const cp = JSON.parse(data) as HarnessCheckpoint;

        // TTL check
        if (this._ttlMs > 0 && Date.now() - cp.updatedAt.getTime() > this._ttlMs) {
          await this.delete(undefined, runID).catch(() => {});
          continue;
        }

        summaries.push(checkpointSummary(cp));
      } catch {
        continue;
      }
    }

    // Sort by UpdatedAt descending (most recent first)
    summaries.sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime());
    return summaries;
  }

  /** Delete removes a checkpoint file by run ID. */
  async delete(_signal: AbortSignal | undefined, runID: string): Promise<void> {
    const filePath = this.pathFor(runID);
    try {
      await fs.promises.unlink(filePath);
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== "ENOENT") {
        throw new Error(`file checkpoint store: delete: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
  }

  /** CleanupExpired removes all checkpoints older than the configured TTL. */
  async cleanupExpired(signal: AbortSignal | undefined): Promise<number> {
    if (this._ttlMs <= 0) return 0;
    let entries: fs.Dirent[];
    try {
      entries = await fs.promises.readdir(this._dir, { withFileTypes: true });
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return 0;
      throw err;
    }
    let cleaned = 0;
    for (const entry of entries) {
      if (entry.isDirectory() || !entry.name.endsWith(".json")) continue;
      const runID = entry.name.slice(0, -5);
      try {
        const data = await fs.promises.readFile(this.pathFor(runID), "utf-8");
        const cp = JSON.parse(data) as HarnessCheckpoint;
        if (Date.now() - cp.updatedAt.getTime() > this._ttlMs) {
          await this.delete(signal, runID);
          cleaned++;
        }
      } catch {
        continue;
      }
    }
    return cleaned;
  }

  private pathFor(runID: string): string {
    return path.join(this._dir, `${runID}.json`);
  }
}
