/**
 * BashTool - executes shell commands.
 *
 * Ported from modules/tools/bash_tool.go.
 * Fixed: streaming output limiting, single timeout mechanism.
 */

import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import type { Tool, ToolMetadata } from "./tool.js";
import { ToolError } from "./tool.js";
import type { SecurityPolicy } from "./security.js";

// ---------------------------------------------------------------------------
// BashTool
// ---------------------------------------------------------------------------

interface BashArgs {
  command: string;
  timeout?: number;
}

const MAX_STDOUT_BYTES = 1024 * 1024;  // 1MB
const MAX_STDERR_BYTES = 256 * 1024;   // 256KB

/**
 * BashTool executes shell commands with streaming output limiting.
 */
export class BashTool implements Tool {
  private readonly _policy: SecurityPolicy | null;
  private readonly _params: Record<string, unknown>;

  /**
   * Creates a new BashTool. If policy is non-null, commands are checked
   * against the security policy before execution.
   */
  constructor(policy: SecurityPolicy | null) {
    this._policy = policy;
    this._params = {
      type: "object",
      properties: {
        command: { type: "string" },
        timeout: { type: "integer" },
      },
      required: ["command"],
    };
  }

  name(): string { return "bash"; }
  description(): string { return "Execute a shell command and return its output."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return { isReadOnly: false, isConcurrencySafe: false, isDestructive: true, isEnabled: true, maxUses: 0, softLimit: 0 }; }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as BashArgs;

    if (!args.command) {
      throw new ToolError("invalid_params", "command is required");
    }

    // Security check
    if (this._policy !== null && !this._policy.isAllowed(args.command)) {
      throw new ToolError("security_violation", "command is blocked by security policy: " + args.command);
    }

    const timeoutMs = args.timeout && args.timeout > 0 ? args.timeout : 30_000;

    return new Promise<string>((resolve, reject) => {
      const child = spawn("sh", ["-c", args.command], {
        stdio: ["pipe", "pipe", "pipe"],
      });

      let stdoutBytes = 0;
      let stderrBytes = 0;
      let stdoutTruncated = false;
      let stderrTruncated = false;
      const stdoutChunks: string[] = [];
      const stderrChunks: string[] = [];

      // Stream stdout with byte limiting
      const stdoutRl = createInterface({ input: child.stdout! });
      stdoutRl.on("line", (line) => {
        if (stdoutTruncated) return;
        const lineBytes = Buffer.byteLength(line, "utf-8") + 1; // +1 for newline
        if (stdoutBytes + lineBytes > MAX_STDOUT_BYTES) {
          stdoutTruncated = true;
          stdoutChunks.push("[output truncated]\n");
          return;
        }
        stdoutChunks.push(line + "\n");
        stdoutBytes += lineBytes;
      });

      // Stream stderr with byte limiting
      const stderrRl = createInterface({ input: child.stderr! });
      stderrRl.on("line", (line) => {
        if (stderrTruncated) return;
        const lineBytes = Buffer.byteLength(line, "utf-8") + 1;
        if (stderrBytes + lineBytes > MAX_STDERR_BYTES) {
          stderrTruncated = true;
          stderrChunks.push("[stderr truncated]\n");
          return;
        }
        stderrChunks.push(line + "\n");
        stderrBytes += lineBytes;
      });

      // Single timeout mechanism
      const timer = setTimeout(() => {
        child.kill("SIGTERM");
      }, timeoutMs);

      child.on("close", (code) => {
        clearTimeout(timer);
        stdoutRl.close();
        stderrRl.close();

        let output = stdoutChunks.join("");
        const stderrOutput = stderrChunks.join("");
        if (stderrOutput) {
          output += "\n" + stderrOutput;
        }

        if (code !== 0 && !output) {
          output = `Process exited with code ${code}`;
        }

        resolve(output);
      });

      child.on("error", (err) => {
        clearTimeout(timer);
        stdoutRl.close();
        stderrRl.close();
        reject(new ToolError("execution_error", err.message));
      });
    });
  }
}
