import { PaneState, type PaneInfo } from "./pane.js";
import type { Backend } from "./backend.js";
import { runCommand } from "./backend.js";

/**
 * TmuxBackend implements Backend for the tmux terminal multiplexer.
 */
export class TmuxBackend implements Backend {
  private panes = new Map<string, PaneInfo>();

  /** name returns the backend identifier used for backend selection. */
  name(): string {
    return "tmux";
  }

  /** isAvailable reports whether we are inside a tmux session (TMUX env set). */
  isAvailable(): boolean {
    return !!process.env.TMUX;
  }

  /**
   * createPane splits the current tmux window horizontally and runs the command.
   * Falls back to a synthesized pane ID if tmux output parsing fails.
   */
  async createPane(name: string, command: string): Promise<PaneInfo> {
    // -P prints pane ID, -F sets format, -h splits horizontally
    const args = ["split-window", "-h", "-P", "-F", "#{pane_id}", "-t", name, "sh", "-c", command];
    const out = await runCommand("tmux", args);

    let paneID = parseTmuxPaneID(out);
    if (!paneID) {
      paneID = `tmux-${name}-${Date.now()}`;
    }

    const info: PaneInfo = {
      id: paneID,
      name,
      state: PaneState.Running,
      createdAt: new Date(),
      backend: "tmux",
    };

    this.panes.set(paneID, info);
    return info;
  }

  /** closePane kills the named tmux pane and drops it from the local registry. */
  async closePane(paneID: string): Promise<void> {
    await runCommand("tmux", ["kill-pane", "-t", paneID]);
    const p = this.panes.get(paneID);
    if (p) {
      this.panes.delete(paneID);
    }
  }

  /** sendText sends a line of text into the pane followed by Enter via send-keys. */
  async sendText(paneID: string, text: string): Promise<void> {
    await runCommand("tmux", ["send-keys", "-t", paneID, text, "Enter"]);
  }

  /** focusPane brings the pane to the foreground via select-pane. */
  async focusPane(paneID: string): Promise<void> {
    await runCommand("tmux", ["select-pane", "-t", paneID]);
  }

  /** captureOutput returns the visible pane content via capture-pane -p. */
  async captureOutput(paneID: string): Promise<string> {
    return runCommand("tmux", ["capture-pane", "-t", paneID, "-p"]);
  }

  /** listPanes returns the locally-tracked panes created by this backend instance. */
  async listPanes(): Promise<PaneInfo[]> {
    return [...this.panes.values()];
  }
}

/**
 * parseTmuxPaneID extracts the pane ID from tmux output (e.g., "%5").
 */
function parseTmuxPaneID(output: string): string {
  const trimmed = output.trim();
  if (!trimmed) return "";

  for (const line of trimmed.split("\n")) {
    const l = line.trim();
    if (l.startsWith("%")) {
      return l;
    }
  }
  return trimmed;
}
