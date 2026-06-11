/**
 * App wraps the TUI model and provides the terminal rendering loop.
 * Uses raw ANSI escape codes for rendering — no external TUI framework needed.
 */

import type { Message } from "@orangecoding/core";
import { Model } from "./model.js";
import { update, type TuiMsg } from "./update.js";
import { view } from "./view.js";

/**
 * Terminal abstraction for I/O. Allows mocking in tests.
 */
export interface Terminal {
  write(data: string): void;
  getSize(): { cols: number; rows: number };
  setRawMode(enable: boolean): void;
  onKeypress(handler: (buf: Buffer) => void): void;
  onResize(handler: () => void): void;
  removeKeypress(handler: (buf: Buffer) => void): void;
  removeResize(handler: () => void): void;
  isTTY: boolean;
}

/**
 * Real terminal backed by process.stdin/stdout.
 */
export class RawTerminal implements Terminal {
  private keypressHandler: ((buf: Buffer) => void) | null = null;
  private resizeHandler: (() => void) | null = null;

  get isTTY(): boolean {
    return !!(process.stdout.isTTY && process.stdin.isTTY);
  }

  write(data: string): void {
    process.stdout.write(data);
  }

  getSize(): { cols: number; rows: number } {
    return {
      cols: process.stdout.columns ?? 80,
      rows: process.stdout.rows ?? 24,
    };
  }

  setRawMode(enable: boolean): void {
    if (process.stdin.isTTY) {
      process.stdin.setRawMode(enable);
    }
  }

  onKeypress(handler: (buf: Buffer) => void): void {
    this.keypressHandler = handler;
    process.stdin.resume();
    process.stdin.on("data", handler);
  }

  onResize(handler: () => void): void {
    this.resizeHandler = handler;
    process.stdout.on("resize", handler);
  }

  removeKeypress(handler: (buf: Buffer) => void): void {
    process.stdin.removeListener("data", handler);
    this.keypressHandler = null;
  }

  removeResize(handler: () => void): void {
    process.stdout.removeListener("resize", handler);
    this.resizeHandler = null;
  }
}

/** ANSI escape sequences */
const ESC = {
  clearScreen: "\x1b[2J",
  clearLine: "\x1b[2K",
  cursorHome: "\x1b[H",
  cursorHide: "\x1b[?25l",
  cursorShow: "\x1b[?25h",
  cursorTo: (row: number, col: number) => `\x1b[${row};${col}H`,
  altScreenOn: "\x1b[?1049h",
  altScreenOff: "\x1b[?1049l",
  reset: "\x1b[0m",
};

/**
 * Parse a keypress buffer into a TuiMsg KeyMsg.
 */
export function parseKeypress(buf: Buffer): TuiMsg {
  const str = buf.toString();
  const code = buf[0]!;

  // Ctrl+C
  if (code === 3) {
    return { type: "key", key: "ctrl+c" };
  }

  // Escape sequences
  if (code === 27) {
    // Plain Escape
    if (buf.length === 1) {
      return { type: "key", key: "escape" };
    }
    // Arrow keys and other sequences — ignore for now
    return { type: "key", key: "escape" };
  }

  // Enter
  if (code === 13 || code === 10) {
    return { type: "key", key: "enter" };
  }

  // Tab
  if (code === 9) {
    return { type: "key", key: "tab" };
  }

  // Backspace (127 or 8)
  if (code === 127 || code === 8) {
    return { type: "key", key: "backspace" };
  }

  // Printable characters
  if (str.length > 0 && code >= 32) {
    return { type: "key", key: "unknown", runes: str };
  }

  return { type: "key", key: "unknown" };
}

/**
 * App is the main TUI application.
 */
export class App {
  model: Model;
  currentStream: string;
  onSubmit: ((text: string) => void) | undefined;
  onCommand: ((cmd: string) => void) | undefined;
  private terminal: Terminal | undefined;
  private renderTimer: ReturnType<typeof setTimeout> | null = null;
  private running: boolean = false;

