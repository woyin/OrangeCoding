import type { Message, Role } from "@orangecoding/core";

/**
 * StyleDescriptor describes a terminal style with foreground/background colors and bold flag.
 * In the Go code this was a lipgloss.Style; here we keep it as a simple descriptor
 * that can be mapped to any rendering backend (ink, blessed, etc.) later.
 */
export interface StyleDescriptor {
  foreground?: string;
  background?: string;
  bold?: boolean;
}

/**
 * Theme holds the colour/style palette for the TUI.
 */
export class Theme {
  constructor(
    public readonly name: string,
    public readonly primary: StyleDescriptor,
    public readonly secondary: StyleDescriptor,
    public readonly success: StyleDescriptor,
    public readonly error: StyleDescriptor,
    public readonly dim: StyleDescriptor,
    public readonly border: StyleDescriptor,
    public readonly input: StyleDescriptor,
  ) {}

  /**
   * Apply a style descriptor to a string. Returns ANSI-escaped text if
   * isTTY is true; otherwise returns the plain text.
   */
  static applyStyle(style: StyleDescriptor, text: string): string {
    // Simple ANSI escape code rendering.
    const codes: string[] = [];
    if (style.bold) codes.push("1");
    if (style.foreground) codes.push(fgColor(style.foreground));
    if (style.background) codes.push(bgColor(style.background));
    if (codes.length === 0) return text;
    return `\x1b[${codes.join(";")}m${text}\x1b[0m`;
  }

  /**
   * StatusBar renders the bottom status bar with mode, session, and token info.
   */
  statusBar(mode: string, sessionID: string, tokens: number): string {
    const modeStr = Theme.applyStyle(this.primary, `mode=${mode}`);
    let sessionStr = "";
    if (sessionID) {
      sessionStr = Theme.applyStyle(this.secondary, ` session=${sessionID}`);
    }
    const tokenStr = Theme.applyStyle(this.dim, ` tokens=${tokens}`);
    return `${modeStr}${sessionStr}${tokenStr}`;
  }

  /**
   * ChatMessage formats a single Message for display in the chat area.
   */
  chatMessage(msg: Message): string {
    let roleLabel: string;
    switch (msg.role as string) {
      case "system":
        roleLabel = Theme.applyStyle(this.dim, "[system]");
        break;
      case "user":
        roleLabel = Theme.applyStyle(this.primary, "[user]");
        break;
      case "assistant":
        roleLabel = Theme.applyStyle(this.success, "[assistant]");
        break;
      case "tool":
        roleLabel = Theme.applyStyle(this.secondary, "[tool]");
        break;
      default:
        roleLabel = Theme.applyStyle(this.dim, "[?]");
    }
    return `${roleLabel} ${msg.content}`;
  }
}

/** DarkTheme is the default dark colour scheme. */
export const DarkTheme = new Theme(
  "dark",
  { foreground: "#7D56F4", bold: true },            // primary
  { foreground: "#9B9B9B" },                         // secondary
  { foreground: "#04B575" },                         // success
  { foreground: "#FF5F87" },                         // error
  { foreground: "#626262" },                         // dim
  { foreground: "#4A4A4A" },                         // border
  { foreground: "#FAFAFA", background: "#3C3C3C" },  // input
);

/** LightTheme is a light colour scheme. */
export const LightTheme = new Theme(
  "light",
  { foreground: "#7D56F4", bold: true },            // primary
  { foreground: "#666666" },                         // secondary
  { foreground: "#04B575" },                         // success
  { foreground: "#CC0000" },                         // error
  { foreground: "#AAAAAA" },                         // dim
  { foreground: "#CCCCCC" },                         // border
  { foreground: "#1A1A1A", background: "#E0E0E0" },  // input
);

// --- ANSI colour helpers ---

function fgColor(hex: string): string {
  const rgb = hexToRGB(hex);
  return `38;2;${rgb[0]};${rgb[1]};${rgb[2]}`;
}

function bgColor(hex: string): string {
  const rgb = hexToRGB(hex);
  return `48;2;${rgb[0]};${rgb[1]};${rgb[2]}`;
}

function hexToRGB(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.substring(0, 2), 16),
    parseInt(h.substring(2, 4), 16),
    parseInt(h.substring(4, 6), 16),
  ];
}
