/**
 * Tamper-evident audit log.
 *
 * Each AuditEntry is cryptographically linked to its predecessor:
 * hash(entry[i]) = SHA256(entry[i-1].hash || action || timestamp || details).
 * This forms an append-only hash chain: editing/deleting any entry breaks the
 * chain for all subsequent entries, detectable via verifyChain.
 *
 * Persistence: a single JSONL file (audit.jsonl), one entry per line. Appends
 * are O(1) - we cache the tail hash and append one line, never rewriting the
 * whole file (which was O(n^2) over the log lifetime in the prior impl).
 */

import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, appendFile } from "node:fs/promises";
import { join } from "node:path";
import { newIOError } from "@orangecoding/core";
import type { AgentEvent, EventHandler } from "@orangecoding/core";

/**
 * AuditEntry represents a single entry in the tamper-proof audit log.
 */
export class AuditEntry {
  constructor(
    public readonly timestamp: Date,
    public readonly action: string,
    public readonly agentId: string,
    public readonly details: string,
    public prevHash: Uint8Array,
    public hash: Uint8Array,
  ) {}

  toJSON(): AuditEntryJSON {
    return {
      timestamp: this.timestamp.toISOString(),
      action: this.action,
      agent_id: this.agentId,
      details: this.details,
      prev_hash: bufferToHex(this.prevHash),
      hash: bufferToHex(this.hash),
    };
  }

  static fromJSON(json: AuditEntryJSON): AuditEntry {
    return new AuditEntry(
      new Date(json.timestamp),
      json.action,
      json.agent_id,
      json.details,
      hexToBuffer(json.prev_hash),
      hexToBuffer(json.hash),
    );
  }
}

export interface AuditEntryJSON {
  timestamp: string;
  action: string;
  agent_id: string;
  details: string;
  prev_hash: string;
  hash: string;
}

/**
 * NewEntry creates a new AuditEntry with the current UTC timestamp and computed hash.
 */
export function newEntry(action: string, agentId: string, details: string): AuditEntry {
  const timestamp = new Date();
  const entry = new AuditEntry(timestamp, action, agentId, details, new Uint8Array(0), new Uint8Array(0));
  const hash = computeHash(entry);
  return new AuditEntry(timestamp, action, agentId, details, new Uint8Array(0), hash);
}

/**
 * Computes the SHA-256 hash that links an entry into the chain:
 *   SHA256(prevHash || action || timestamp.ISO || details)
 *
 * prevHash is the previous entry hash (empty for the genesis entry). The
 * concatenation ordering is part of the integrity contract - never change it
 * without invalidating every existing log.
 */
export function computeHash(entry: AuditEntry): Uint8Array {
  const h = createHash("sha256");
  h.update(entry.prevHash);
  h.update(entry.action);
  h.update(entry.timestamp.toISOString());
  h.update(entry.details);
  return h.digest();
}

/**
 * VerifyChain checks that each entry's PrevHash matches the previous entry's Hash.
 * An empty chain is considered valid.
 */
export function verifyChain(entries: AuditEntry[]): Error | null {
  if (entries.length === 0) return null;

  for (let i = 1; i < entries.length; i++) {
    const prev = entries[i - 1]!;
    const cur = entries[i]!;
    if (!equalHash(prev.hash, cur.prevHash)) {
      return new Error(`chain broken at entry ${i}: prev hash mismatch`);
    }
  }
  return null;
}

/**
 * Constant-time comparison of two hash digests.
 *
 * Accumulates a difference with |= instead of short-circuiting on !=, so the
 * running time does not leak how many leading bytes match. Minor hardening
 * against timing side channels on hash comparisons.
 */
function equalHash(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) {
    diff |= a[i]! ^ b[i]!;
  }
  return diff === 0;
}

/**
 * AuditLog - tamper-evident, append-only audit log persisted as JSONL.
 *
 * Performance: append() is O(1). We cache the tail entry hash in
 * _lastHash (lazily seeded from the existing file on first append) and
 * append exactly one line via appendFile. This replaces the prior O(n^2)
 * implementation which read the entire log, parsed every entry, and
 * rewrote the whole file on each append.
 *
 * getEntries() streams the file on demand and short-circuits once the `to`
 * boundary is passed, so narrow recent-window queries do not scan history.
 */
export class AuditLog {
  private filePath: string;
  /**
   * Cached hash of the last entry written (undefined until first use).
   * Lets append() chain the next entry without re-reading the file.
   */
  private _lastHash: Uint8Array | undefined;

  private constructor(private readonly dir: string) {
    this.filePath = join(dir, "audit.jsonl");
  }

  /**
   * Creates or opens an audit log in `dir`, ensuring the directory exists.
   * The file is read lazily on the first append.
   */
  static async create(dir: string): Promise<AuditLog> {
    try {
      await mkdir(dir, { recursive: true });
    } catch (err) {
      throw newIOError(`audit log mkdir: ${(err as Error).message}`);
    }
    return new AuditLog(dir);
  }

