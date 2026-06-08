import { PaneState, type PaneInfo } from "./pane.js";
import type { Backend } from "./backend.js";
import { runCommand } from "./backend.js";

/**
 * ZellijBackend implements Backend for the zellij terminal multiplexer.
 */
export class ZellijBackend implements Backend {
  private panes = new Map<string, PaneInfo>();

  name(): string {
    return "zellij";
  }

  isAvailable(): boolean {
    return !!process.env.ZELLIJ_SESSION_NAME;
  }

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

  async closePane(paneID: string): Promise<void> {
    await runCommand("zellij", ["action", "close-pane", "--pane-id", paneID]);
    this.panes.delete(paneID);
  }

  async sendText(paneID: string, text: string): Promise<void> {
    await runCommand("zellij", ["action", "write-chars", "--pane-id", paneID, text]);
  }

  async focusPane(paneID: string): Promise<void> {
    await runCommand("zellij", ["action", "focus-pane", "--pane-id", paneID]);
  }

  async captureOutput(paneID: string): Promise<string> {
    return runCommand("zellij", ["action", "dump-screen", "--pane-id", paneID]);
  }

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
