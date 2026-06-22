/**
 * Tests for the Scheduler class — lifecycle, job management, and execution.
 *
 * Uses MemoryJobStore to avoid filesystem side effects.
 */

import { Scheduler } from "../scheduler.js";
import { MemoryJobStore } from "../store.js";
import { JobStatus } from "../job.js";
import type { JobState, JobConfig } from "../job.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Create a scheduler with an in-memory store and fast tick interval. */
function createScheduler(
  overrides: { tickIntervalMs?: number; maxConcurrentJobs?: number } = {},
  callbacks: {
    onJobStart?: (job: JobState) => Promise<string>;
    onJobComplete?: (job: JobState, result: string) => void;
    onJobFail?: (job: JobState, error: Error) => void;
  } = {},
): { scheduler: Scheduler; store: MemoryJobStore } {
  const store = new MemoryJobStore();
  const scheduler = new Scheduler(
    {
      tickIntervalMs: overrides.tickIntervalMs ?? 100,
      maxConcurrentJobs: overrides.maxConcurrentJobs ?? 5,
      store,
    },
    callbacks,
  );
  return { scheduler, store };
}

function makeJobConfig(overrides: Partial<JobConfig> = {}): JobConfig {
  return {
    name: "test-job",
    prompt: "do something",
    cron: "* * * * *",
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Job creation
// ---------------------------------------------------------------------------

describe("Scheduler — job creation", () => {
  it("creates a job with pending status", () => {
    const { scheduler } = createScheduler();
    const job = scheduler.create(makeJobConfig());

    expect(job.id).toBeTruthy();
    expect(job.status).toBe(JobStatus.Pending);
    expect(job.runCount).toBe(0);
    expect(job.retryCount).toBe(0);
    expect(job.lastResult).toBeNull();
    expect(job.lastError).toBeNull();
    expect(job.nextRunAt).not.toBeNull();
  });

  it("assigns unique IDs to different jobs", () => {
    const { scheduler } = createScheduler();
    const job1 = scheduler.create(makeJobConfig());
    const job2 = scheduler.create(makeJobConfig());

    expect(job1.id).not.toBe(job2.id);
  });

  it("retrieves a created job by ID", () => {
    const { scheduler } = createScheduler();
    const job = scheduler.create(makeJobConfig());
    const retrieved = scheduler.get(job.id);

    expect(retrieved).toBeDefined();
    expect(retrieved!.id).toBe(job.id);
  });

  it("returns undefined for non-existent job ID", () => {
    const { scheduler } = createScheduler();
    expect(scheduler.get("non-existent")).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// Job listing
// ---------------------------------------------------------------------------

describe("Scheduler — listing", () => {
  it("lists all jobs when no status filter is provided", () => {
    const { scheduler } = createScheduler();
    scheduler.create(makeJobConfig({ name: "job-1" }));
    scheduler.create(makeJobConfig({ name: "job-2" }));
    scheduler.create(makeJobConfig({ name: "job-3" }));

    expect(scheduler.list()).toHaveLength(3);
  });

  it("filters jobs by status", () => {
    const { scheduler } = createScheduler();
    const job1 = scheduler.create(makeJobConfig({ name: "job-1" }));
    scheduler.create(makeJobConfig({ name: "job-2" }));
    scheduler.create(makeJobConfig({ name: "job-3" }));

    // Pause job1
    scheduler.pause(job1.id);

    expect(scheduler.list(JobStatus.Pending)).toHaveLength(2);
    expect(scheduler.list(JobStatus.Paused)).toHaveLength(1);
  });

  it("returns empty array when no jobs exist", () => {
    const { scheduler } = createScheduler();
    expect(scheduler.list()).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// Pause / Resume / Cancel
// ---------------------------------------------------------------------------

describe("Scheduler — pause, resume, cancel", () => {
  it("pauses a pending job", () => {
    const { scheduler } = createScheduler();
    const job = scheduler.create(makeJobConfig());

    scheduler.pause(job.id);
    const updated = scheduler.get(job.id);

    expect(updated!.status).toBe(JobStatus.Paused);
  });

  it("resumes a paused job back to pending", () => {
    const { scheduler } = createScheduler();
    const job = scheduler.create(makeJobConfig());

    scheduler.pause(job.id);
    scheduler.resume(job.id);
    const updated = scheduler.get(job.id);

    expect(updated!.status).toBe(JobStatus.Pending);
  });

  it("does not resume a non-paused job", () => {
    const { scheduler } = createScheduler();
    const job = scheduler.create(makeJobConfig());

    // Job is already pending, resume should be a no-op
    scheduler.resume(job.id);
    expect(scheduler.get(job.id)!.status).toBe(JobStatus.Pending);
  });

  it("cancels and removes a job", () => {
    const { scheduler } = createScheduler();
    const job = scheduler.create(makeJobConfig());

    scheduler.cancel(job.id);
    expect(scheduler.get(job.id)).toBeUndefined();
    expect(scheduler.list()).toHaveLength(0);
  });

  it("pause is a no-op for non-existent job", () => {
    const { scheduler } = createScheduler();
    // Should not throw
    scheduler.pause("non-existent");
  });

  it("cancel is a no-op for non-existent job", () => {
    const { scheduler } = createScheduler();
    // Should not throw
    scheduler.cancel("non-existent");
  });
});

// ---------------------------------------------------------------------------
// Lifecycle — start / stop
// ---------------------------------------------------------------------------

describe("Scheduler — lifecycle", () => {
  it("starts and stops without errors", async () => {
    const { scheduler } = createScheduler();
    await scheduler.start();
    await scheduler.stop();
  });

  it("stops when abort signal fires", async () => {
    const { scheduler } = createScheduler();
    const controller = new AbortController();

    await scheduler.start(controller.signal);
    controller.abort();

    // Give a tick to process the abort
    await new Promise((r) => setTimeout(r, 50));

    // Should be stopped (no timer running)
    await scheduler.stop();
  });

  it("does not start if signal is already aborted", async () => {
    const { scheduler } = createScheduler();
    const controller = new AbortController();
    controller.abort();

    await scheduler.start(controller.signal);
    await scheduler.stop();
  });
});

// ---------------------------------------------------------------------------
// MemoryJobStore
// ---------------------------------------------------------------------------

describe("MemoryJobStore", () => {
  it("saves and loads job state", async () => {
    const store = new MemoryJobStore();
    const jobs = new Map<string, JobState>();

    const job: JobState = {
      id: "test-1",
      config: makeJobConfig(),
      status: JobStatus.Pending,
      lastRunAt: null,
      nextRunAt: null,
      runCount: 0,
      retryCount: 0,
      lastResult: null,
      lastError: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    };

    jobs.set(job.id, job);
    await store.save(jobs);

    const loaded = await store.load();
    expect(loaded.size).toBe(1);
    expect(loaded.get("test-1")?.id).toBe("test-1");
  });

  it("returns an empty map when no jobs saved", async () => {
    const store = new MemoryJobStore();
    const loaded = await store.load();
    expect(loaded.size).toBe(0);
  });

  it("load returns a copy (mutations do not affect store)", async () => {
    const store = new MemoryJobStore();
    const jobs = new Map<string, JobState>();
    const job: JobState = {
      id: "test-1",
      config: makeJobConfig(),
      status: JobStatus.Pending,
      lastRunAt: null,
      nextRunAt: null,
      runCount: 0,
      retryCount: 0,
      lastResult: null,
      lastError: null,
      createdAt: new Date(),
      updatedAt: new Date(),
    };
    jobs.set(job.id, job);
    await store.save(jobs);

    const loaded = await store.load();
    loaded.delete("test-1");

    const loaded2 = await store.load();
    expect(loaded2.size).toBe(1);
  });
});
