/**
 * Scheduler — cron-based job scheduling with persistent state.
 *
 * The heartbeat of loop engineering: automatically run tasks on a schedule.
 */

import { randomUUID } from "node:crypto";
import { parseCron, nextCronMatch, matchCron } from "./cron.js";
import {
  JobConfig,
  JobState,
  JobStatus,
  backoffDelay,
  getRetryDelayMs,
  getTimeoutMs,
  shouldRetry,
} from "./job.js";
import { FileJobStore, MemoryJobStore } from "./store.js";
import type { JobStore } from "./store.js";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export interface SchedulerConfig {
  /** How often to check for due jobs (default: 60_000 = 1 minute) */
  tickIntervalMs?: number;
  /** Maximum concurrent running jobs (default: 5) */
  maxConcurrentJobs?: number;
  /** Job store implementation (default: FileJobStore) */
  store?: JobStore;
}

const DEFAULT_CONFIG: Required<Pick<SchedulerConfig, "tickIntervalMs" | "maxConcurrentJobs">> = {
  tickIntervalMs: 60_000,
  maxConcurrentJobs: 5,
};

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

export type JobCallback = (job: JobState) => Promise<string>;

export interface SchedulerCallbacks {
  /** Called when a job starts executing. Return the result string. */
  onJobStart?: JobCallback;
  /** Called when a job completes successfully */
  onJobComplete?: (job: JobState, result: string) => void;
  /** Called when a job fails */
  onJobFail?: (job: JobState, error: Error) => void;
}

// ---------------------------------------------------------------------------
// Scheduler Class
// ---------------------------------------------------------------------------

export class Scheduler {
  private readonly _config: Required<Pick<SchedulerConfig, "tickIntervalMs" | "maxConcurrentJobs">>;
  private readonly _store: JobStore;
  private readonly _callbacks: SchedulerCallbacks;
  private _jobs: Map<string, JobState> = new Map();
  private _tickTimer: NodeJS.Timeout | null = null;
  private _runningJobs: Set<string> = new Set();

  constructor(config?: SchedulerConfig, callbacks?: SchedulerCallbacks) {
    this._config = {
      tickIntervalMs: config?.tickIntervalMs ?? DEFAULT_CONFIG.tickIntervalMs,
      maxConcurrentJobs: config?.maxConcurrentJobs ?? DEFAULT_CONFIG.maxConcurrentJobs,
    };
    this._store = config?.store ?? new FileJobStore();
    this._callbacks = callbacks ?? {};
  }

  // -------------------------------------------------------------------------
  // Lifecycle
  // -------------------------------------------------------------------------

  /**
   * Load jobs from store and start the tick loop.
   */
  async start(signal?: AbortSignal): Promise<void> {
    this._jobs = await this._store.load();

    // Check for aborted signal before starting timer
    if (signal?.aborted) {
      return;
    }

    this._tickTimer = setInterval(() => {
      this._tick().catch((err) => {
        console.error("scheduler tick error:", err);
      });
    }, this._config.tickIntervalMs);

    // Stop on signal
    signal?.addEventListener("abort", () => {
      this.stop();
    }, { once: true });
  }

  /**
   * Stop the scheduler and persist final state.
   */
  async stop(): Promise<void> {
    if (this._tickTimer) {
      clearInterval(this._tickTimer);
      this._tickTimer = null;
    }
    await this._store.save(this._jobs);
  }

  // -------------------------------------------------------------------------
  // Job Management
  // -------------------------------------------------------------------------

  /**
   * Create a new scheduled job.
   */
  create(config: JobConfig): JobState {
    const id = randomUUID();
    const cronExpr = parseCron(config.cron);
    const nextRun = nextCronMatch(cronExpr);

    const job: JobState = {
      id,
      config,
      status: JobStatus.Pending,
      lastRunAt: null,
      nextRunAt: nextRun,
      runCount: 0,
      retryCount: 0,
      lastResult: null,
      lastError: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    };

    this._jobs.set(id, job);

    if (config.durable) {
      this._store.save(this._jobs).catch((err) => {
        console.error("failed to persist job:", err);
      });
    }

    return job;
  }

