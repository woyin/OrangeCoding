/**
 * BashTool - executes shell commands.
 *
 * Ported from modules/tools/bash_tool.go.
 */

import { execFile } from "node:child_process";
import type { Tool, ToolMetadata } from "./tool.js";
import { ToolError } from "./tool.js";
import type { SecurityPolicy } from "./security.js";

// ---------------------------------------------------------------------------
// limitedWrite helper
// ---------------------------------------------------------------------------

class LimitedBuffer {
  private _buf: string[] = [];
  private _len = 0;

  constructor(private readonly _max: number) {}

  append(data: string): void {
    if (this._len + data.length > this._max) {
      const remaining = this._max - this._len;
      if (remaining > 0) {
        this._buf.push(data.slice(0, remaining));
        this._len += remaining;
      }
      // Silently drop the rest
      return;
    }
    this._buf.push(data);
    this._len += data.length;
  }

  toString(): string {
    return this._buf.join("");
  }

  get length(): number {
    return this._len;
  }
}

// ---------------------------------------------------------------------------
// BashTool
// ---------------------------------------------------------------------------

interface BashArgs {
  command: string;
  timeout?: number;
}

/**
 * BashTool executes shell commands.
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
  metadata(): ToolMetadata { return { isReadOnly: false, isConcurrencySafe: false, isDestructive: true, isEnabled: true }; }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as BashArgs;

    if (!args.command) {
      throw new ToolError("invalid_params", "command is required");
    }

    // Security check
    if (this._policy !== null && !this._policy.isAllowed(args.command)) {
      throw new ToolError("security_violation", "command is blocked by security policy: " + args.command);
    }

    const timeoutMs = args.timeout && args.timeout > 0 ? args.timeout : undefined;

    return new Promise<string>((resolve) => {
      const execArgs = ["-c", args.command];

      const child = execFile("sh", execArgs, {
        timeout: timeoutMs,
        maxBuffer: 1024 * 1024,
        killSignal: "SIGTERM",
      }, (error, stdout, stderr) => {
        const stdoutLimited = new LimitedBuffer(1024 * 1024);  // 1MB
        const stderrLimited = new LimitedBuffer(256 * 1024);   // 256KB

        stdoutLimited.append(stdout ?? "");
        stderrLimited.append(stderr ?? "");

        let output = stdoutLimited.toString();
        if (stderrLimited.length > 0) {
          output += "\n" + stderrLimited.toString();
        }

        if (error) {
          if (output === "") {
            output = error.message;
          }
          resolve(output);
          return;
        }

        resolve(output);
      });

      // Prevent the process from blocking the event loop indefinitely
      if (timeoutMs) {
        setTimeout(() => {
          child.kill("SIGTERM");
        }, timeoutMs);
      }
    });
  }
}
