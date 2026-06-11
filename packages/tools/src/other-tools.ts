/**
 * Other tools: Fetch, Python, Calc, Task, and stub tools.
 *
 * Ported from modules/tools/other_tools.go.
 */

import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { lookup } from "node:dns";

const lookupAsync = promisify(lookup);
import { mkdtemp, writeFile as fsWriteFile, unlink } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import type { Tool, ToolMetadata } from "./tool.js";
import { ToolError } from "./tool.js";
import { defaultMetadata, readOnlyMetadata } from "./tool.js";

// ---------------------------------------------------------------------------
// FetchTool
// ---------------------------------------------------------------------------

interface FetchArgs {
  url: string;
  method?: string;
}

const MAX_FETCH_SIZE = 100 * 1024; // 100KB

const BLOCKED_HOST_PREFIXES: string[] = [
  "169.254.", // cloud metadata
  "10.",
  "172.16.", "172.17.", "172.18.", "172.19.",
  "172.20.", "172.21.", "172.22.", "172.23.",
  "172.24.", "172.25.", "172.26.", "172.27.",
  "172.28.", "172.29.", "172.30.", "172.31.",
  "192.168.",
  "0.0.0.0",
  "localhost",
  "127.",
];

/**
 * FetchTool makes HTTP requests and returns the response body.
 */
export class FetchTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        url: { type: "string" },
        method: { type: "string" },
      },
      required: ["url"],
    };
  }

  name(): string { return "fetch"; }
  description(): string { return "Fetch content from a URL."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return defaultMetadata(); }

  async execute(ctx: unknown, input: unknown): Promise<string> {
    const args = input as FetchArgs;

    if (!args.url) {
      throw new ToolError("invalid_params", "url is required");
    }

    // Block requests to internal/private networks.
    const lowerUrl = args.url.toLowerCase();
    if (!lowerUrl.startsWith("http://") && !lowerUrl.startsWith("https://")) {
      throw new ToolError("security_violation", "only http/https URLs are allowed");
    }

    // Extract host from URL and check against blocked prefixes.
    const host = extractHost(args.url);
    for (const prefix of BLOCKED_HOST_PREFIXES) {
      if (host.startsWith(prefix)) {
        throw new ToolError("security_violation", "access to internal/private network addresses is blocked");
      }
    }

    // DNS resolution check to prevent DNS rebinding attacks
    try {
      const resolved = await lookupAsync(host);
      const resolvedIp = resolved.address.toLowerCase();
      for (const prefix of BLOCKED_HOST_PREFIXES) {
        if (resolvedIp.startsWith(prefix)) {
          throw new ToolError("security_violation", "access to internal/private network addresses is blocked");
        }
      }
    } catch (err) {
      if (err instanceof ToolError) throw err;
      // DNS lookup failed - allow the request to proceed (will fail at fetch time)
    }

    const method = args.method || "GET";

    // Set up AbortController for timeout (30s default)
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 30_000);

    try {
      const resp = await fetch(args.url, {
        method,
        signal: controller.signal,
      });

      const buffer = await resp.arrayBuffer();
      const bytes = new Uint8Array(buffer);

      let result: string;
      if (bytes.length > MAX_FETCH_SIZE) {
        // Find a safe UTF-8 truncation point.
        let truncateAt = MAX_FETCH_SIZE;
        const decoder = new TextDecoder("utf-8", { fatal: false });
        result = decoder.decode(bytes.slice(0, truncateAt)) + "\n... (truncated)";
      } else {
        const decoder = new TextDecoder("utf-8", { fatal: false });
        result = decoder.decode(bytes);
      }

      return result;
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    } finally {
      clearTimeout(timeout);
    }
  }
}

// ---------------------------------------------------------------------------
// PythonTool
// ---------------------------------------------------------------------------

interface PythonArgs {
  code: string;
  timeout?: number;
}

/**
 * Executes Python code by writing it to a temp file and running python3.
 */
