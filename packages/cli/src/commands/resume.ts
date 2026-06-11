/**
 * Handles the `resume` command.
 *
 * Lists and resumes saved sessions. When resuming, the session's
 * conversation history is loaded and the user can continue the
 * interaction from where they left off.
 *
 * This is equivalent to `orangecoding launch --resume <session-id>`.
 */

import * as os from "node:os";
import * as path from "node:path";
import { runLaunch } from "./launch.js";
import { listSavedSessions } from "./resume-helper.js";

/**
 * Run the resume command.
 * @param sessionID - Optional session ID to resume. If omitted, lists sessions.
 */
export async function runResume(sessionID?: string): Promise<void> {
  const sessionDir = getSessionDir();

  if (!sessionID) {
    // List saved sessions
    await listSessions(sessionDir);
    return;
  }

  // Delegate to launch with --resume flag
  await runLaunch(undefined, false, undefined, sessionID);
}

async function listSessions(sessionDir: string): Promise<void> {
  try {
    const sessions = await listSavedSessions(sessionDir);

    if (sessions.length === 0) {
      console.log("No saved sessions found.");
      console.log("Sessions are saved automatically after each agent run.");
      return;
    }

    console.log("\x1b[36m⚡ Resumable Sessions\x1b[0m (" + sessions.length + " total)\n");

    for (const s of sessions) {
      const age = timeSince(s.updatedAt);
      const taskPreview = s.task ? ` | Task: ${s.task.slice(0, 40)}` : "";

      console.log(`  \x1b[33m${s.id.toString()}\x1b[0m`);
      console.log(`    Updated: ${s.updatedAt.toISOString()} (${age}) | Messages: ${s.messageCount}${taskPreview}`);
      console.log();
    }

    console.log("\x1b[90mUsage: orangecoding resume <session-id>\x1b[0m");
    console.log("\x1b[90m   or: orangecoding launch -r <session-id> -p \"continue with...\"\x1b[0m");
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`Error listing sessions: ${msg}`);
  }
}

function timeSince(date: Date): string {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}

function getSessionDir(): string {
  const home = os.homedir() || ".";
  return path.join(home, ".orangecoding", "sessions");
}
