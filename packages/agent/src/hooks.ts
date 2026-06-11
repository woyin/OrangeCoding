/**
 * HookManager manages and executes hooks at various lifecycle points.
 * Ported from modules/agent/hooks.go.
 *
 * Note: Shell command hooks require a Node.js runtime with child_process
 * available. In environments without subprocess support, hooks are no-ops.
 */

import { execFile } from "node:child_process";

export type HookPoint = "pre_tool_call" | "post_tool_call" | "pre_sampling" | "post_sampling";

export interface Hook {
  point: HookPoint;
  command: string;
}

const HOOK_TIMEOUT_MS = 10_000;

/** Sanitize data before passing to hook stdin. Removes null bytes and limits length. */
function sanitizeForStdin(data: string): string {
  // Remove null bytes
  let sanitized = data.replace(/\0/g, "");
  // Limit to 1MB
  if (sanitized.length > 1024 * 1024) {
    sanitized = sanitized.slice(0, 1024 * 1024);
  }
  return sanitized;
}

export class HookManager {
  private _hooks: Map<HookPoint, Hook[]>;

  constructor() {
    this._hooks = new Map();
  }

  /** Register adds a hook to be executed at the specified hook point. */
  register(hook: Hook): void {
    const existing = this._hooks.get(hook.point) ?? [];
    existing.push(hook);
    this._hooks.set(hook.point, existing);
  }

  /** Run executes all hooks registered for the given point.
   *  If any hook fails, execution continues but the first error is returned. */
  async run(_signal: AbortSignal | undefined, point: HookPoint, data: string): Promise<Error | null> {
    const hooks = this._hooks.get(point);
    if (!hooks || hooks.length === 0) return null;

    const sanitizedData = sanitizeForStdin(data);

    let firstErr: Error | null = null;
    for (const hook of hooks) {
      try {
        await new Promise<void>((resolve, reject) => {
          const proc = execFile("sh", ["-c", hook.command], {
            timeout: HOOK_TIMEOUT_MS,
            maxBuffer: 1024 * 1024,
          }, (err) => {
            if (err) reject(err);
            else resolve();
          });
          if (proc.stdin) {
            proc.stdin.write(sanitizedData);
            proc.stdin.end();
          }
        });
      } catch (err) {
        if (firstErr === null) {
          firstErr = err instanceof Error ? err : new Error(String(err));
        }
      }
    }
    return firstErr;
  }
}
