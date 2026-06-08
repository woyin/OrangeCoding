import type { Message } from "@orangecoding/core";
import { DarkTheme, type Theme } from "./theme.js";

/**
 * Model is the TUI application state model.
 * (In Go this was a Bubble Tea tea.Model; in TS the actual rendering
 * framework is pluggable -- ink, blessed, etc.)
 */
export class Model {
  messages: Message[];
  input: string;
  width: number;
  height: number;
  sidebar: boolean;
  status: string;
  mode: string; // "normal", "plan", "goal", "ultra"
  theme: Theme;
  error: Error | undefined;
  quitting: boolean;

  constructor(opts?: Partial<Model>) {
    this.messages = opts?.messages ?? [];
    this.input = opts?.input ?? "";
    this.width = opts?.width ?? 80;
    this.height = opts?.height ?? 24;
    this.sidebar = opts?.sidebar ?? false;
    this.status = opts?.status ?? "ready";
    this.mode = opts?.mode ?? "normal";
    this.theme = opts?.theme ?? DarkTheme;
    this.error = opts?.error;
    this.quitting = opts?.quitting ?? false;
  }
}
