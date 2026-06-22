import { PaneState, type PaneInfo } from "./pane.js";
import type { Backend } from "./backend.js";
import { runCommand } from "./backend.js";

/**
 * ZellijBackend implements Backend for the zellij terminal multiplexer.
 */
export class ZellijBackend implements Backend {
  private panes = new Map<string, PaneInfo>();

  /** name returns the backend identifier used for backend selection. */
  name(): string {
    return "zellij";
  }

  /** isAvailable reports whether we are inside a zellij session (ZELLIJ_SESSION_NAME set). */
  isAvailable(): boolean {
    return !!process.env.ZELLIJ_SESSION_NAME;
  }

  /**
   * createPane opens a new zellij pane running the command and records it locally.
   * The pane ID is parsed from zellij output or synthesized on failure.
   */
  async createPane(name: string, command: string): Promise<PaneInfo> {
    const args = ["action", "new-pane", "--name", name, "--", "sh", "-c", command];
    const out = await runCommand("zellij", args);

    const paneID = parseZellijPaneID(out, name);
    const info: PaneInfo = {
      id: paneID,
      name,
      state: PaneState.Running,
      createdAt: new Date(),
      backend: "zellij",
    };

    this.panes.set(paneID, info);
    return info;
  }

  /** closePane closes the pane via zellij action and drops the local record. */
  async closePane(paneID: string): Promise<void> {
    await runCommand("zellij", ["action", "close-pane", "--pane-id", paneID]);
    this.panes.delete(paneID);
  }

  /** sendText writes characters into the pane via zellij write-chars. */
  async sendText(paneID: string, text: string): Promise<void> {
    await runCommand("zellij", ["action", "write-chars", "--pane-id", paneID, text]);
  }

  /** focusPane switches keyboard focus to the named pane via focus-pane. */
  async focusPane(paneID: string): Promise<void> {
    await runCommand("zellij", ["action", "focus-pane", "--pane-id", paneID]);
  }

  /** captureOutput dumps the current screen contents of the pane via dump-screen. */
  async captureOutput(paneID: string): Promise<string> {
    return runCommand("zellij", ["action", "dump-screen", "--pane-id", paneID]);
  }

  /** listPanes returns the locally-tracked panes created by this backend instance. */
  async listPanes(): Promise<PaneInfo[]> {
    return [...this.panes.values()];
  }
}

/**
 * parseZellijPaneID extracts a pane identifier from zellij command output.
 * Falls back to a generated ID if parsing fails.
 */
function parseZellijPaneID(output: string, name: string): string {
  const trimmed = output.trim();
  if (trimmed) {
    for (const line of trimmed.split("\n")) {
      const l = line.trim();
      if (l) return l;
    }
  }
  return `zellij-${name}-${Date.now()}`;
}
