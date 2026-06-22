/**
 * MarkdownRenderer：把 Markdown 内容渲染成带样式的终端输出。
 * 用 ANSI 转义码实现富文本，不依赖外部库。
 */

const BOLD_ON = "\x1b[1m";
const BOLD_OFF = "\x1b[22m";
const ITALIC_ON = "\x1b[3m";
const ITALIC_OFF = "\x1b[23m";
const DIM_ON = "\x1b[2m";
const DIM_OFF = "\x1b[22m";
const RESET = "\x1b[0m";
const CYAN = "\x1b[36m";
const YELLOW = "\x1b[33m";
const GREEN = "\x1b[32m";
const GRAY = "\x1b[90m";
const MAGENTA = "\x1b[35m";

// --- Pre-compiled regexes -------------------------------------------------
// These patterns are applied to every line on every render(). Compiling them
// once at module load (rather than per-call inside render) avoids re-running
// the regex compiler on each streamed chunk - a significant win for the TUI
// hot path, which re-renders on every stream delta.
const RE_HR = /^(?:-{3,}|_{3,}|\*{3,})$/;
const RE_H1 = /^# (.+)$/;
const RE_H2 = /^## (.+)$/;
const RE_H3 = /^### (.+)$/;
const RE_FENCE = /^\s*```/;
const RE_BULLET = /^(\s*)[-*+]\s+(.+)$/;
const RE_NUMBERED = /^(\s*)(\d+)\.\s+(.+)$/;

export class MarkdownRenderer {
  private readonly wordWrap: number;

  constructor(wordWrap = 80) {
    this.wordWrap = wordWrap;
  }

  /**
   * Render converts the given Markdown content to styled output.
   */
  render(content: string): string {
    if (content === "") return "";

    const lines = content.split("\n");
    const output: string[] = [];
    let inCodeBlock = false;
    let codeBlockLang = "";
    const codeLines: string[] = [];

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i]!;

      // Code block toggle
      if (line.trimStart().startsWith("```")) {
        if (!inCodeBlock) {
          inCodeBlock = true;
          codeBlockLang = line.trimStart().slice(3).trim();
          codeLines.length = 0;
          continue;
        } else {
          inCodeBlock = false;
          // Render collected code block
          output.push(this.renderCodeBlock(codeLines, codeBlockLang));
          codeBlockLang = "";
          continue;
        }
      }

      if (inCodeBlock) {
        codeLines.push(line);
        continue;
      }

      // Horizontal rule (--- / ___ / ***)
      if (RE_HR.test(line.trim())) {
        output.push(GRAY + "─".repeat(Math.min(this.wordWrap, 40)) + RESET);
        continue;
      }

      // Headers (use pre-compiled patterns).
      const h1 = line.match(RE_H1);
      if (h1) {
        output.push(BOLD_ON + CYAN + h1[1] + RESET);
        continue;
      }

      const h2 = line.match(RE_H2);
      if (h2) {
        output.push(BOLD_ON + MAGENTA + h2[1] + RESET);
        continue;
      }

      const h3 = line.match(RE_H3);
      if (h3) {
        output.push(BOLD_ON + h3[1] + RESET);
        continue;
      }

      // Blockquote
      if (line.startsWith("> ")) {
        const text = this.renderInline(line.slice(2));
        output.push(GRAY + "│ " + RESET + text);
        continue;
      }

      // Bullet list (-, *, +)
      const bullet = line.match(RE_BULLET);
      if (bullet) {
        const indent = bullet[1] ?? "";
        const text = this.renderInline(bullet[2]!);
        output.push(indent + GREEN + "• " + RESET + text);
        continue;
      }

      // Numbered list (1. 2. 3.)
      const numbered = line.match(RE_NUMBERED);
      if (numbered) {
        const indent = numbered[1] ?? "";
        const num = numbered[2]!;
        const text = this.renderInline(numbered[3]!);
        output.push(indent + GREEN + num + ". " + RESET + text);
        continue;
      }

      // Regular paragraph line
      output.push(this.renderInline(line));
    }

    // Handle unclosed code block
    if (inCodeBlock && codeLines.length > 0) {
      output.push(this.renderCodeBlock(codeLines, codeBlockLang));
    }

    return output.join("\n");
  }

  /**
   * Render inline markdown elements (bold, italic, code, links).
   */
  private renderInline(text: string): string {
    // Inline code: `code`
    text = text.replace(/`([^`]+)`/g, (_, code) => {
      return DIM_ON + CYAN + code + RESET;
    });

    // Bold: **text** or __text__
    text = text.replace(/\*\*(.+?)\*\*/g, (_, inner) => {
      return BOLD_ON + inner + BOLD_OFF;
    });
    text = text.replace(/__(.+?)__/g, (_, inner) => {
      return BOLD_ON + inner + BOLD_OFF;
    });

    // Italic: *text* or _text_
    text = text.replace(/(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)/g, (_, inner) => {
      return ITALIC_ON + inner + ITALIC_OFF;
    });
    text = text.replace(/(?<!_)_(?!_)(.+?)(?<!_)_(?!_)/g, (_, inner) => {
      return ITALIC_ON + inner + ITALIC_OFF;
    });

    // Links: [text](url)
    text = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, linkText, url) => {
      return CYAN + linkText + RESET + GRAY + " (" + url + ")" + RESET;
    });

    return text;
  }

  /**
   * Render a fenced code block.
   */
  private renderCodeBlock(lines: string[], lang: string): string {
    const header = lang ? ` ${lang} ` : " code ";
    const width = Math.min(this.wordWrap, 60);
    const headerLine = GRAY + "┌─" + header + "─".repeat(Math.max(0, width - header.length - 3)) + RESET;
    const footerLine = GRAY + "└" + "─".repeat(width - 1) + RESET;

    const body = lines.map((line) => {
      const padded = line.length > width - 2 ? line.slice(0, width - 2) : line;
      return GRAY + "│ " + RESET + YELLOW + padded + RESET;
    });

    return [headerLine, ...body, footerLine].join("\n");
  }
}
