/**
 * Cron expression parser and matcher.
 *
 * Supports standard 5-field cron: minute hour day-of-month month day-of-week.
 * All fields support: numbers, ranges (1-5), lists (1,3,5), steps (&#42;/5),
 *   and wildcards (&#42;).
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CronExpression {
  readonly minute: number[];
  readonly hour: number[];
  readonly dayOfMonth: number[];
  readonly month: number[];
  readonly dayOfWeek: number[];
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const FIELD_RANGES: Record<string, { min: number; max: number }> = {
  minute: { min: 0, max: 59 },
  hour: { min: 0, max: 23 },
  "day-of-month": { min: 1, max: 31 },
  month: { min: 1, max: 12 },
  "day-of-week": { min: 0, max: 6 },
} as const;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function parseField(field: string, fieldName: string): number[] {
  const range = FIELD_RANGES[fieldName];
  if (!range) {
    throw new Error(`unknown cron field: "${fieldName}"`);
  }

  // Wildcard
  if (field === "*") {
    return rangeArray(range.min, range.max);
  }

  const values: Set<number> = new Set();

  // Split by comma to handle lists
  const parts = field.split(",");
  for (const part of parts) {
    if (part.includes("/")) {
      // Step pattern: range/step or */step
      const [rangePart, stepStr] = part.split("/", 2);
      if (!stepStr || stepStr.length === 0) {
        throw new Error(`invalid cron step: "${part}" in field "${fieldName}"`);
      }
      const step = parseInt(stepStr, 10);
      if (isNaN(step) || step < 1) {
        throw new Error(`invalid cron step value: "${stepStr}" in field "${fieldName}"`);
      }
      if (!rangePart) {
        throw new Error(`invalid cron range: "${part}" in field "${fieldName}"`);
      }
      // Check for range/step pattern like 1-10/3 before falling through to parseInt.
      // parseInt("1-10", 10) returns 1, which would silently skip the range end,
      // so we must detect the dash first.
      if (rangePart !== "*" && rangePart.includes("-")) {
        const [rStart, rEnd] = rangePart.split("-", 2);
        const rs = parseInt(rStart ?? "", 10);
        const re = parseInt(rEnd ?? "", 10);
        if (isNaN(rs) || isNaN(re)) {
          throw new Error(`invalid cron range/step: "${part}" in field "${fieldName}"`);
        }
        for (let i = rs; i <= re; i += step) {
          values.add(i);
        }
        continue;
      }
      const start = rangePart === "*" ? range.min : parseInt(rangePart, 10);
      if (isNaN(start)) {
        // Fallback: try range pattern like 1-5/2
        if (rangePart.includes("-")) {
          const [rStart, rEnd] = rangePart.split("-", 2);
          const rs = parseInt(rStart ?? "", 10);
          const re = parseInt(rEnd ?? "", 10);
          if (isNaN(rs) || isNaN(re)) {
            throw new Error(`invalid cron range/step: "${part}" in field "${fieldName}"`);
          }
          for (let i = rs; i <= re; i += step) {
            values.add(i);
          }
        } else {
          throw new Error(`invalid cron start value: "${rangePart}" in field "${fieldName}"`);
        }
        continue;
      }
      for (let i = start; i <= range.max; i += step) {
        values.add(i);
      }
    } else if (part.includes("-")) {
      // Range pattern: 1-5
      const [startStr, endStr] = part.split("-", 2);
      const s = parseInt(startStr ?? "", 10);
      const e = parseInt(endStr ?? "", 10);
      if (isNaN(s) || isNaN(e)) {
        throw new Error(`invalid cron range: "${part}" in field "${fieldName}"`);
      }
      for (let i = s; i <= e; i++) {
        values.add(i);
      }
    } else {
      // Single value
      const v = parseInt(part, 10);
      if (isNaN(v)) {
        throw new Error(`invalid cron value: "${part}" in field "${fieldName}"`);
      }
      values.add(v);
    }
  }

  const result = [...values].sort((a, b) => a - b);
  for (const v of result) {
    if (v < range.min || v > range.max) {
      throw new Error(
        `cron value ${v} out of range [${range.min}, ${range.max}] for field "${fieldName}"`
      );
    }
  }
  return result;
}

function rangeArray(min: number, max: number): number[] {
  const result: number[] = [];
  for (let i = min; i <= max; i++) {
    result.push(i);
  }
  return result;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Parse a 5-field cron expression string into field arrays.
 *
 * Format: "minute hour day-of-month month day-of-week"
 * Example: "&#42;/5 9 &#42; &#42; 1-5" = every 5 minutes, 9am-9:59, weekdays
 */
export function parseCron(expr: string): CronExpression {
  const fields = expr.trim().split(/\s+/);
  if (fields.length !== 5) {
    throw new Error(
      `invalid cron expression: expected 5 fields, got ${fields.length}: "${expr}"`
    );
  }

  return {
    minute: parseField(fields[0] ?? "", "minute"),
    hour: parseField(fields[1] ?? "", "hour"),
    dayOfMonth: parseField(fields[2] ?? "", "day-of-month"),
    month: parseField(fields[3] ?? "", "month"),
    dayOfWeek: parseField(fields[4] ?? "", "day-of-week"),
  };
}

/**
 * Check whether a parsed cron expression matches a given date.
 */
export function matchCron(expr: CronExpression, date: Date): boolean {
  if (!expr.month.includes(date.getMonth() + 1)) return false;
  if (!expr.dayOfMonth.includes(date.getDate())) return false;
  // 0 = Sunday in cron, getDay() returns 0 = Sunday
  if (!expr.dayOfWeek.includes(date.getDay())) return false;
  if (!expr.hour.includes(date.getHours())) return false;
  if (!expr.minute.includes(date.getMinutes())) return false;
  return true;
}

/**
 * Calculate the next matching datetime from a parsed cron expression.
 *
 * @param expr - parsed cron expression
 * @param from - starting point (default: now)
 * @returns the next Date that matches, or null if none exists within a reasonable horizon
 */
export function nextCronMatch(expr: CronExpression, from: Date = new Date()): Date | null {
  // Start from the next minute
  const candidate = new Date(from);
  candidate.setSeconds(0, 0);
  candidate.setMinutes(candidate.getMinutes() + 1);

  // Search up to 3 years forward to avoid infinite loops
  const horizon = new Date(from);
  horizon.setFullYear(horizon.getFullYear() + 3);

  while (candidate <= horizon) {
    if (matchCron(expr, candidate)) {
      return candidate;
    }
    // Advance by one minute
    candidate.setMinutes(candidate.getMinutes() + 1);
  }

  return null;
}
