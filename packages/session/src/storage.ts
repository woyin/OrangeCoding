/**
 * @module session-storage
 *
 * Session persistence layer — stores and retrieves agent sessions.
 *
 * Provides file-based session storage with atomic writes, session
 * listing, and cleanup of expired sessions.
 */
import { mkdir, rename, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { SessionId } from "@orangecoding/core";
import { newIOError } from "@orangecoding/core";
import type { Session } from "./session.js";

/**
 * First-line header of a JSONL session file. Subsequent lines are message
 * JSON objects. Keeping the header separate avoids scanning every message to
 * recover session metadata.
 */
/**
 * SessionHeader is the first JSON line of a JSONL session file.
 * Subsequent lines are message JSON objects. Keeping metadata in a header
 * avoids scanning every message to recover session-level fields.
 */
interface SessionHeader {
  id: string;
  metadata: Record<string, string>;
  token_usage: { prompt_tokens: number; completion_tokens: number; total_tokens: number } | null;
  created_at: string;
  updated_at: string;
  parent_id?: string;
}

/**
 * Persist a session as JSONL: one header line followed by one line per message.
 * Written via temp-file + rename so a crash never leaves a truncated session
 * file. Throws newIOError on any filesystem failure.
 */
export async function writeSession(dir: string, s: Session): Promise<void> {
  try {
    await mkdir(dir, { recursive: true });
  } catch (err) {
    throw newIOError(`session storage mkdir: ${(err as Error).message}`);
  }

  const path = join(dir, `${s.id.toString()}.jsonl`);
  const tmpPath = path + ".tmp";

  const header: SessionHeader = {
    id: s.id.toJSON(),
    metadata: s.metadata,
    token_usage: s.tokenUsage ? s.tokenUsage.toJSON() : null,
    created_at: s.createdAt.toISOString(),
    updated_at: s.updatedAt.toISOString(),
    parent_id: s.parentID?.toJSON(),
  };

  // Serialize: header first, then one JSON line per message. The trailing
  // newline makes the file safely append-able and line-oriented.
  const lines: string[] = [];
  lines.push(JSON.stringify(header));

  for (const msg of s.messages) {
    lines.push(JSON.stringify(msg.toJSON()));
  }

  const content = lines.join("\n") + "\n";

  try {
    await writeFile(tmpPath, content, "utf-8");
  } catch (err) {
    throw newIOError(`session storage write: ${(err as Error).message}`);
  }

  try {
    await rename(tmpPath, path);
  } catch (err) {
    throw newIOError(`session storage rename: ${(err as Error).message}`);
  }
}

/**
 * Read and parse a session JSONL file. The first line is the header; every
 * subsequent non-empty line is a message. Dynamic imports avoid a circular
 * dependency on the core package. Throws newIOError on read/parse failures.
 */
export async function readSession(dir: string, id: SessionId): Promise<Session> {
  const { SessionId: SessionIdClass, TokenUsage, Message } = await import("@orangecoding/core");

  const path = join(dir, `${id.toString()}.jsonl`);
  let content: string;
  try {
    content = await readFile(path, "utf-8");
  } catch (err) {
    throw newIOError(`session storage open: ${(err as Error).message}`);
  }

  // Split on newlines and drop empties so a trailing newline does not create a phantom record.
  const lines = content.split("\n").filter((line) => line.length > 0);

  if (lines.length === 0) {
    throw newIOError(`session storage: empty file ${path}`);
  }

  let header: SessionHeader;
  try {
    header = JSON.parse(lines[0]!);
  } catch (err) {
    throw newIOError(`session storage unmarshal header: ${(err as Error).message}`);
  }

  const sessionId = SessionIdClass.parse(header.id);
  const tokenUsage = header.token_usage
    ? new TokenUsage(header.token_usage.prompt_tokens, header.token_usage.completion_tokens, header.token_usage.total_tokens)
    : new TokenUsage(0, 0, 0);

  const messages: InstanceType<typeof Message>[] = [];
  for (let i = 1; i < lines.length; i++) {
    try {
      const msgJSON = JSON.parse(lines[i]!);
      messages.push(
        new Message(
          msgJSON.role,
          msgJSON.content ?? "",
          new Date(msgJSON.created_at),
          msgJSON.name,
          msgJSON.tool_calls,
          msgJSON.tool_call_id,
        ),
      );
    } catch (err) {
      throw newIOError(`session storage unmarshal message: ${(err as Error).message}`);
    }
  }

  // Import Session class dynamically to avoid circular dependency
  const { Session } = await import("./session.js");
  return new Session(
    sessionId,
    messages,
    header.metadata,
    tokenUsage,
    new Date(header.created_at),
    new Date(header.updated_at),
    header.parent_id ? SessionIdClass.parse(header.parent_id) : undefined,
  );
}
