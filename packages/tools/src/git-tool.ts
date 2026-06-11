/**
 * GitTool — git operations for coding agents.
 *
 * Provides: status, diff, log, blame, show, branch, stash.
 * Comparable to claude code / opencode / pi mono git integration.
 */

import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { Tool, ToolMetadata } from "./tool.js";
import { ToolError } from "./tool.js";
import { readOnlyMetadata } from "./tool.js";

const execFileAsync = promisify(execFile);

const MAX_OUTPUT = 256 * 1024; // 256KB
const DEFAULT_TIMEOUT = 30_000;

// ---------------------------------------------------------------------------
// GitTool
// ---------------------------------------------------------------------------

interface GitArgs {
  /** The git sub-command to run (status, diff, log, blame, show, branch, stash, remote). */
  action: string;
  /** File path(s) for the operation. */
  path?: string;
  /** For log: max number of commits. For diff: revision spec. For blame: line range. */
  revision?: string;
  /** For log: max count. */
  count?: number;
  /** Working directory for the git command. */
  cwd?: string;
}

export class GitTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        action: {
          type: "string",
          description: "Git sub-command: status, diff, log, blame, show, branch, stash, remote",
        },
        path: { type: "string", description: "File path(s) for the operation" },
        revision: { type: "string", description: "Revision spec, commit hash, or branch" },
        count: { type: "integer", description: "Max number of results (for log)" },
        cwd: { type: "string", description: "Working directory (defaults to repo root)" },
      },
      required: ["action"],
    };
  }

  name(): string { return "git"; }
  description(): string {
    return "Run git operations: status, diff, log, blame, show, branch, stash, remote. " +
      "Use for understanding code history, reviewing changes, and managing branches.";
  }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as GitArgs;
    if (!args.action) {
      throw new ToolError("invalid_params", "action is required");
    }

    const cmdArgs = this.buildArgs(args);
    const cwd = args.cwd || ".";

    try {
      const { stdout, stderr } = await execFileAsync("git", cmdArgs, {
        cwd,
        timeout: DEFAULT_TIMEOUT,
        maxBuffer: MAX_OUTPUT,
        killSignal: "SIGTERM",
      });

      let output = stdout;
      if (stderr && stderr.trim()) {
        output += (output ? "\n" : "") + stderr;
      }
      if (!output.trim()) {
        output = this.emptyMessage(args.action);
      }
      return output;
    } catch (err: unknown) {
      const e = err as { stdout?: string; stderr?: string; message?: string; killed?: boolean };
      if (e.killed) {
        throw new ToolError("execution_error", "git command timed out");
      }
      // Git returns non-zero for some valid cases (e.g., diff with no changes)
      const stderr = (e.stderr || "").trim();
      const stdout = (e.stdout || "").trim();
      if (stdout) return stdout;
      if (stderr.includes("not a git repository")) {
        throw new ToolError("execution_error", "not a git repository");
      }
      if (stderr.includes("bad revision")) {
        throw new ToolError("invalid_params", "bad revision: " + (args.revision || ""));
      }
      throw new ToolError("execution_error", stderr || e.message || "git command failed");
    }
  }

  private buildArgs(args: GitArgs): string[] {
    const cmd: string[] = [];

    switch (args.action) {
      case "status":
        cmd.push("status", "--short", "--branch");
        if (args.path) cmd.push("--", args.path);
        break;

      case "diff":
        cmd.push("diff");
        if (args.revision) cmd.push(args.revision);
        cmd.push("--stat", "--patch");
        if (args.path) cmd.push("--", args.path);
        break;

      case "log": {
        const count = args.count && args.count > 0 ? args.count : 20;
        cmd.push("log", `--max-count=${count}`, "--oneline", "--decorate", "--graph");
        if (args.revision) cmd.push(args.revision);
        if (args.path) cmd.push("--", args.path);
        break;
      }

      case "blame":
        cmd.push("blame", "--line-porcelain");
        if (args.revision) cmd.push(args.revision);
        if (!args.path) {
          throw new ToolError("invalid_params", "path is required for blame");
        }
        cmd.push("--", args.path);
        break;

      case "show":
        cmd.push("show", "--stat", "--patch");
        if (args.revision) cmd.push(args.revision);
        else cmd.push("HEAD");
        if (args.path) cmd.push("--", args.path);
        break;

      case "branch":
        cmd.push("branch", "--list", "--verbose", "--all");
        break;

      case "stash":
        cmd.push("stash", "list");
        break;

      case "remote":
        cmd.push("remote", "-v");
        break;

      case "tag":
        cmd.push("tag", "--list", "--sort=-creatordate");
        break;

      case "shortlog": {
        const count = args.count && args.count > 0 ? args.count : 20;
        cmd.push("shortlog", "-sn", `--max-count=${count}`);
        if (args.revision) cmd.push(args.revision);
        break;
      }

      default:
        throw new ToolError("invalid_params",
          `unknown action "${args.action}". Supported: status, diff, log, blame, show, branch, stash, remote, tag, shortlog`);
    }

    return cmd;
  }

  private emptyMessage(action: string): string {
    switch (action) {
      case "status": return "Working tree clean — no uncommitted changes.";
      case "diff": return "No differences found.";
      case "log": return "No commits found.";
      case "blame": return "No blame information available.";
      case "branch": return "No branches found.";
      case "stash": return "No stashes found.";
      case "remote": return "No remotes configured.";
      case "tag": return "No tags found.";
      default: return "No output.";
    }
  }
}