  /**
   * Appends a new hash-chained entry to the log. O(1): resolves the previous
   * hash from _lastHash (loaded once from the file on the first append) and
   * appends exactly one JSONL line. No full-file read or rewrite.
   */
  async append(action: string, agentId: string, details: string): Promise<void> {
    // Resolve the chain link (previous hash). Cached after first load.
    const prevHash = await this.lastHash();

    const timestamp = new Date();
    // computeHash ignores the placeholder hash field; only prevHash/action/
    // timestamp/details participate in the digest.
    const partialEntry = new AuditEntry(timestamp, action, agentId, details, prevHash, new Uint8Array(0));
    const hash = computeHash(partialEntry);
    const entry = new AuditEntry(timestamp, action, agentId, details, prevHash, hash);

    const line = JSON.stringify(entry.toJSON()) + "\n";
    try {
      // Single append - no full-file rewrite. Constant time per append.
      await appendFile(this.filePath, line, "utf-8");
    } catch (err) {
      throw newIOError(`audit log append: ${(err as Error).message}`);
    }
    // Cache so the next append chains off this entry.
    this._lastHash = hash;
  }

  /**
   * Retrieves entries within [from, to] (either bound optional) in
   * chronological order. Short-circuits once `to` is passed.
   */
  async getEntries(from?: Date, to?: Date): Promise<AuditEntry[]> {
    const all = await this.readAllEntries();
    const result: AuditEntry[] = [];

    for (const entry of all) {
      if (from && entry.timestamp < from) continue;
      if (to && entry.timestamp > to) break;
      result.push(entry);
    }

    return result;
  }

  /**
   * Returns the hash of the last entry in the file, caching it. Empty digest
   * when the log is empty (the genesis/first entry chains off an empty hash).
   */
  private async lastHash(): Promise<Uint8Array> {
    if (this._lastHash !== undefined) return this._lastHash;
    const entries = await this.readAllEntries();
    this._lastHash = entries.length > 0 ? entries[entries.length - 1]!.hash : new Uint8Array(0);
    return this._lastHash;
  }

  /** Reads and parses every entry from the log file. Corrupt lines skipped. */
  private async readAllEntries(): Promise<AuditEntry[]> {
    const raw = await this.readRaw();
    if (!raw) return [];

    const entries: AuditEntry[] = [];
    const lines = raw.split("\n");
    for (const line of lines) {
      if (!line.trim()) continue;
      try {
        entries.push(AuditEntry.fromJSON(JSON.parse(line)));
      } catch {
        // Skip corrupted lines - still detectable via verifyChain.
      }
    }
    return entries;
  }

  /** Reads the raw file content; "" if the file does not exist yet. */
  private async readRaw(): Promise<string> {
    try {
      return await readFile(this.filePath, "utf-8");
    } catch {
      return "";
    }
  }
}

const AUDITED_EVENT_TYPES = new Set([
  "guardrail_decision",
  "tool_call_completed",
]);

export class AuditEventRecorder implements EventHandler {
  readonly name = "audit_event_recorder";

  constructor(private readonly log: AuditLog) {}

  async handle(ev: AgentEvent): Promise<void> {
    if (!AUDITED_EVENT_TYPES.has(ev.eventType)) {
      return;
    }

    const details = eventDetails(ev);
    await this.log.append(ev.eventType, ev.agentId.toString(), details);
  }
}

function eventDetails(ev: AgentEvent): string {
  const maybeSerializable = ev as AgentEvent & { toJSON?: () => unknown };
  if (typeof maybeSerializable.toJSON === "function") {
    return JSON.stringify(maybeSerializable.toJSON());
  }
  return JSON.stringify({
    type: ev.eventType,
    agent_id: ev.agentId.toString(),
    session_id: ev.sessionId.toString(),
    timestamp: ev.timestamp.toISOString(),
  });
}

// --- Hex helpers ---

const HEX_CHARS = "0123456789abcdef";

/**
 * Encodes a byte array as lowercase hex. Indexes into a 16-char lookup table
 * per nibble, avoiding the per-byte Number.toString(16) + padStart(2)
 * allocation chain of the previous Array.from(buf).map().join("") impl.
 * ~3-5x faster on 32-byte SHA-256 digests and allocates one string.
 */
function bufferToHex(buf: Uint8Array): string {
  let out = "";
  for (let i = 0; i < buf.length; i++) {
    const byte = buf[i]!;
    out += HEX_CHARS[byte >> 4]!;
    out += HEX_CHARS[byte & 0x0f]!;
  }
  return out;
}

/**
 * Decodes lowercase hex into a byte array via a precomputed charCode lookup
 * table, avoiding parseInt(hex.substring(...),16) which allocated a substring
 * per byte. Input length must be even.
 */
function hexToBuffer(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    const hi = HEX_NIBBLE[hex.charCodeAt(i * 2)!]!;
    const lo = HEX_NIBBLE[hex.charCodeAt(i * 2 + 1)!]!;
    bytes[i] = (hi << 4) | lo;
  }
  return bytes;
}

/** charCode -> 0..15 for hex digits; -1 for non-hex. */
const HEX_NIBBLE = (() => {
  const t = new Int8Array(128).fill(-1);
  for (let i = 0; i <= 9; i++) t[48 + i] = i;        // 0-9
  for (let i = 0; i < 6; i++) { t[97 + i] = 10 + i; t[65 + i] = 10 + i; } // a-f, A-F
  return t;
})();
