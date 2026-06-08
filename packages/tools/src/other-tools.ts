/**
 * Other tools: Fetch, Python, Calc, Task, and stub tools.
 *
 * Ported from modules/tools/other_tools.go.
 */

import { execFile } from "node:child_process";
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
// StubTool
// ---------------------------------------------------------------------------

/**
 * A placeholder tool that returns a "not implemented" error.
 */
export class StubTool implements Tool {
  private readonly _name: string;
  private readonly _desc: string;
  private readonly _params: Record<string, unknown>;

  constructor(name: string, desc: string) {
    this._name = name;
    this._desc = desc;
    this._params = { type: "object", properties: {} };
  }

  name(): string { return this._name; }
  description(): string { return this._desc; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return defaultMetadata(); }

  async execute(_ctx: unknown, _input: unknown): Promise<string> {
    throw new ToolError("execution_error", `${this._name} tool is not yet implemented`);
  }
}

/** Creates a stub BrowserTool. */
export function newBrowserTool(): StubTool {
  return new StubTool("browser", "Interact with a web browser (not implemented).");
}

/** Creates a stub SshTool. */
export function newSshTool(): StubTool {
  return new StubTool("ssh", "Execute commands via SSH (not implemented).");
}

/** Creates a stub LspTool. */
export function newLspTool(): StubTool {
  return new StubTool("lsp", "Language Server Protocol operations (not implemented).");
}

/** Creates a stub WebSearchTool. */
export function newWebSearchTool(): StubTool {
  return new StubTool("web_search", "Search the web (not implemented).");
}

/** Creates a stub NotebookTool. */
export function newNotebookTool(): StubTool {
  return new StubTool("notebook", "Jupyter notebook operations (not implemented).");
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