  /**
   * Get a job by ID.
   */
  get(id: string): JobState | undefined {
    return this._jobs.get(id);
  }

  /**
   * List all jobs, optionally filtered by status.
   */
  list(status?: JobStatus): JobState[] {
    const jobs = [...this._jobs.values()];
    return status ? jobs.filter((j) => j.status === status) : jobs;
  }

  /**
   * Pause a job (won't run until resumed).
   */
  pause(id: string): void {
    const job = this._jobs.get(id);
    if (!job) return;

    job.status = JobStatus.Paused;
    job.updatedAt = new Date();

    if (job.config.durable) {
      this._store.save(this._jobs).catch((err) => {
        console.error("failed to persist job:", err);
      });
    }
  }

  /**
   * Resume a paused job.
   */
  resume(id: string): void {
    const job = this._jobs.get(id);
    if (!job || job.status !== JobStatus.Paused) return;

    job.status = JobStatus.Pending;
    const cronExpr = parseCron(job.config.cron);
    job.nextRunAt = nextCronMatch(cronExpr);
    job.updatedAt = new Date();

    if (job.config.durable) {
      this._store.save(this._jobs).catch((err) => {
        console.error("failed to persist job:", err);
      });
    }
  }

  /**
   * Cancel and remove a job.
   */
  cancel(id: string): void {
    this._jobs.delete(id);
    this._store.save(this._jobs).catch((err) => {
      console.error("failed to persist jobs:", err);
    });
  }

  // -------------------------------------------------------------------------
  // Internal
  // -------------------------------------------------------------------------

  /**
   * Tick: check for due jobs and execute them.
   */
  private async _tick(): Promise<void> {
    const now = new Date();
    const dueJobs: JobState[] = [];

    for (const job of this._jobs.values()) {
      if (job.status !== JobStatus.Pending) continue;
      if (!job.nextRunAt) continue;
      if (job.nextRunAt > now) continue;
      if (this._runningJobs.size >= this._config.maxConcurrentJobs) break;

      dueJobs.push(job);
    }

    for (const job of dueJobs) {
      await this._executeJob(job);
    }
  }

  /**
   * Execute a single job.
   */
  private async _executeJob(job: JobState): Promise<void> {
    const timeout = getTimeoutMs(job.config);
    const abortController = new AbortController();
    const timeoutId = setTimeout(() => abortController.abort(), timeout);

    this._runningJobs.add(job.id);
    job.status = JobStatus.Running;
    job.lastRunAt = new Date();
    job.updatedAt = new Date();

    try {
      if (!this._callbacks.onJobStart) {
        throw new Error("no onJobStart callback provided");
      }

      const result = await this._callbacks.onJobStart(job);

      job.status = JobStatus.Completed;
      job.lastResult = result;
      job.lastError = null;
      job.runCount++;
      job.retryCount = 0;

      this._callbacks.onJobComplete?.(job, result);
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));

      job.status = JobStatus.Failed;
      job.lastError = error.message;
      job.retryCount++;

      this._callbacks.onJobFail?.(job, error);

      // Check if we should retry
      if (shouldRetry(job)) {
        const delay = backoffDelay(getRetryDelayMs(job.config), job.retryCount);
        job.nextRunAt = new Date(Date.now() + delay);
        job.status = JobStatus.Pending;
      }
    } finally {
      clearTimeout(timeoutId);
      this._runningJobs.delete(job.id);
      job.updatedAt = new Date();

      // Calculate next run time
      const cronExpr = parseCron(job.config.cron);
      job.nextRunAt = nextCronMatch(cronExpr);

      if (job.config.durable) {
        await this._store.save(this._jobs).catch((err) => {
          console.error("failed to persist job:", err);
        });
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------

export { FileJobStore, MemoryJobStore } from "./store.js";
export type { JobStore } from "./store.js";