export class PythonTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        code: { type: "string" },
      },
      required: ["code"],
    };
  }

  name(): string { return "python"; }
  description(): string { return "Execute Python code."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return defaultMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as PythonArgs;

    if (!args.code) {
      throw new ToolError("invalid_params", "code is required");
    }

    // Enforce a default timeout of 30 seconds.
    const timeoutMs = (args.timeout && args.timeout > 0) ? args.timeout : 30_000;

    // Write code to a temp file
    const tmpDir = await mkdtemp(join(tmpdir(), "python-"));
    const tmpFile = join(tmpDir, "script.py");

    try {
      await fsWriteFile(tmpFile, args.code, "utf-8");

      return new Promise<string>((resolve) => {
        execFile("python3", [tmpFile], {
          timeout: timeoutMs,
          maxBuffer: 1024 * 1024,
          killSignal: "SIGTERM",
        }, (error, stdout, stderr) => {
          let output = (stdout ?? "");
          if (stderr && stderr.length > 0) {
            output += "\n" + stderr;
          }

          if (error) {
            resolve(output || error.message);
            return;
          }

          resolve(output);
        });
      });
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    } finally {
      // Clean up temp file
      try {
        await unlink(tmpFile);
      } catch {
        // ignore
      }
    }
  }
}

// ---------------------------------------------------------------------------
// CalcTool
// ---------------------------------------------------------------------------

interface CalcArgs {
  expression: string;
}

/**
 * Evaluates arithmetic expressions.
 */
export class CalcTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        expression: { type: "string" },
      },
      required: ["expression"],
    };
  }

  name(): string { return "calc"; }
  description(): string { return "Evaluate an arithmetic expression."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as CalcArgs;

    if (!args.expression) {
      throw new ToolError("invalid_params", "expression is required");
    }

    const result = evalExpression(args.expression.trim());
    return `${args.expression} = ${formatNumber(result)}`;
  }
}

// ---------------------------------------------------------------------------
// CalcTool internals - recursive descent parser
// ---------------------------------------------------------------------------

function formatNumber(f: number): string {
  if (Number.isInteger(f)) {
    return f.toString();
  }
  return f.toString();
}

function evalExpression(expr: string): number {
  const parser = new ExprParser(expr);
  return parser.parse();
}

class ExprParser {
  private tokens: string[] = [];
  private pos = 0;

  constructor(expr: string) {
    let buf = "";
    for (const ch of expr) {
      switch (ch) {
        case " ":
        case "\t":
        case "\n":
          if (buf.length > 0) {
            this.tokens.push(buf);
            buf = "";
          }
          break;
        case "+":
        case "-":
        case "*":
        case "/":
        case "(":
        case ")":
          if (buf.length > 0) {
            this.tokens.push(buf);
            buf = "";
          }
          this.tokens.push(ch);
          break;
        default:
          buf += ch;
      }
    }
    if (buf.length > 0) {
      this.tokens.push(buf);
    }
  }

  private peek(): string {
    if (this.pos >= this.tokens.length) return "";
    return this.tokens[this.pos]!;
  }

  private next(): string {
    const t = this.peek();
    this.pos++;
    return t;
  }

  parse(): number {
    return this.parseAddSub();
  }

  private parseAddSub(): number {
    let left = this.parseMulDiv();
    while (true) {
      const op = this.peek();
      if (op !== "+" && op !== "-") break;
      this.next();
      const right = this.parseMulDiv();
      if (op === "+") {
        left += right;
      } else {
        left -= right;
      }
    }
    return left;
  }

  private parseMulDiv(): number {
    let left = this.parsePrimary();
    while (true) {
      const op = this.peek();
      if (op !== "*" && op !== "/") break;
      this.next();
      const right = this.parsePrimary();
      if (op === "*") {
        left *= right;
      } else {
        if (right === 0) {
          throw new ToolError("execution_error", "division by zero");
        }
        left /= right;
      }
    }
    return left;
  }

  private parsePrimary(): number {
    const tok = this.peek();
    if (tok === "(") {
      this.next();
      const val = this.parseAddSub();
      if (this.peek() !== ")") {
        throw new ToolError("execution_error", `expected ')', got "${this.peek()}"`);
      }
      this.next();
      return val;
    }

    // Handle unary minus
    if (tok === "-") {
      this.next();
      const val = this.parsePrimary();
      return -val;
    }

    // Number
    this.next();
    const f = Number(tok);
    if (isNaN(f)) {
      throw new ToolError("execution_error", `expected number, got "${tok}"`);
    }
    return f;
  }
}

