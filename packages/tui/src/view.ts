/**
 * @module tui-view
 *
 * TUI rendering engine — converts application state to terminal output.
 *
 * The view layer renders the terminal UI by composing layout primitives:
 * - Header bar with session info
 * - Conversation panel with message history
 * - Status indicators (thinking, tool execution, etc.)
 * - Input area for user messages
 *
 * Rendering is pure: the same state always produces the same output.
 */
import type { Message } from "@orangecoding/core";
import { Model } from "./model.js";
import { Theme } from "./theme.js";

/**
 * view renders the full TUI layout as a string.
 */
export function view(m: Model): string {
  if (m.quitting) {
    return "Goodbye!\n";
  }

  let w = m.width;
  let h = m.height;

  // Minimum usable dimensions
  if (w < 20) w = 80;
  if (h < 10) h = 24;

  // Reserve 1 row for status bar, 2 rows for input area.
  let chatHeight = h - 3;
  if (chatHeight < 1) chatHeight = 1;

  let sidebarWidth = 0;
  if (m.sidebar) {
    sidebarWidth = 20;
  }
  let chatWidth = w - sidebarWidth;
  if (chatWidth < 20) chatWidth = 20;

  // Render sidebar
  let sidebarStr = "";
  if (m.sidebar) {
    const sidebarContent =
      Theme.applyStyle(m.theme.dim, "Sessions\n\n") +
      Theme.applyStyle(m.theme.secondary, "  (no sessions)");
    sidebarStr = renderBox(sidebarContent, sidebarWidth, chatHeight);
  }

  // Render chat messages
  const chatContent = renderChatArea(m, chatWidth, chatHeight);
  const chatStr = renderBox(chatContent, chatWidth, chatHeight);

  // Combine sidebar + chat horizontally
  const mainArea = joinHorizontal(sidebarStr, chatStr);

  // Input area
  const inputStr = Theme.applyStyle(m.theme.input, `> ${m.input}`);

  // Status bar
  const statusStr = m.theme.statusBar(m.mode, "", 0);

  // Combine vertically
  return joinVertical(mainArea, inputStr, statusStr);
}

/**
 * renderChatArea renders all messages into a string.
 */
function renderChatArea(m: Model, width: number, height: number): string {
  const lines: string[] = [];

  for (const msg of m.messages) {
    const rendered = m.theme.chatMessage(msg);
    for (const line of rendered.split("\n")) {
      lines.push(line.length > width ? line.substring(0, width) : line);
    }
  }

  // Pad or trim to fit height
  while (lines.length < height) {
    lines.push("");
  }
  if (lines.length > height) {
    return lines.slice(lines.length - height).join("\n");
  }

  return lines.join("\n");
}

/** Simple box rendering with border. */
function renderBox(content: string, width: number, _height: number): string {
  const lines = content.split("\n");
  const padded = lines.map((line) => line.padEnd(width)).join("\n");
  return padded;
}

/** Join strings side by side. */
function joinHorizontal(left: string, right: string): string {
  if (!left) return right;
  if (!right) return left;

  const leftLines = left.split("\n");
  const rightLines = right.split("\n");
  const maxLines = Math.max(leftLines.length, rightLines.length);
  const result: string[] = [];

  for (let i = 0; i < maxLines; i++) {
    const l = (leftLines[i] ?? "").padEnd(leftLines[0]?.length ?? 0);
    const r = rightLines[i] ?? "";
    result.push(l + r);
  }

  return result.join("\n");
}

/** Join strings top to bottom. */
function joinVertical(...parts: string[]): string {
  return parts.join("\n");
}
