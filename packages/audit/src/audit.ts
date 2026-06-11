import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, rename, writeFile } from "node:fs/promises";
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
 * ComputeHash returns SHA256(PrevHash + Action + Timestamp.RFC3339Nano + Details).
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

function equalHash(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

/**
 * AuditLog is a tamper-proof audit log backed by the filesystem (JSONL files).
 * (In Go this used bbolt; in TS we use JSONL files for portability.)
 */
export class AuditLog {
  private filePath: string;

  private constructor(private readonly dir: string) {
    this.filePath = join(dir, "audit.jsonl");
  }

  /**
   * NewAuditLog creates or opens an audit log in the given directory.
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
   * Append creates a new audit entry linked to the last entry in the log,
   * and saves it to the log file.
   */
  async append(action: string, agentId: string, details: string): Promise<void> {
    const entries = await this.readAllEntries();
    let prevHash = new Uint8Array(0);
    if (entries.length > 0) {
      prevHash = new Uint8Array(entries[entries.length - 1]!.hash);
    }

    const timestamp = new Date();
    const partialEntry = new AuditEntry(timestamp, action, agentId, details, prevHash, new Uint8Array(0));
    const hash = computeHash(partialEntry);
    const entry = new AuditEntry(timestamp, action, agentId, details, prevHash, hash);

    const line = JSON.stringify(entry.toJSON()) + "\n";

    // Atomic write: write to temp then rename.
    const tmpPath = this.filePath + ".tmp";
    try {
      const existingContent = await this.readRaw();
      await writeFile(tmpPath, existingContent + line, "utf-8");
      await rename(tmpPath, this.filePath);
    } catch (err) {
      throw newIOError(`audit log append: ${(err as Error).message}`);
    }
  }

  /**
   * GetEntries retrieves audit entries within the given time range [from, to].
   * If from is undefined, it starts from the beginning.
   * If to is undefined, it includes all entries after from.
   * Entries are returned in chronological order.
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

  /** Read all entries from the log file. */
  private async readAllEntries(): Promise<AuditEntry[]> {
    const raw = await this.readRaw();
    if (!raw) return [];

    const entries: AuditEntry[] = [];
    for (const line of raw.split("\n")) {
      if (!line.trim()) continue;
      try {
        entries.push(AuditEntry.fromJSON(JSON.parse(line)));
      } catch {
        // Skip corrupted lines.
      }
    }
    return entries;
  }

  /** Read the raw file content. */
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

function bufferToHex(buf: Uint8Array): string {
  return Array.from(buf)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function hexToBuffer(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
  }
  return bytes;
}