// ---------------------------------------------------------------------------
// TaskTool
// ---------------------------------------------------------------------------

interface TaskArgs {
  action: string;
  id?: string;
  description?: string;
  subagent_type?: string;
  scope?: string;
  expected_output?: string;
}

interface TaskEntry {
  id: string;
  description: string;
  status: string;
}

/**
 * Manages an in-memory task list.
 */
export class TaskTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _tasks = new Map<string, TaskEntry>();
  private _nextID = 0;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        action: { type: "string" },
        id: { type: "string" },
        description: { type: "string" },
        subagent_type: { type: "string", description: "Suggested sub-agent role such as explorer, reviewer, implementer, verifier, or documenter." },
        scope: { type: "string", description: "Files, modules, or problem boundary the sub-agent should own." },
        expected_output: { type: "string", description: "Concrete artifact or answer the sub-agent should return." },
      },
      required: ["action"],
    };
  }

  name(): string { return "task"; }
  description(): string { return "Manage an in-memory task list."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as TaskArgs;

    switch (args.action) {
      case "create": {
        this._nextID++;
        let id = args.id || "";
        if (!id) {
          id = `task-${this._nextID}`;
        }
        const entry: TaskEntry = {
          id,
          description: args.description || "",
          status: "pending",
        };
        this._tasks.set(id, entry);
        return `Task created: ${id} - ${args.description || ""}`;
      }

      case "update": {
        if (!args.id) {
          throw new ToolError("invalid_params", "id is required for update");
        }
        const task = this._tasks.get(args.id);
        if (!task) {
          throw new ToolError("not_found", "task not found: " + args.id);
        }
        if (args.description) {
          task.description = args.description;
        }
        return `Task updated: ${args.id}`;
      }

      case "list": {
        if (this._tasks.size === 0) {
          return "No tasks.";
        }
        const lines: string[] = [];
        for (const task of this._tasks.values()) {
          lines.push(`${task.id}\t${task.status}\t${task.description}`);
        }
        return lines.join("\n");
      }

      case "delete": {
        if (!args.id) {
          throw new ToolError("invalid_params", "id is required for delete");
        }
        if (!this._tasks.has(args.id)) {
          throw new ToolError("not_found", "task not found: " + args.id);
        }
        this._tasks.delete(args.id);
        return `Task deleted: ${args.id}`;
      }

      case "delegate": {
        if (!args.description) {
          throw new ToolError("invalid_params", "description is required for delegate");
        }
        const subagentType = args.subagent_type || "generalist";
        const lines: string[] = [];
        lines.push("Sub-agent delegation");
        lines.push("type: " + subagentType);
        lines.push("task: " + args.description);
        if (args.scope) {
          lines.push("scope: " + args.scope);
        }
        if (args.expected_output) {
          lines.push("expected_output: " + args.expected_output);
        }
        lines.push("coordination: keep ownership narrow, return evidence, changed files, verification commands, and unresolved risks.");
        return lines.join("\n");
      }

      default:
        throw new ToolError("invalid_params", "unknown action: " + args.action);
    }
  }
}

// ---------------------------------------------------------------------------
// BrowserTool — fetches a URL and extracts readable text from HTML
// ---------------------------------------------------------------------------

interface BrowserArgs {
  url: string;
  max_length?: number;
}

/**
 * BrowserTool fetches a web page and returns human-readable text content.
 * Strips HTML tags, scripts, and styles, keeping only visible text.
 */
