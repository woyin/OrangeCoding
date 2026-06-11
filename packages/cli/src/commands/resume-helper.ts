/**
 * Resume helper — restores a saved session's conversation for continued use.
 *
 * This module provides the core logic for loading a saved session from disk
 * and reconstructing the conversation messages so they can be loaded into
 * an AgentContext for continued agent execution.
 */

import type { SessionId, Message } from "@orangecoding/core";
import { SessionManager } from "@orangecoding/session";

export interface RestoredSession {
  /** The session ID. */
  sessionId: SessionId;
  /** All messages from the saved session, in order. */
  messages: Message[];
  /** Session metadata (e.g. task name). */
  metadata: Record<string, string>;
}

/**
 * Restores a saved session from the given directory.
 * Throws if the session is not found or corrupted.
 */
export async function restoreConversation(
  sessionDir: string,
  sessionId: SessionId,
): Promise<RestoredSession> {
  const manager = new SessionManager(sessionDir);
  const session = await manager.get(sessionId);

  return {
    sessionId: session.id,
    messages: [...session.messages],
    metadata: { ...session.metadata },
  };
}

/**
 * Lists all saved sessions in the given directory.
 */
export async function listSavedSessions(
  sessionDir: string,
): Promise<Array<{ id: SessionId; updatedAt: Date; messageCount: number; task?: string }>> {
  const manager = new SessionManager(sessionDir);
  const sessions = await manager.list();

  return sessions.map((s) => ({
    id: s.id,
    updatedAt: s.updatedAt,
    messageCount: s.messages.length,
    task: s.metadata["task"],
  }));
}
