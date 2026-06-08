import { AiError, AiErrorKind } from "./error.js";

// ---------------------------------------------------------------------------
// RateLimitPolicy — configures retry behavior for 429 responses
// ---------------------------------------------------------------------------

export interface RateLimitPolicy {
  /** Whether to prompt the user or auto-retry silently */
  promptUser: boolean;
  /** Maximum number of retry attempts (0 = unlimited) */
  maxRetries: number;
  /** Base interval between retries in ms (doubles on each attempt) */
  retryIntervalMs: number;
  /** Maximum retry interval cap in ms */
  maxIntervalMs: number;
}

const DEFAULT_POLICY: RateLimitPolicy = {
  promptUser: true,
  maxRetries: 0,
  retryIntervalMs: 30_000,
  maxIntervalMs: 300_000,
};

// ---------------------------------------------------------------------------
// RateLimitResult
// ---------------------------------------------------------------------------

export type RetryDecision = "retry" | "abort";

// ---------------------------------------------------------------------------
// RateLimitHandler
// ---------------------------------------------------------------------------

/**
 * Handles 429 rate-limit errors with user prompting and exponential backoff.
 *
 * Usage:
 *   const handler = new RateLimitHandler(policy);
 *   const result = await handler.handleRateLimit(error, async () => {
 *     // the operation to retry
 *     return await provider.chatCompletion(messages, tools, opts);
 *   });
 */
export class RateLimitHandler {
  private _policy: RateLimitPolicy;
  private _attempt: number;
  private _askFn: (msg: string) => Promise<RetryDecision>;

  constructor(
    policy: Partial<RateLimitPolicy> = {},
    askFn?: (msg: string) => Promise<RetryDecision>,
  ) {
    this._policy = { ...DEFAULT_POLICY, ...policy };
    this._attempt = 0;
    this._askFn = askFn ?? defaultAskFn;
  }

  /**
   * Execute an operation with automatic rate-limit retry.
   * On 429, prompts the user (if configured), then retries with backoff.
   */
  async execute<T>(fn: () => Promise<T>): Promise<T> {
    try {
      const result = await fn();
      this._attempt = 0;
      return result;
    } catch (err) {
      if (!isRateLimitError(err)) throw err;
      return this.handleRateLimitError(err as AiError, fn);
    }
  }

  private async handleRateLimitError<T>(err: AiError, fn: () => Promise<T>): Promise<T> {
    this._attempt++;
    const retryAfterSec = err.retryAfter > 0 ? err.retryAfter : 0;

    if (this._policy.maxRetries > 0 && this._attempt > this._policy.maxRetries) {
      throw err;
    }

    if (this._policy.promptUser && this._attempt === 1) {
      const waitSec = retryAfterSec > 0 ? retryAfterSec : "unknown";
      const decision = await this._askFn(
        `Coding plan rate limited (429). Retry-After: ${waitSec}s. Attempt #${this._attempt}. Wait and retry?`,
      );
      if (decision === "abort") {
        throw err;
      }
    }

    const delayMs = this.computeDelay(retryAfterSec);
    await sleep(delayMs);

    try {
      const result = await fn();
      this._attempt = 0;
      return result;
    } catch (retryErr) {
      if (!isRateLimitError(retryErr)) throw retryErr;
      return this.handleRateLimitError(retryErr as AiError, fn);
    }
  }

  private computeDelay(retryAfterSec: number): number {
    if (retryAfterSec > 0) {
      return Math.min(retryAfterSec * 1000, this._policy.maxIntervalMs);
    }
    const exponential = this._policy.retryIntervalMs * Math.pow(2, this._attempt - 1);
    return Math.min(exponential, this._policy.maxIntervalMs);
  }

  get attempt(): number {
    return this._attempt;
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isRateLimitError(err: unknown): boolean {
  return err instanceof AiError && err.kind === AiErrorKind.RateLimit;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Default user prompt function — reads from stdin.
 */
async function defaultAskFn(msg: string): Promise<RetryDecision> {
  const { createInterface } = await import("readline");
  return new Promise((resolve) => {
    const rl = createInterface({
      input: process.stdin,
      output: process.stderr,
    });
    rl.question(`${msg} [Y/n] `, (answer: string) => {
      rl.close();
      const lower = answer.trim().toLowerCase();
      if (lower === "n" || lower === "no") {
        resolve("abort");
      } else {
        resolve("retry");
      }
    });
  });
}

/**
 * Wrap an AiProvider to automatically handle 429 errors.
 */
export function withRateLimitRetry(
  policy: Partial<RateLimitPolicy> = {},
  askFn?: (msg: string) => Promise<RetryDecision>,
): (provider: { chatCompletion: (...args: any[]) => Promise<any>; chatCompletionStream: (...args: any[]) => Promise<any> }) => typeof provider {
  const handler = new RateLimitHandler(policy, askFn);
  return (provider) => ({
    chatCompletion: (...args: Parameters<typeof provider.chatCompletion>) =>
      handler.execute(() => provider.chatCompletion(...args)),
    chatCompletionStream: (...args: Parameters<typeof provider.chatCompletionStream>) =>
      handler.execute(() => provider.chatCompletionStream(...args)),
  });
}
