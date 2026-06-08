/**
 * TraceStore persists and queries trace events independently from checkpoints.
 * Ported from modules/agent/harness_trace.go.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import type { SessionId } from "@orangecoding/core";
import type { HarnessState } from "./harness-state.js";

// ---------------------------------------------------------------------------
// TraceSchemaVersion
// ---------------------------------------------------------------------------

export const TRACE_SCHEMA_VERSION = "1.0";

// ---------------------------------------------------------------------------
// TraceEvent
// ---------------------------------------------------------------------------

export interface TraceEvent {
  schemaVersion: string;
  runID: string;
  sessionID: SessionId;
  fromState: HarnessState;
  toState: HarnessState;
  reason?: string;
  metadata?: Record<string, string>;
  createdAt: Date;
}

// ---------------------------------------------------------------------------
// TraceQuery
// ---------------------------------------------------------------------------

export interface TraceQuery {
  runID?: string;
  sessionID?: SessionId;
  fromState?: HarnessState;
  toState?: HarnessState;
  startTime?: Date;
  endTime?: Date;
  limit?: number;
}

// ---------------------------------------------------------------------------
// TraceStore interface
// ---------------------------------------------------------------------------

export interface TraceStore {
  append(signal: AbortSignal | undefined, event: TraceEvent): Promise<void>;
  query(signal: AbortSignal | undefined, q: TraceQuery): Promise<TraceEvent[]>;
}

// ---------------------------------------------------------------------------
// MemoryTraceStore
// ---------------------------------------------------------------------------

export class MemoryTraceStore implements TraceStore {
  private _events: TraceEvent[];

  constructor() {
    this._events = [];
  }

  async append(_signal: AbortSignal | undefined, event: TraceEvent): Promise<void> {
    this._events.push(event);
  }

  async query(_signal: AbortSignal | undefined, q: TraceQuery): Promise<TraceEvent[]> {
    let result = this._events.filter((e) => matchesTraceQuery(e, q));
    // Sort by createdAt descending
    result.sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime());
    if (q.limit && q.limit > 0 && result.length > q.limit) {
      result = result.slice(0, q.limit);
    }
    return result;
  }
}

// ---------------------------------------------------------------------------
// FileTraceStore
// ---------------------------------------------------------------------------

export class FileTraceStore implements TraceStore {
  private _dir: string;

  constructor(dir: string) {
    this._dir = dir;
  }

  async append(_signal: AbortSignal | undefined, event: TraceEvent): Promise<void> {
    if (!event.runID) throw new Error("trace store: run id is required");
    await fs.promises.mkdir(this._dir, { recursive: true });
    if (!event.schemaVersion) event.schemaVersion = TRACE_SCHEMA_VERSION;
    const data = JSON.stringify(event);
    const filePath = this.pathFor(event.runID);
    await fs.promises.appendFile(filePath, data + "\n", "utf-8");
  }

  async query(_signal: AbortSignal | undefined, q: TraceQuery): Promise<TraceEvent[]> {
    let runIDs: string[];
    if (q.runID) {
      runIDs = [q.runID];
    } else {
      try {
        const entries = await fs.promises.readdir(this._dir);
        runIDs = entries
          .filter((e) => !e.startsWith(".") && e.endsWith(".ndjson"))
          .map((e) => e.slice(0, -8)); // remove .ndjson
      } catch (err) {
        if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
        throw err;
      }
    }

    let result: TraceEvent[] = [];
    for (const runID of runIDs) {
      const events = await this.loadRunTrace(runID);
      for (const e of events) {
        if (matchesTraceQuery(e, q)) result.push(e);
      }
    }

    result.sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime());
    if (q.limit && q.limit > 0 && result.length > q.limit) {
      result = result.slice(0, q.limit);
    }
    return result;
  }

  private async loadRunTrace(runID: string): Promise<TraceEvent[]> {
    try {
      const data = await fs.promises.readFile(this.pathFor(runID), "utf-8");
      const events: TraceEvent[] = [];
      for (const line of data.split("\n")) {
        const trimmed = line.trim();
        if (!trimmed) continue;
        try {
          events.push(JSON.parse(trimmed));
        } catch {
          continue;
        }
      }
      return events;
    } catch {
      return [];
    }
  }

  private pathFor(runID: string): string {
    return path.join(this._dir, `${runID}.ndjson`);
  }
}

// ---------------------------------------------------------------------------
// matchesTraceQuery
// ---------------------------------------------------------------------------

function matchesTraceQuery(e: TraceEvent, q: TraceQuery): boolean {
  if (q.runID && e.runID !== q.runID) return false;
  if (q.sessionID && e.sessionID !== q.sessionID) return false;
  if (q.fromState && e.fromState !== q.fromState) return false;
  if (q.toState && e.toState !== q.toState) return false;
  if (q.startTime && e.createdAt < q.startTime) return false;
  if (q.endTime && e.createdAt > q.endTime) return false;
  return true;
}

// ---------------------------------------------------------------------------
// OTLPSpan
// ---------------------------------------------------------------------------

export interface OTLPSpan {
  traceID: string;
  spanID: string;
  name: string;
  startTime: Date;
  endTime: Date;
  attrs?: Record<string, string>;
}

/** TraceEventsToSpans converts trace events to OTLP spans for export. */
export function traceEventsToSpans(events: TraceEvent[]): OTLPSpan[] {
  return events.map((e, i) => ({
    traceID: e.runID,
    spanID: `${e.runID}-${i}`,
    name: `${e.fromState} -> ${e.toState}`,
    startTime: e.createdAt,
    endTime: e.createdAt,
    attrs: {
      run_id: e.runID,
      from_state: e.fromState,
      to_state: e.toState,
      reason: e.reason ?? "",
      schema: e.schemaVersion,
    },
  }));
}
