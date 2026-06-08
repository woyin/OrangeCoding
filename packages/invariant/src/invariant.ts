// ---------------------------------------------------------------------------
// Invariant interface
// ---------------------------------------------------------------------------

/**
 * Invariant represents a runtime check that must hold true for the system to
 * be in a valid state.
 */
export interface Invariant {
  /** Name returns a human-readable identifier for this invariant. */
  name(): string;
  /** Check evaluates the invariant. A rejected promise means the invariant is violated. */
  check(ctx?: unknown): Promise<void>;
}

// ---------------------------------------------------------------------------
// Guard
// ---------------------------------------------------------------------------

/**
 * Guard runs a set of invariants and reports the first violation.
 */
export class Guard {
  constructor(private readonly invariants: Invariant[]) {}

  /**
   * Check runs all invariants sequentially and returns the first error
   * encountered, formatted as "invariant {name} violated: {error}".
   * Returns void when all invariants pass.
   */
  async check(ctx?: unknown): Promise<void> {
    for (const inv of this.invariants) {
      try {
        await inv.check(ctx);
      } catch (err) {
        throw new Error(`invariant ${inv.name()} violated: ${(err as Error).message}`);
      }
    }
  }
}

/**
 * NewGuard creates a Guard that will check the given invariants in order.
 */
export function newGuard(invariants: Invariant[]): Guard {
  return new Guard(invariants);
}

// ---------------------------------------------------------------------------
// Engine (checkpoint / rollback)
// ---------------------------------------------------------------------------

/**
 * Engine stores named snapshots of state that can be restored later via
 * rollback.
 */
export class Engine {
  private snapshots = new Map<string, unknown>();

  /**
   * Checkpoint stores a deep copy of state under the given id. If a snapshot
   * with the same id already exists it is overwritten.
   */
  checkpoint(id: string, state: unknown): void {
    this.snapshots.set(id, deepCopy(state));
  }

  /**
   * Rollback retrieves the snapshot stored under id.
   * Throws an error if no snapshot is found for the given id.
   */
  rollback(id: string): unknown {
    const state = this.snapshots.get(id);
    if (state === undefined) {
      throw new Error(`checkpoint "${id}" not found`);
    }
    return state;
  }
}

/**
 * deepCopy creates an independent copy of v for supported reference types
 * (maps, arrays). For value types, it returns v directly.
 */
function deepCopy(v: unknown): unknown {
  if (v === null || v === undefined) return v;

  if (Array.isArray(v)) {
    return [...v];
  }

  if (v instanceof Map) {
    return new Map(v);
  }

  if (v instanceof Set) {
    return new Set(v);
  }

  if (typeof v === "object") {
    return { ...(v as Record<string, unknown>) };
  }

  return v;
}

/**
 * NewEngine creates a new Engine with no snapshots.
 */
export function newEngine(): Engine {
  return new Engine();
}

// ---------------------------------------------------------------------------
// SelfHealingPolicy
// ---------------------------------------------------------------------------

/**
 * SelfHealingPolicy retries a fix function up to a configured number of
 * attempts.
 */
export class SelfHealingPolicy {
  private readonly maxAttempts: number;
  private readonly fix: (ctx?: unknown) => Promise<void>;

  constructor(maxAttempts: number, fix: (ctx?: unknown) => Promise<void>) {
    this.maxAttempts = maxAttempts < 1 ? 1 : maxAttempts;
    this.fix = fix;
  }

  /**
   * Execute runs the fix function repeatedly until it succeeds or the maximum
   * number of attempts is exhausted. Returns void on success, or the last error
   * if all attempts fail.
   */
  async execute(ctx?: unknown): Promise<void> {
    let lastErr: Error | undefined;
    for (let i = 0; i < this.maxAttempts; i++) {
      try {
        await this.fix(ctx);
        return;
      } catch (err) {
        lastErr = err as Error;
      }
    }
    throw lastErr!;
  }
}

/**
 * NewSelfHealingPolicy creates a policy that will call fix up to maxAttempts
 * times. A non-positive maxAttempts is treated as 1.
 */
export function newSelfHealingPolicy(
  maxAttempts: number,
  fix: (ctx?: unknown) => Promise<void>,
): SelfHealingPolicy {
  return new SelfHealingPolicy(maxAttempts, fix);
}
