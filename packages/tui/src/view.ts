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
 * view：把 TUI 应用状态（Model）渲染为完整布局字符串。
 * 纯函数：相同状态恒定产出相同输出。
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
 * renderChatArea 渲染消息列表为字符串，裁剪到只显示最近 height 行。
 *
 * 性能优化：原实现先渲染“全部”消息、对整体 split("\n")，
 * 再 slice 到最后 height 行——长会话下每帧都把整段历史渲染并切片，
 * 浪费显著。改为从最新消息向前回溯，只收集够 height 行就停，
 * 早期消息不再被渲染/拆分。
 */
function renderChatArea(m: Model, width: number, height: number): string {
  // 从尾部向前累积，最多保留 height 行。
  const collected: string[] = [];
  for (let mi = m.messages.length - 1; mi >= 0 && collected.length < height; mi--) {
    const rendered = m.theme.chatMessage(m.messages[mi]!);
    const renderedLines = rendered.split("\n");
    // 逐行截宽，并从尾部向前 unshift 进 collected（保持时间正序）
    for (let li = renderedLines.length - 1; li >= 0 && collected.length < height; li--) {
      const line = renderedLines[li]!;
      collected.unshift(line.length > width ? line.substring(0, width) : line);
    }
  }

  // 不足 height 行时在顶部补空行（旧实现是在尾部 pad，渲染时视觉等价）
  while (collected.length < height) {
    collected.push("");
  }

  return collected.join("\n");
}

/** renderBox：把内容按宽度 padEnd 对齐（简化版盒子渲染）。 */
function renderBox(content: string, width: number, _height: number): string {
  const lines = content.split("\n");
  const padded = lines.map((line) => line.padEnd(width)).join("\n");
  return padded;
}

/** joinHorizontal：把两段文本左右并排拼接（按行对齐）。 */
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

/** joinVertical：把多段文本自上而下拼接。 */
function joinVertical(...parts: string[]): string {
  return parts.join("\n");
}
