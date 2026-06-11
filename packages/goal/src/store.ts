/**
 * Goal state store — persists goal state to disk.
 */

import { readFile, writeFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import type { GoalState } from "./types.js";

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export interface GoalStore {
  load(goalId: string): Promise<GoalState | null>;
  save(state: GoalState): Promise<void>;
  delete(goalId: string): Promise<void>;
  list(): Promise<string[]>;
}

// ---------------------------------------------------------------------------
// File-backed store
// ---------------------------------------------------------------------------

export class FileGoalStore implements GoalStore {
  private readonly _dir: string;

  constructor(dir: string) {
    this._dir = dir;
  }

  private _path(goalId: string): string {
    return join(this._dir, `${goalId}.json`);
  }

  async load(goalId: string): Promise<GoalState | null> {
    try {
      const raw = await readFile(this._path(goalId), "utf-8");
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      return {
        id: parsed.id as string,
        config: parsed.config as GoalState["config"],
        status: parsed.status as GoalState["status"],
        iteration: (parsed.iteration as number) ?? 0,
        totalTokensUsed: (parsed.totalTokensUsed as number) ?? 0,
        lastEvalResult: parsed.lastEvalResult ? (parsed.lastEvalResult as GoalState["lastEvalResult"]) : null,
        createdAt: new Date(parsed.createdAt as string),
        updatedAt: new Date(parsed.updatedAt as string),
      };
    } catch {
      return null;
    }
  }

  async save(state: GoalState): Promise<void> {
    await mkdir(this._dir, { recursive: true });
    await writeFile(this._path(state.id), JSON.stringify(state, null, 2), "utf-8");
  }

  async delete(goalId: string): Promise<void> {
    const { rm } = await import("node:fs/promises");
    try {
      await rm(this._path(goalId));
    } catch {
      // Ignore if file doesn't exist
    }
  }

  async list(): Promise<string[]> {
    const { readdir } = await import("node:fs/promises");
    try {
      const files = await readdir(this._dir);
      return files
        .filter((f) => f.endsWith(".json"))
        .map((f) => f.replace(/\.json$/, ""));
    } catch {
      return [];
    }
  }
}

// ---------------------------------------------------------------------------
// In-memory store (for testing)
// ---------------------------------------------------------------------------

export class MemoryGoalStore implements GoalStore {
  private readonly _states: Map<string, GoalState> = new Map();

  async load(goalId: string): Promise<GoalState | null> {
    return this._states.get(goalId) ?? null;
  }

  async save(state: GoalState): Promise<void> {
    this._states.set(state.id, { ...state });
  }

  async delete(goalId: string): Promise<void> {
    this._states.delete(goalId);
  }

  async list(): Promise<string[]> {
    return [...this._states.keys()];
  }
}
