/**
 * Tests for the cron expression parser and matcher.
 *
 * Covers: wildcard, single values, ranges, lists, step patterns,
 *   boundary values, error cases, and date matching.
 */

import { parseCron, matchCron, nextCronMatch } from "../cron.js";

// ---------------------------------------------------------------------------
// parseCron — field parsing
// ---------------------------------------------------------------------------

describe("parseCron", () => {
  it("parses all wildcards: '* * * * *'", () => {
    const expr = parseCron("* * * * *");
    expect(expr.minute).toHaveLength(60);
    expect(expr.hour).toHaveLength(24);
    expect(expr.dayOfMonth).toHaveLength(31);
    expect(expr.month).toHaveLength(12);
    expect(expr.dayOfWeek).toHaveLength(7);
  });

  it("parses single numeric values", () => {
    const expr = parseCron("30 9 15 6 3");
    expect(expr.minute).toEqual([30]);
    expect(expr.hour).toEqual([9]);
    expect(expr.dayOfMonth).toEqual([15]);
    expect(expr.month).toEqual([6]);
    expect(expr.dayOfWeek).toEqual([3]);
  });

  it("parses range patterns: '1-5'", () => {
    const expr = parseCron("1-5 9-17 * * 1-5");
    expect(expr.minute).toEqual([1, 2, 3, 4, 5]);
    expect(expr.hour).toEqual([9, 10, 11, 12, 13, 14, 15, 16, 17]);
    expect(expr.dayOfWeek).toEqual([1, 2, 3, 4, 5]);
  });

  it("parses comma-separated lists: '0,15,30,45'", () => {
    const expr = parseCron("0,15,30,45 * * * *");
    expect(expr.minute).toEqual([0, 15, 30, 45]);
  });

  it("parses step patterns: '*/15'", () => {
    const expr = parseCron("*/15 * * * *");
    expect(expr.minute).toEqual([0, 15, 30, 45]);
  });

  it("parses range/step combinations: '1-10/3'", () => {
    const expr = parseCron("1-10/3 * * * *");
    expect(expr.minute).toEqual([1, 4, 7, 10]);
  });

  it("parses zero minute and zero day-of-week", () => {
    const expr = parseCron("0 0 1 1 0");
    expect(expr.minute).toEqual([0]);
    expect(expr.hour).toEqual([0]);
    expect(expr.dayOfMonth).toEqual([1]);
    expect(expr.month).toEqual([1]);
    expect(expr.dayOfWeek).toEqual([0]);
  });

  it("parses max boundary values", () => {
    const expr = parseCron("59 23 31 12 6");
    expect(expr.minute).toEqual([59]);
    expect(expr.hour).toEqual([23]);
    expect(expr.dayOfMonth).toEqual([31]);
    expect(expr.month).toEqual([12]);
    expect(expr.dayOfWeek).toEqual([6]);
  });

  it("throws on too few fields", () => {
    expect(() => parseCron("* * *")).toThrow("expected 5 fields");
  });

  it("throws on too many fields", () => {
    expect(() => parseCron("* * * * * *")).toThrow("expected 5 fields");
  });

  it("throws on out-of-range minute", () => {
    expect(() => parseCron("60 * * * *")).toThrow("out of range");
  });

  it("throws on out-of-range hour", () => {
    expect(() => parseCron("* 24 * * *")).toThrow("out of range");
  });

  it("throws on out-of-range month", () => {
    expect(() => parseCron("* * * 13 *")).toThrow("out of range");
  });

  it("throws on out-of-range day-of-week", () => {
    expect(() => parseCron("* * * * 7")).toThrow("out of range");
  });

  it("throws on zero day-of-month", () => {
    expect(() => parseCron("* * 0 * *")).toThrow("out of range");
  });

  it("trims leading and trailing whitespace", () => {
    const expr = parseCron("  0 12 * * 1  ");
    expect(expr.minute).toEqual([0]);
    expect(expr.hour).toEqual([12]);
  });

  it("sorts values in ascending order", () => {
    const expr = parseCron("45,0,30,15 * * * *");
    expect(expr.minute).toEqual([0, 15, 30, 45]);
  });

  it("deduplicates overlapping values in lists", () => {
    const expr = parseCron("1,2,1,3,2 * * * *");
    expect(expr.minute).toEqual([1, 2, 3]);
  });
});

// ---------------------------------------------------------------------------
// matchCron — date matching
// ---------------------------------------------------------------------------

