/**
 * @module health
 * Health monitoring for managed agents with heartbeat tracking and recovery.
 */

import type { AgentId } from "@orangecoding/core";
import type { ManagedAgent, HealthReport } from "./registry.js";

// ---------------------------------------------------------------------------
// HealthMonitorConfig
// ---------------------------------------------------------------------------

/** Configures the health monitor. */
export interface HealthMonitorConfig {
  checkIntervalMs: number;
  missedThreshold: number;
  maxRestarts: number;
}

// ---------------------------------------------------------------------------
// HealthMonitor
// ---------------------------------------------------------------------------

/** Tracks agent heartbeats and triggers recovery. */
export class HealthMonitor {
  private config: HealthMonitorConfig;
  private lastSeen = new Map<string, Date>();
  private restarts = new Map<string, number>();
  private timers: ReturnType<typeof setInterval>[] = [];

  constructor(config: HealthMonitorConfig) {
    this.config = config;
  }

  /**
   * Start begins monitoring an agent.
   * @param agent - The agent to monitor.
   * @param restartHandler - Callback invoked when an agent needs recovery.
   * @returns A stop function to cease monitoring.
   */
  start(agent: ManagedAgent, restartHandler?: (agent: ManagedAgent) => void): () => void {
    const id = agent.id().toString();
    this.lastSeen.set(id, new Date());

    const timer = setInterval(() => {
      this.checkAgent(agent, restartHandler);
    }, this.config.checkIntervalMs);

    this.timers.push(timer);

    // Return a stop function (equivalent to ctx cancellation in Go).
    return () => {
      clearInterval(timer);
      const idx = this.timers.indexOf(timer);
      if (idx !== -1) {
        this.timers.splice(idx, 1);
      }
    };
  }

  private checkAgent(agent: ManagedAgent, restartHandler?: (agent: ManagedAgent) => void): void {
    const id = agent.id().toString();
    const last = this.lastSeen.get(id);
    if (!last) return;

    const elapsed = Date.now() - last.getTime();
    const missed = Math.floor(elapsed / this.config.checkIntervalMs);

    if (missed >= this.config.missedThreshold) {
      const restartCount = this.restarts.get(id) ?? 0;
      if (restartCount < this.config.maxRestarts) {
        this.restarts.set(id, restartCount + 1);
        if (restartHandler) {
          // Fire-and-forget (mirrors goroutine behavior).
          Promise.resolve().then(() => restartHandler(agent)).catch(() => {});
        }
      }
    }
  }

  /** RecordHeartbeat marks an agent as alive. */
  recordHeartbeat(agentID: string): void {
    this.lastSeen.set(agentID, new Date());
  }
}