export class BrowserTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        url: { type: "string", description: "The URL to fetch and read." },
        max_length: { type: "integer", description: "Maximum characters to return (default 8000)." },
      },
      required: ["url"],
    };
  }

  name(): string { return "browser"; }
  description(): string {
    return "Fetch a web page and extract readable text content. " +
      "Use for reading documentation, articles, or any web page. " +
      "Returns cleaned text with HTML tags removed.";
  }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as BrowserArgs;

    if (!args.url) {
      throw new ToolError("invalid_params", "url is required");
    }

    // Validate URL scheme
    const lowerUrl = args.url.toLowerCase();
    if (!lowerUrl.startsWith("http://") && !lowerUrl.startsWith("https://")) {
      throw new ToolError("security_violation", "only http/https URLs are allowed");
    }

    // Block internal network access
    const host = extractHost(args.url);
    for (const prefix of BLOCKED_HOST_PREFIXES) {
      if (host.startsWith(prefix)) {
        throw new ToolError("security_violation", "access to internal/private network addresses is blocked");
      }
    }

    const maxLength = args.max_length && args.max_length > 0 ? args.max_length : 8000;

    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 30_000);

    try {
      const resp = await fetch(args.url, {
        method: "GET",
        signal: controller.signal,
        headers: {
          "User-Agent": "Mozilla/5.0 (compatible; OrangeCoding/1.0)",
          "Accept": "text/html,application/xhtml+xml,text/plain,*/*",
        },
      });

      if (!resp.ok) {
        return `HTTP ${resp.status} ${resp.statusText}`;
      }

      const contentType = resp.headers.get("content-type") || "";
      const raw = await resp.text();

      // If it's not HTML, return as-is (truncated)
      if (!contentType.includes("html") && !contentType.includes("xhtml")) {
        return raw.length > maxLength ? raw.slice(0, maxLength) + "\n... (truncated)" : raw;
      }

      // Extract readable text from HTML
      const text = htmlToReadableText(raw, maxLength);
      return text;
    } catch (err) {
      if (err instanceof Error && err.name === "AbortError") {
        throw new ToolError("execution_error", "request timed out (30s)");
      }
      throw new ToolError("execution_error", (err as Error).message);
    } finally {
      clearTimeout(timeout);
    }
  }
}

/**
 * Convert HTML to readable plain text.
 * Strips scripts, styles, and tags; preserves structure.
 */
