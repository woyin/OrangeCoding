import { mkdir, readdir, rm } from "node:fs/promises";
import { join } from "node:path";
import { SessionId, TokenUsage, type TokenUsage as TokenUsageType, newIOError } from "@orangecoding/core";
import type { Message } from "@orangecoding/core";
import { writeSession, readSession } from "./storage.js";

/**
 * Session represents a conversation session with messages and metadata.
 */
export class Session {
  constructor(
    public readonly id: SessionId,
    public messages: Message[],
    public metadata: Record<string, string>,
    public tokenUsage: TokenUsage,
    public createdAt: Date,
    public updatedAt: Date,
    public parentID?: SessionId,
  ) {}
}

/**
 * SessionManager manages session persistence on disk.
 */
export class SessionManager {
  constructor(private readonly storageDir: string) {}

  /**
   * Create creates a new session with a random ID, empty messages, and current timestamps.
   * The session is NOT persisted to disk until Update is called.
   */
  create(): Session {
    const now = new Date();
    return new Session(
      SessionId.create(),
      [],
      {},
      new TokenUsage(0, 0, 0),
      now,
      now,
    );
  }

  /**
   * Get loads a session from disk by its ID.
   */
  async get(id: SessionId): Promise<Session> {
    return readSession(this.storageDir, id);
  }

  /**
   * Update persists the session to disk and updates UpdatedAt.
   */
  async update(s: Session): Promise<void> {
    (s as { updatedAt: Date }).updatedAt = new Date();
    return writeSession(this.storageDir, s);
  }

  /**
   * Delete removes a session file from disk.
   */
  async delete(id: SessionId): Promise<void> {
    const path = join(this.storageDir, `${id.toString()}.jsonl`);
    try {
      await rm(path);
    } catch (err) {
      throw newIOError(`session delete: ${(err as Error).message}`);
    }
  }

  /**
   * List returns all sessions sorted by UpdatedAt descending (most recent first).
   * Skips unreadable or corrupted session files.
   */
  async list(): Promise<Session[]> {
    try {
      await mkdir(this.storageDir, { recursive: true });
    } catch (err) {
      throw newIOError(`session list mkdir: ${(err as Error).message}`);
    }

    let entries: string[];
    try {
      entries = await readdir(this.storageDir);
    } catch (err) {
      throw newIOError(`session list readdir: ${(err as Error).message}`);
    }

    const sessions: Session[] = [];

    for (const entry of entries) {
      if (!entry.endsWith(".jsonl")) continue;

      const baseName = entry.slice(0, -".jsonl".length);
      let id: SessionId;
      try {
        id = SessionId.parse(baseName);
      } catch {
        continue;
      }

      try {
        const s = await readSession(this.storageDir, id);
        sessions.push(s);
      } catch {
        continue;
      }
    }

    sessions.sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime());

    return sessions;
  }
}
