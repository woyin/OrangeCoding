import { mkdir, rename, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { SessionId } from "@orangecoding/core";
import { newIOError } from "@orangecoding/core";
import type { Session } from "./session.js";

/** Internal structure for the first line of a JSONL session file. */
interface SessionHeader {
  id: string;
  metadata: Record<string, string>;
  token_usage: { prompt_tokens: number; completion_tokens: number; total_tokens: number } | null;
  created_at: string;
  updated_at: string;
  parent_id?: string;
}

/**
 * WriteSession writes a session to a JSONL file atomically (write-to-temp + rename).
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
 * ReadSession reads a session from a JSONL file in the given directory.
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
