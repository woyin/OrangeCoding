/**
 * Persistent job state store using JSON file on disk.
 *
 * Stores jobs in a JSON array file (e.g., .claude/scheduled_tasks.json).
 */

import { readFile, writeFile, mkdir, access } from "node:fs/promises";
import { join, dirname } from "node:path";
import type { JobState } from "./job.js";

// ---------------------------------------------------------------------------
// Store Interface
// ---------------------------------------------------------------------------

export interface JobStore {
  load(): Promise<Map<string, JobState>>;
  save(jobs: Map<string, JobState>): Promise<void>;
}

// ---------------------------------------------------------------------------
// File-backed implementation
// ---------------------------------------------------------------------------

export interface FileJobStoreConfig {
  /** Path to the JSON state file */
  filePath: string;
}

export class FileJobStore implements JobStore {
  private readonly _filePath: string;

  constructor(config?: Partial<FileJobStoreConfig>) {
    this._filePath = config?.filePath ?? ".claude/scheduled_tasks.json";
  }

  async load(): Promise<Map<string, JobState>> {
    try {
      await access(this._filePath);
      const raw = await readFile(this._filePath, "utf-8");
      const parsed: unknown = JSON.parse(raw);

      if (!Array.isArray(parsed)) {
        return new Map();
      }

      const jobs = new Map<string, JobState>();
      for (const entry of parsed) {
        const job = entry as Record<string, unknown>;
        // Rehydrate Date fields
        if (typeof job.id === "string") {
          jobs.set(job.id, {
            id: job.id as string,
            config: job.config as JobState["config"],
            status: job.status as JobState["status"],
            lastRunAt: job.lastRunAt ? new Date(job.lastRunAt as string) : null,
            nextRunAt: job.nextRunAt ? new Date(job.nextRunAt as string) : null,
            runCount: (job.runCount as number) ?? 0,
            retryCount: (job.retryCount as number) ?? 0,
            lastResult: (job.lastResult as string) ?? null,
            lastError: (job.lastError as string) ?? null,
            createdAt: new Date(job.createdAt as string),
            updatedAt: new Date(job.updatedAt as string),
          });
        }
      }
      return jobs;
    } catch {
      return new Map();
    }
  }

  async save(jobs: Map<string, JobState>): Promise<void> {
    await mkdir(dirname(this._filePath), { recursive: true });
    const data = JSON.stringify([...jobs.values()], null, 2);
    await writeFile(this._filePath, data, "utf-8");
  }
}

// ---------------------------------------------------------------------------
// In-memory store (for testing)
// ---------------------------------------------------------------------------

export class MemoryJobStore implements JobStore {
  private _jobs: Map<string, JobState> = new Map();

  async load(): Promise<Map<string, JobState>> {
    return new Map(this._jobs);
  }

  async save(jobs: Map<string, JobState>): Promise<void> {
    this._jobs = new Map(jobs);
  }

  /** Direct access for test assertions */
  get jobs(): Map<string, JobState> {
    return new Map(this._jobs);
  }
}
