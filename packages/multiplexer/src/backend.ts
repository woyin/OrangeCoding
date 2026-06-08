import type { PaneInfo } from "./pane.js";
import type { MultiplexerConfig } from "./config.js";
import { ZellijBackend } from "./zellij.js";
import { TmuxBackend } from "./tmux.js";

/**
 * Backend abstracts terminal multiplexer operations.
 * Implementations wrap CLI commands for zellij or tmux.
 */
export interface Backend {
  /** Name returns "zellij", "tmux", or "none". */
  name(): string;

  /** IsAvailable checks whether this backend can be used in the current environment. */
  isAvailable(): boolean;

  /** CreatePane spawns a new pane running the given command. */
  createPane(name: string, command: string): Promise<PaneInfo>;

  /** ClosePane terminates the named pane. */
  closePane(paneID: string): Promise<void>;

  /** SendText writes text into the pane's stdin (as if typed). */
  sendText(paneID: string, text: string): Promise<void>;

  /** FocusPane brings the pane to foreground focus. */
  focusPane(paneID: string): Promise<void>;

  /** CaptureOutput reads the current visible buffer of the pane. */
  captureOutput(paneID: string): Promise<string>;

  /** ListPanes returns all panes managed by this backend. */
  listPanes(): Promise<PaneInfo[]>;
}

/**
 * DetectBackend returns the best available backend.
 * Priority: zellij > tmux > null.
 */
export function detectBackend(): Backend | null {
  const z = new ZellijBackend();
  if (z.isAvailable()) return z;

  const t = new TmuxBackend();
  if (t.isAvailable()) return t;

  return null;
}

/**
 * newBackendFromConfig returns a backend based on the config preference.
 */
export function newBackendFromConfig(cfg: MultiplexerConfig): Backend | null {
  switch (cfg.preferredBackend) {
    case "zellij":
      return new ZellijBackend();
    case "tmux":
      return new TmuxBackend();
    case "auto":
    case "":
      return detectBackend();
    default:
      return null;
  }
}

/**
 * runCommand executes a command and returns its combined stdout+stderr.
 */
export async function runCommand(
  command: string,
  args: string[],
  options?: { signal?: AbortSignal; timeout?: number },
): Promise<string> {
  const { execFile } = await import("node:child_process");
  const { promisify } = await import("node:util");
  const execFileAsync = promisify(execFile);

  try {
    const { stdout, stderr } = await execFileAsync(command, args, {
      signal: options?.signal,
      timeout: options?.timeout,
    });
    return stdout + stderr;
  } catch (err) {
    const execErr = err as Error & { stdout?: string; stderr?: string };
    const output = (execErr.stdout ?? "") + (execErr.stderr ?? "");
    throw new Error(`${command}: ${execErr.message} (output: ${output})`);
  }
}
