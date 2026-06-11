/**
 * Job definition, state, and status types.
 */

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

export const JobStatus = {
  Pending: "pending",
  Running: "running",
  Completed: "completed",
  Failed: "failed",
  Paused: "paused",
} as const;

export type JobStatus = (typeof JobStatus)[keyof typeof JobStatus];

// ---------------------------------------------------------------------------
// Config & State
// ---------------------------------------------------------------------------

export interface JobConfig {
  /** Human-readable name for the job */
  name: string;
  /** Prompt to execute when the job fires */
  prompt: string;
  /** Cron expression in standard 5-field format */
  cron: string;
  /** Maximum number of retries on failure (default: 3) */
  maxRetries?: number;
  /** Base retry delay in ms for exponential backoff (default: 60_000) */
  retryDelayMs?: number;
  /** Timeout per job run in ms (default: 300_000 = 5 min) */
  timeoutMs?: number;
  /** Whether to persist job state to disk (default: false) */
  durable?: boolean;
  /** Whether to run in an isolated git worktree (default: false) */
  worktree?: boolean;
}

export interface JobState {
  /** Unique job ID (uuid) */
  id: string;
  /** Job configuration */
  config: JobConfig;
  /** Current status */
  status: JobStatus;
  /** When the job last started running (null if never run) */
  lastRunAt: Date | null;
  /** When the job is scheduled to run next */
  nextRunAt: Date | null;
  /** Total number of runs */
  runCount: number;
  /** Retry count for the current run */
  retryCount: number;
  /** Last success result string */
  lastResult: string | null;
  /** Last error message */
  lastError: string | null;
  /** Creation timestamp */
  createdAt: Date;
  /** Last update timestamp */
  updatedAt: Date;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Get the effective max retries from a job config.
 */
export function getMaxRetries(config: JobConfig): number {
  return config.maxRetries ?? 3;
}

/**
 * Get the effective retry delay from a job config.
 */
export function getRetryDelayMs(config: JobConfig): number {
  return config.retryDelayMs ?? 60_000;
}

/**
 * Get the effective timeout from a job config.
 */
export function getTimeoutMs(config: JobConfig): number {
  return config.timeoutMs ?? 300_000;
}

/**
 * Calculate the exponential backoff delay for a retry attempt.
 * base * 2^attempt, capped at 1 hour.
 */
export function backoffDelay(baseMs: number, attempt: number): number {
  const delay = baseMs * Math.pow(2, attempt);
  return Math.min(delay, 3_600_000); // cap at 1 hour
}

/**
 * Check whether a job should be retried based on its state and config.
 */
export function shouldRetry(job: JobState): boolean {
  const maxRetries = getMaxRetries(job.config);
  return job.status === JobStatus.Failed && job.retryCount < maxRetries;
}