  constructor(opts?: { terminal?: Terminal }) {
    this.model = new Model();
    this.currentStream = "";
    this.terminal = opts?.terminal;
  }

  /**
   * Send a message to the app (equivalent to Bubble Tea's Update).
   * Intercepts "enter" to call onSubmit callback.
   */
  send(msg: TuiMsg): void {
    // Intercept enter to fire onSubmit
    if (msg.type === "key" && msg.key === "enter") {
      const text = this.model.input.trim();
      const prevMessages = this.model.messages.length;
      this.model = update(this.model, msg);

      // If the message was not a slash command (messages count increased by exactly 1
      // and it's a user message), fire onSubmit
      if (text && this.model.messages.length > prevMessages) {
        const lastMsg = this.model.messages[this.model.messages.length - 1];
        if (lastMsg && (lastMsg.role as string) === "user" && this.onSubmit) {
          this.onSubmit(text);
        }
      }
      return;
    }

    this.model = update(this.model, msg);
  }

  /**
   * Render the current view as a string.
   */
  render(): string {
    return view(this.model);
  }

  /**
   * Append streaming text from the AI provider.
   */
  appendStream(chunk: string): void {
    this.currentStream += chunk;
  }

  /**
   * Clear the streaming buffer.
   */
  clearStream(): void {
    this.currentStream = "";
  }

  /**
   * Set tool execution status.
   */
  setToolStatus(toolName: string, success: boolean): void {
    const icon = success ? "✅" : "❌";
    this.send({ type: "status", status: `${toolName} ${icon}` });
  }

  /**
   * Run starts the TUI application loop with real terminal rendering.
   * Returns when the user quits (Ctrl+C, /quit, etc.).
   */
  async run(): Promise<void> {
    const term = this.terminal ?? new RawTerminal();

    if (!term.isTTY) {
      // Non-TTY fallback: just return immediately
      return;
    }

    this.terminal = term;
    this.running = true;

    // Initialize terminal size
    const size = term.getSize();
    this.send({ type: "window_size", width: size.cols, height: size.rows });

    // Enter alternate screen buffer
    term.write(ESC.altScreenOn);
    term.write(ESC.cursorHide);
    term.write(ESC.clearScreen);

    // Initial render
    this.drawScreen();

    // Handle resize
    const onResize = () => {
      const s = term.getSize();
      this.send({ type: "window_size", width: s.cols, height: s.rows });
      this.drawScreen();
    };
    term.onResize(onResize);

    // Handle keypress
    const onKeypress = (buf: Buffer) => {
      const msg = parseKeypress(buf);
      this.send(msg);

      if (this.model.quitting) {
        this.cleanup();
        return;
      }

      this.drawScreen();
    };
    term.onKeypress(onKeypress);

    // Set raw mode for character-by-character input
    term.setRawMode(true);

    // Wait for quit signal
    return new Promise<void>((resolve) => {
      const checkQuit = () => {
        if (this.model.quitting || !this.running) {
          resolve();
          return;
        }
        setTimeout(checkQuit, 50);
      };
      checkQuit();
    });
  }

  /**
   * Stop the TUI loop programmatically.
   */
  stop(): void {
    this.running = false;
    this.cleanup();
  }

  /**
   * Draw the full screen using ANSI codes.
   */
  private drawScreen(): void {
    const term = this.terminal;
    if (!term) return;

    const output = this.render();
    // Move cursor to home and clear screen, then write
    term.write(ESC.cursorHome);
    term.write(output);

    // Show streaming content if any
    if (this.currentStream) {
      term.write("\n");
      term.write(this.currentStream);
    }

    // Clear any leftover lines below
    const size = term.getSize();
    const lines = output.split("\n").length;
    for (let i = lines; i < size.rows; i++) {
      term.write(ESC.cursorTo(i, 1) + ESC.clearLine);
    }
  }

  /**
   * Clean up terminal state.
   */
  private cleanup(): void {
    const term = this.terminal;
    if (!term) return;

    term.setRawMode(false);
    term.write(ESC.cursorShow);
    term.write(ESC.altScreenOff);
    this.running = false;
  }
}
