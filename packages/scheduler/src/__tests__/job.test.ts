/**
 * Tests for the job types module — helper functions and backoff calculations.
 *
 * Covers: config defaults, exponential backoff, and retry logic.
 */

import {
  JobStatus,
  getMaxRetries,
  getRetryDelayMs,
  getTimeoutMs,
  backoffDelay,
  shouldRetry,
} from "../job.js";
import type { JobConfig, JobState } from "../job.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Minimal JobConfig factory for testing. */
function makeConfig(overrides: Partial<JobConfig> = {}): JobConfig {
  return {
    name: "test-job",
    prompt: "do something",
    cron: "* * * * *",
    ...overrides,
  };
}

/** Minimal JobState factory for testing. */
function makeJobState(overrides: Partial<JobState> = {}): JobState {
  return {
    id: "test-id",
    config: makeConfig(),
    status: JobStatus.Pending,
    lastRunAt: null,
    nextRunAt: null,
    runCount: 0,
    retryCount: 0,
    lastResult: null,
    lastError: null,
    createdAt: new Date(),
    updatedAt: new Date(),
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Config defaults
// ---------------------------------------------------------------------------

describe("job config defaults", () => {
  it("getMaxRetries returns 3 when not specified", () => {
    expect(getMaxRetries(makeConfig())).toBe(3);
  });

  it("getMaxRetries returns the configured value", () => {
    expect(getMaxRetries(makeConfig({ maxRetries: 5 }))).toBe(5);
  });

  it("getRetryDelayMs returns 60000 when not specified", () => {
    expect(getRetryDelayMs(makeConfig())).toBe(60_000);
  });

  it("getRetryDelayMs returns the configured value", () => {
    expect(getRetryDelayMs(makeConfig({ retryDelayMs: 10_000 }))).toBe(10_000);
  });

  it("getTimeoutMs returns 300000 when not specified", () => {
    expect(getTimeoutMs(makeConfig())).toBe(300_000);
  });

  it("getTimeoutMs returns the configured value", () => {
    expect(getTimeoutMs(makeConfig({ timeoutMs: 60_000 }))).toBe(60_000);
  });
});

// ---------------------------------------------------------------------------
// backoffDelay — exponential backoff
// ---------------------------------------------------------------------------

describe("backoffDelay", () => {
  it("returns base * 2^0 for attempt 0", () => {
    expect(backoffDelay(1000, 0)).toBe(1000);
  });

  it("returns base * 2^1 for attempt 1", () => {
    expect(backoffDelay(1000, 1)).toBe(2000);
  });

  it("returns base * 2^3 for attempt 3", () => {
    expect(backoffDelay(1000, 3)).toBe(8000);
  });

  it("caps at 1 hour (3600000 ms)", () => {
    // 60000 * 2^20 = ~63 billion, should be capped
    expect(backoffDelay(60_000, 20)).toBe(3_600_000);
  });

  it("scales linearly with base", () => {
    expect(backoffDelay(500, 2)).toBe(2000);
    expect(backoffDelay(2000, 2)).toBe(8000);
  });
});

// ---------------------------------------------------------------------------
// shouldRetry — retry decision
// ---------------------------------------------------------------------------

describe("shouldRetry", () => {
  it("returns true when job is failed and retryCount < maxRetries", () => {
    const job = makeJobState({
      status: JobStatus.Failed,
      retryCount: 1,
      config: makeConfig({ maxRetries: 3 }),
    });
    expect(shouldRetry(job)).toBe(true);
  });

  it("returns false when retryCount >= maxRetries", () => {
    const job = makeJobState({
      status: JobStatus.Failed,
      retryCount: 3,
      config: makeConfig({ maxRetries: 3 }),
    });
    expect(shouldRetry(job)).toBe(false);
  });

  it("returns false when job is not failed", () => {
    const job = makeJobState({
      status: JobStatus.Completed,
      retryCount: 0,
      config: makeConfig({ maxRetries: 3 }),
    });
    expect(shouldRetry(job)).toBe(false);
  });

  it("returns false when job is pending", () => {
    const job = makeJobState({
      status: JobStatus.Pending,
      retryCount: 0,
      config: makeConfig({ maxRetries: 3 }),
    });
    expect(shouldRetry(job)).toBe(false);
  });

  it("handles maxRetries = 0 (never retry)", () => {
    const job = makeJobState({
      status: JobStatus.Failed,
      retryCount: 0,
      config: makeConfig({ maxRetries: 0 }),
    });
    expect(shouldRetry(job)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// JobStatus constants
// ---------------------------------------------------------------------------

describe("JobStatus", () => {
  it("has all expected status values", () => {
    expect(JobStatus.Pending).toBe("pending");
    expect(JobStatus.Running).toBe("running");
    expect(JobStatus.Completed).toBe("completed");
    expect(JobStatus.Failed).toBe("failed");
    expect(JobStatus.Paused).toBe("paused");
  });
});