function htmlToReadableText(html: string, maxLength: number): string {
  let text = html;

  // Remove script and style blocks entirely
  text = text.replace(/<script[\s\S]*?<\/script>/gi, "");
  text = text.replace(/<style[\s\S]*?<\/style>/gi, "");
  text = text.replace(/<svg[\s\S]*?<\/svg>/gi, "");

  // Remove HTML comments
  text = text.replace(/<!--[\s\S]*?-->/g, "");

  // Block-level elements: insert newlines
  text = text.replace(/<\/(p|div|h[1-6]|li|tr|blockquote|section|article|pre|br|hr)[\s\/]*>/gi, "\n");
  text = text.replace(/<(br|hr)\s*\/?>/gi, "\n");

  // Extract link href attributes for reference
  text = text.replace(/<a[^>]*href=["']([^"']+)["'][^>]*>([\s\S]*?)<\/a>/gi, "$2 [$1]");

  // Extract image alt text
  text = text.replace(/<img[^>]*alt=["']([^"']+)["'][^>]*>/gi, "[$1]");

  // Remove all remaining HTML tags
  text = text.replace(/<[^>]+>/g, "");

  // Decode common HTML entities
  text = text.replace(/&amp;/g, "&");
  text = text.replace(/&lt;/g, "<");
  text = text.replace(/&gt;/g, ">");
  text = text.replace(/&quot;/g, '"');
  text = text.replace(/&#39;/g, "'");
  text = text.replace(/&nbsp;/g, " ");
  text = text.replace(/&#(\d+);/g, (_, code) => String.fromCharCode(Number(code)));
  text = text.replace(/&\w+;/g, "");

  // Normalize whitespace: collapse runs of spaces/tabs, keep newlines
  text = text.replace(/[^\S\n]+/g, " ");
  // Collapse 3+ consecutive newlines into 2
  text = text.replace(/\n{3,}/g, "\n\n");
  // Trim each line
  text = text.split("\n").map((l) => l.trim()).join("\n");
  // Trim overall
  text = text.trim();

  if (text.length > maxLength) {
    text = text.slice(0, maxLength) + "\n... (truncated)";
  }

  return text;
}

// ---------------------------------------------------------------------------
// SshTool — execute commands on remote hosts via SSH
// ---------------------------------------------------------------------------

interface SshArgs {
  host: string;
  command: string;
  user?: string;
  port?: number;
  timeout?: number;
}

/**
 * SshTool executes commands on a remote host via the system ssh client.
 * Requires SSH access to be pre-configured (keys, config, etc.).
 */
export class SshTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        host: { type: "string", description: "Remote host (hostname or IP)." },
        command: { type: "string", description: "Command to execute on the remote host." },
        user: { type: "string", description: "SSH username (default: current user)." },
        port: { type: "integer", description: "SSH port (default: 22)." },
        timeout: { type: "integer", description: "Connection timeout in seconds (default: 30)." },
      },
      required: ["host", "command"],
    };
  }

  name(): string { return "ssh"; }
  description(): string {
    return "Execute a command on a remote host via SSH. " +
      "Requires SSH access to be pre-configured (SSH keys, ~/.ssh/config). " +
      "Use for remote server management, deployment, and debugging.";
  }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return defaultMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as SshArgs;

    if (!args.host) throw new ToolError("invalid_params", "host is required");
    if (!args.command) throw new ToolError("invalid_params", "command is required");

    // Block internal network access
    const hostLower = args.host.toLowerCase();
    for (const prefix of BLOCKED_HOST_PREFIXES) {
      if (hostLower.startsWith(prefix) || hostLower === "localhost") {
        throw new ToolError("security_violation", "SSH to internal/private addresses is blocked");
      }
    }

    const sshArgs: string[] = [];
    if (args.port && args.port > 0) {
      sshArgs.push("-p", String(args.port));
    }
    sshArgs.push("-o", "StrictHostKeyChecking=accept-new");
    sshArgs.push("-o", `ConnectTimeout=${args.timeout ?? 30}`);

    const target = args.user ? `${args.user}@${args.host}` : args.host;
    sshArgs.push(target, args.command);

    const timeoutMs = ((args.timeout ?? 30) + 5) * 1000;

    return new Promise<string>((resolve) => {
      execFile("ssh", sshArgs, {
        timeout: timeoutMs,
        maxBuffer: 1024 * 1024,
        killSignal: "SIGTERM",
      }, (error, stdout, stderr) => {
        const output = (stdout ?? "") + (stderr ? "\n" + stderr : "");
        if (error) {
          resolve(output || error.message);
          return;
        }
        resolve(output || "(no output)");
      });
    });
  }
}

// ---------------------------------------------------------------------------
// NotebookTool — read and execute Jupyter notebook cells
// ---------------------------------------------------------------------------

interface NotebookArgs {
  action: "read" | "execute_cell" | "list_cells";
  path: string;
  cell_index?: number;
  code?: string;
}

/**
 * NotebookTool reads .ipynb (Jupyter notebook) files and can execute cells.
 */
