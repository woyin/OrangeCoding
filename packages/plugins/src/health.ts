/**
 * Plugin health monitoring — periodic checks and circuit breaker.
 */

import type { PluginInstance, HealthStatus } from "./types.js";

/**
 * Perform a health check on a plugin.
 */
export async function healthCheck(instance: PluginInstance): Promise<HealthStatus> {
  const name = instance.manifest.name;
  const uptimeMs = instance.startedAt ? Date.now() - instance.startedAt.getTime() : 0;

  if (!instance.client || instance.status === "stopped" || instance.status === "error") {
    return {
      name,
      alive: false,
      uptimeMs,
      lastError: instance.error ?? null,
      toolCount: instance.manifest.tools.length,
    };
  }

  try {
    // MCP "ping"-like: listTools is a lightweight call
    const tools = await instance.client.listTools();
    return {
      name,
      alive: true,
      uptimeMs,
      lastError: null,
      toolCount: tools.length,
    };
  } catch (err) {
    return {
      name,
      alive: false,
      uptimeMs,
      lastError: (err as Error).message,
      toolCount: 0,
    };
  }
}

/**
 * Circuit breaker state for a plugin.
 */
export interface CircuitBreakerState {
  failures: number;
  lastFailure: Date | null;
  tripped: boolean;
  trippedAt: Date | null;
}

/**
 * Circuit breaker: tracks failures and trips when threshold exceeded.
 */
export class CircuitBreaker {
  private readonly _maxFailures: number;
  private readonly _resetMs: number;
  private _state: Map<string, CircuitBreakerState> = new Map();

  constructor(maxFailures: number, resetWindowMs: number) {
    this._maxFailures = maxFailures;
    this._resetMs = resetWindowMs;
  }

  /**
   * Record a failure for a plugin.
   * Returns true if the circuit is now tripped.
   */
  recordFailure(name: string): boolean {
    const now = new Date();
    let state = this._state.get(name);

    if (!state) {
      state = { failures: 0, lastFailure: null, tripped: false, trippedAt: null };
      this._state.set(name, state);
    }

    // Reset if window has passed
    if (state.lastFailure && (now.getTime() - state.lastFailure.getTime()) > this._resetMs) {
      state.failures = 0;
      state.tripped = false;
    }

    state.failures++;
    state.lastFailure = now;

    if (state.failures >= this._maxFailures) {
      state.tripped = true;
      state.trippedAt = now;
    }

    return state.tripped;
  }

  /**
   * Record a success (reset failure count).
   */
  recordSuccess(name: string): void {
    const state = this._state.get(name);
    if (state) {
      state.failures = 0;
      state.tripped = false;
      state.trippedAt = null;
    }
  }

  /**
   * Check if a plugin's circuit is tripped.
   */
  isTripped(name: string): boolean {
    const state = this._state.get(name);
    if (!state) return false;

    // Auto-reset after timeout
    if (state.tripped && state.trippedAt) {
      const elapsed = Date.now() - state.trippedAt.getTime();
      if (elapsed > this._resetMs) {
        state.tripped = false;
        state.failures = 0;
        state.trippedAt = null;
        return false;
      }
    }

    return state.tripped;
  }

  /**
   * Reset all circuit breaker state.
   */
  reset(): void {
    this._state.clear();
  }
}