describe("matchCron", () => {
  const date = (year: number, month: number, day: number, hour: number, minute: number): Date =>
    new Date(year, month - 1, day, hour, minute, 0);

  it("matches every minute with all wildcards", () => {
    const expr = parseCron("* * * * *");
    expect(matchCron(expr, date(2025, 6, 14, 10, 30))).toBe(true);
    expect(matchCron(expr, date(2025, 1, 1, 0, 0))).toBe(true);
  });

  it("matches a specific time: '30 9 * * *'", () => {
    const expr = parseCron("30 9 * * *");
    expect(matchCron(expr, date(2025, 6, 14, 9, 30))).toBe(true);
    expect(matchCron(expr, date(2025, 6, 14, 9, 31))).toBe(false);
    expect(matchCron(expr, date(2025, 6, 14, 10, 30))).toBe(false);
  });

  it("matches weekdays only: '0 9 * * 1-5'", () => {
    const expr = parseCron("0 9 * * 1-5");
    // 2025-06-16 is a Monday (dayOfWeek=1)
    expect(matchCron(expr, date(2025, 6, 16, 9, 0))).toBe(true);
    // 2025-06-14 is a Saturday (dayOfWeek=6) — excluded
    expect(matchCron(expr, date(2025, 6, 14, 9, 0))).toBe(false);
  });

  it("matches specific day-of-month: '0 0 15 * *'", () => {
    const expr = parseCron("0 0 15 * *");
    expect(matchCron(expr, date(2025, 6, 15, 0, 0))).toBe(true);
    expect(matchCron(expr, date(2025, 6, 14, 0, 0))).toBe(false);
  });

  it("matches specific month: '0 0 1 12 *'", () => {
    const expr = parseCron("0 0 1 12 *");
    expect(matchCron(expr, date(2025, 12, 1, 0, 0))).toBe(true);
    expect(matchCron(expr, date(2025, 11, 1, 0, 0))).toBe(false);
  });

  it("matches Sunday as dayOfWeek=0", () => {
    const expr = parseCron("0 12 * * 0");
    // 2025-06-15 is a Sunday
    expect(matchCron(expr, date(2025, 6, 15, 12, 0))).toBe(true);
    // 2025-06-16 is a Monday
    expect(matchCron(expr, date(2025, 6, 16, 12, 0))).toBe(false);
  });

  it("matches step pattern: '*/5 * * * *'", () => {
    const expr = parseCron("*/5 * * * *");
    expect(matchCron(expr, date(2025, 6, 14, 10, 0))).toBe(true);
    expect(matchCron(expr, date(2025, 6, 14, 10, 5))).toBe(true);
    expect(matchCron(expr, date(2025, 6, 14, 10, 10))).toBe(true);
    expect(matchCron(expr, date(2025, 6, 14, 10, 3))).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// nextCronMatch — next occurrence calculation
// ---------------------------------------------------------------------------

describe("nextCronMatch", () => {
  const date = (year: number, month: number, day: number, hour: number, minute: number): Date =>
    new Date(year, month - 1, day, hour, minute, 0);

  it("finds the next minute for '*/15 * * * *'", () => {
    const expr = parseCron("*/15 * * * *");
    const from = date(2025, 6, 14, 10, 7);
    const next = nextCronMatch(expr, from);
    expect(next).not.toBeNull();
    expect(next!.getMinutes()).toBe(15);
    expect(next!.getHours()).toBe(10);
  });

  it("wraps to next hour when current hour's matches are exhausted", () => {
    const expr = parseCron("0 * * * *");
    const from = date(2025, 6, 14, 10, 30);
    const next = nextCronMatch(expr, from);
    expect(next).not.toBeNull();
    expect(next!.getHours()).toBe(11);
    expect(next!.getMinutes()).toBe(0);
  });

  it("finds next Monday 9am for '0 9 * * 1'", () => {
    const expr = parseCron("0 9 * * 1");
    // 2025-06-14 is Saturday
    const from = date(2025, 6, 14, 10, 0);
    const next = nextCronMatch(expr, from);
    expect(next).not.toBeNull();
    expect(next!.getDay()).toBe(1); // Monday
    expect(next!.getHours()).toBe(9);
    expect(next!.getMinutes()).toBe(0);
    expect(next!.getDate()).toBe(16);
  });

  it("skips to the next minute from the 'from' date", () => {
    const expr = parseCron("* * * * *");
    const from = date(2025, 6, 14, 10, 30);
    const next = nextCronMatch(expr, from);
    expect(next).not.toBeNull();
    expect(next!.getMinutes()).toBe(31);
  });

  it("returns a result within the search horizon", () => {
    const expr = parseCron("0 0 1 1 *"); // midnight Jan 1
    const from = date(2025, 6, 14, 10, 0);
    const next = nextCronMatch(expr, from);
    expect(next).not.toBeNull();
    expect(next!.getFullYear()).toBe(2026);
    expect(next!.getMonth()).toBe(0); // January
    expect(next!.getDate()).toBe(1);
  });
});
