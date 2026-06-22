/**
 * Performance regression test for AuditLog.append().
 *
 * The prior implementation was O(n^2) over the lifetime of the log: each
 * append read the whole file, parsed every entry, and rewrote the whole
 * file. This test asserts that the current O(1)-per-append implementation
 * holds by comparing per-append latency at two log sizes (small vs large).
 *
 * Assertion strategy: if append is O(1), per-append time at N=500 should be
 * in the same ballpark as per-append time at N=20 (ratio < 4x with generous
 * slack for CI noise / GC / disk jitter). Under the old O(n)-per-append
 * implementation, the ratio would be ~25x.
 *
 * NOTE: These are real filesystem appends, so absolute numbers vary by
 * machine. The *ratio* between small and large is the stable signal.
 */

import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { AuditLog, verifyChain } from "../index.js";

/**
 * Append `count` entries and return the per-append latency in milliseconds.
 * Reuses a single AuditLog instance so the _lastHash cache is exercised.
 */
async function measureAppend(count: number): Promise<{ perAppendMs: number }> {
  const dir = await mkdtemp(join(tmpdir(), "audit-perf-"));
  try {
    const log = await AuditLog.create(dir);
    // Warm-up: first append seeds _lastHash by reading the (empty) file.
    await log.append("warmup", "perf", "{}");

    const t0 = performance.now();
    for (let i = 0; i < count; i++) {
      await log.append("tool_call_completed", "agent-perf", `{"i":${i}}`);
    }
    const t1 = performance.now();
    return { perAppendMs: (t1 - t0) / count };
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

describe("AuditLog append scaling (O(1) regression)", () => {
  it("per-append latency stays bounded as the log grows (ratio < 4x)", async () => {
    const small = await measureAppend(20);
    const large = await measureAppend(500);
    const ratio = large.perAppendMs / small.perAppendMs;

    // Log observable diagnostics for debugging flakiness.
    // eslint-disable-next-line no-console
    console.log(
      `  append perf: 20x=${small.perAppendMs.toFixed(4)}ms  500x=${large.perAppendMs.toFixed(4)}ms  ratio=${ratio.toFixed(2)}x`,
    );

    // O(1) per-append: ratio stays near 1. Old O(n) per-append: ratio ~25.
    // 4x headroom absorbs CI/disk/GC noise while still catching an O(n)
    // regression decisively.
    expect(ratio).toBeLessThan(4);
  }, 30_000);

  it("appends maintain hash-chain integrity at scale", async () => {
    const dir = await mkdtemp(join(tmpdir(), "audit-integrity-"));
    try {
      const log = await AuditLog.create(dir);
      for (let i = 0; i < 100; i++) {
        await log.append("tool_call_completed", "agent-1", `{"i":${i}}`);
      }
      const entries = await log.getEntries();
      expect(entries).toHaveLength(100);
      // verifyChain walks every link; a broken chain returns an Error.
      expect(verifyChain(entries)).toBeNull();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
