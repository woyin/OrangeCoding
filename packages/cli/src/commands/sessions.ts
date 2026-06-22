/**
 * @module cli-sessions
 *
 * CLI command for managing agent sessions.
 *
 * Provides:
 * - Listing saved sessions
 * - Resuming previous sessions
 * - Deleting session data
 * - Exporting session transcripts
 */

import * as os from "node:os";
import * as path from "node:path";
import { SessionManager } from "@orangecoding/session";

/**
 * Lists all saved sessions.
 */
export async function runSessions(): Promise<void> {
  const home = os.homedir() || ".";
  const sessionDir = path.join(home, ".orangecoding", "sessions");
  const manager = new SessionManager(sessionDir);

  // Load all sessions; if the session directory is missing or empty, print
  // a friendly message instead of an error.
  try {
    const sessions = await manager.list();

    if (sessions.length === 0) {
      console.log("No saved sessions found.");
      console.log("Sessions are saved automatically after each agent run.");
      return;
    }

    console.log(`\x1b[36m⚡ Saved Sessions\x1b[0m (${sessions.length} total)\n`);

    for (const session of sessions) {
      const age = timeSince(session.updatedAt);
      const msgCount = session.messages.length;
      const lastMsg = session.messages.length > 0
        ? session.messages[session.messages.length - 1]!
        : null;
      const preview = lastMsg
        ? lastMsg.content.slice(0, 60).replace(/\n/g, " ")
        : "(empty)";

      console.log(`  \x1b[33m${session.id.toString()}\x1b[0m`);
      console.log(`    Updated: ${session.updatedAt.toISOString()} (${age})`);
      console.log(`    Messages: ${msgCount}`);
      console.log(`    Last: ${preview}...`);
      if (session.metadata["task"]) {
        console.log(`    Task: ${session.metadata["task"]}`);
      }
      console.log();
    }

    console.log("\x1b[90mUsage: orangecoding resume <session-id>\x1b[0m");
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`Error listing sessions: ${msg}`);
  }
}

/** timeSince formats a Date as a human-readable relative age (e.g. "5m ago"). */
function timeSince(date: Date): string {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}