export class NotebookTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        action: {
          type: "string",
          description: "Action: read (read notebook), list_cells (list all cells), execute_cell (run a cell).",
        },
        path: { type: "string", description: "Path to the .ipynb file." },
        cell_index: { type: "integer", description: "Cell index for execute_cell action (0-based)." },
        code: { type: "string", description: "Code to execute (overrides cell content for execute_cell)." },
      },
      required: ["action", "path"],
    };
  }

  name(): string { return "notebook"; }
  description(): string {
    return "Read and interact with Jupyter notebook (.ipynb) files. " +
      "Use for reading notebook content, listing cells, or executing code cells.";
  }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return defaultMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as NotebookArgs;

    if (!args.action) throw new ToolError("invalid_params", "action is required");
    if (!args.path) throw new ToolError("invalid_params", "path is required");

    const { readFile } = await import("node:fs/promises");
    const { existsSync } = await import("node:fs");

    if (!existsSync(args.path)) {
      throw new ToolError("not_found", `notebook not found: ${args.path}`);
    }

    let raw: string;
    try {
      raw = await readFile(args.path, "utf-8");
    } catch (err) {
      throw new ToolError("execution_error", `failed to read notebook: ${(err as Error).message}`);
    }

    let notebook: { cells?: Array<{ cell_type: string; source: string[]; outputs?: Array<{ text?: string[] }> }> };
    try {
      notebook = JSON.parse(raw);
    } catch {
      throw new ToolError("execution_error", "invalid notebook JSON");
    }

    const cells = notebook.cells;
    if (!cells || !Array.isArray(cells)) {
      throw new ToolError("execution_error", "notebook has no cells array");
    }

    switch (args.action) {
      case "read": {
        const lines: string[] = [];
        for (let i = 0; i < cells.length; i++) {
          const cell = cells[i]!;
          const source = Array.isArray(cell.source) ? cell.source.join("") : String(cell.source);
          lines.push(`--- Cell ${i} (${cell.cell_type}) ---`);
          lines.push(source);
          if (cell.outputs && cell.outputs.length > 0) {
            lines.push("--- Output ---");
            for (const out of cell.outputs) {
              if (out.text) {
                lines.push(Array.isArray(out.text) ? out.text.join("") : String(out.text));
              }
            }
          }
          lines.push("");
        }
        const result = lines.join("\n");
        return result.length > 12000 ? result.slice(0, 12000) + "\n... (truncated)" : result;
      }

      case "list_cells": {
        const lines: string[] = [];
        for (let i = 0; i < cells.length; i++) {
          const cell = cells[i]!;
          const source = Array.isArray(cell.source) ? cell.source.join("") : String(cell.source);
          const preview = source.slice(0, 80).replace(/\n/g, " ");
          lines.push(`${i}: [${cell.cell_type}] ${preview}${source.length > 80 ? "..." : ""}`);
        }
        return lines.join("\n") || "No cells.";
      }

      case "execute_cell": {
        if (args.cell_index === undefined || args.cell_index < 0 || args.cell_index >= cells.length) {
          throw new ToolError("invalid_params", `cell_index must be 0-${cells.length - 1}`);
        }
        const cell = cells[args.cell_index]!;
        const code = args.code ?? (Array.isArray(cell.source) ? cell.source.join("") : String(cell.source));

        if (cell.cell_type !== "code") {
          return `[${cell.cell_type} cell] ${code}`;
        }

        // Execute via Python
        const { mkdtemp, writeFile: fsWriteFile, unlink } = await import("node:fs/promises");
        const { join } = await import("node:path");
        const { tmpdir } = await import("node:os");

        const tmpDir = await mkdtemp(join(tmpdir(), "notebook-"));
        const tmpFile = join(tmpDir, "cell.py");

        try {
          await fsWriteFile(tmpFile, code, "utf-8");
          return new Promise<string>((resolve) => {
            execFile("python3", [tmpFile], {
              timeout: 30_000,
              maxBuffer: 1024 * 1024,
              killSignal: "SIGTERM",
            }, (error, stdout, stderr) => {
              const output = (stdout ?? "") + (stderr ? "\n" + stderr : "");
              if (error) {
                resolve(output || error.message);
                return;
              }
              resolve(output || "(no output)");
            });
          });
        } catch (err) {
          throw new ToolError("execution_error", (err as Error).message);
        } finally {
          try { await unlink(tmpFile); } catch { /* ignore */ }
        }
      }

      default:
        throw new ToolError("invalid_params", `unknown action: ${args.action}`);
    }
  }
}

// ---------------------------------------------------------------------------
// Dead stub factories — superseded by real implementations above
// These are kept only for backward compatibility with existing imports.
// ---------------------------------------------------------------------------

/** @deprecated Use BrowserTool from browser-tool.ts instead. */
export function newBrowserTool(): BrowserTool {
  return new BrowserTool();
}

/** @deprecated Use SshTool from ssh-tool.ts instead. */
export function newSshTool(): SshTool {
  return new SshTool();
}

/** @deprecated Use NotebookTool from notebook-tool.ts instead. */
export function newNotebookTool(): NotebookTool {
  return new NotebookTool();
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/** Extracts the hostname from a URL string. */
function extractHost(rawURL: string): string {
  try {
    const u = new URL(rawURL);
    // Normalize to lowercase for comparison.
    return u.hostname.toLowerCase();
  } catch {
    return "";
  }
}
