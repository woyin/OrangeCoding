/**
 * LspTool — Language Server Protocol operations for code navigation.
 *
 * Replaces the stub newLspTool() with a real implementation that
 * shells out to language-specific tools when no LSP server is available,
 * and uses JSON-RPC over stdio when one is configured.
 *
 * Actions: definition, references, hover, diagnostics, symbols, rename.
 * Comparable to claude code / opencode LSP integration.
 */

import { execFile, spawn, type ChildProcess } from "node:child_process";
import { promisify } from "node:util";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { extname, resolve, dirname, join } from "node:path";
import type { Tool, ToolMetadata } from "./tool.js";
import { ToolError } from "./tool.js";
import { readOnlyMetadata } from "./tool.js";

const execFileAsync = promisify(execFile);
const MAX_OUTPUT = 256 * 1024;
const CMD_TIMEOUT = 30_000;

// ---------------------------------------------------------------------------
// LSP protocol response types (subset of LSP 3.17)
// ---------------------------------------------------------------------------

/** LspPosition is a 0-based (line, character) cursor position in the LSP protocol. */
/** LSP text document position: line (0-based) and character offset. */
interface LspPosition {
  line: number;
  character: number;
}

/** LspRange is a half-open [start, end) span within a document. */
/** LSP text range: start and end positions within a document. */
interface LspRange {
  start: LspPosition;
  end: LspPosition;
}

/** LspLocation is a target URI plus the range to highlight (definition/reference results). */
/** LSP location: a file URI combined with a text range. */
interface LspLocation {
  uri: string;
  range: LspRange;
  targetSelectionRange?: LspRange;
}

/** Result of an LSP hover request (documentation/type info at a position). */
interface LspHoverResult {
  contents: string | { kind: string; value: string } | Array<string | { kind: string; value: string }>;
}

/** Information about a symbol (variable, function, class) from LSP. */
interface LspSymbolInfo {
  name: string;
  kind: number;
  range?: LspRange;
  selectionRange?: LspRange;
}

/** A single text replacement edit within a document. */
interface LspTextEdit {
  range: LspRange;
  newText: string;
}

/** A set of edits spanning multiple files in a workspace. */
interface LspWorkspaceEdit {
  changes?: Record<string, LspTextEdit[]>;
}

/** A single code completion suggestion from the LSP server. */
interface LspCompletionItem {
  label: string;
  kind?: number;
  detail?: string;
}

/** A list of completion items returned for a specific cursor position. */
interface LspCompletionList {
  isIncomplete: boolean;
  items: LspCompletionItem[];
}

// ---------------------------------------------------------------------------
// LSP server config (user can override)
// ---------------------------------------------------------------------------

/** LspServerConfig maps file extensions to the language-server command line. */
/**
 * Configuration for connecting to and managing an LSP language server.
 * Specifies the server command, arguments, initialization options,
 * and supported language/file extensions.
 */
interface LspServerConfig {
  /** Language to command map, e.g. { "typescript": ["typescript-language-server", "--stdio"] } */
  commands: Record<string, string[]>;
}

/** Default LSP server commands by language extension. */
const DEFAULT_LSP_COMMANDS: Record<string, string[]> = {
  ".ts": ["npx", "typescript-language-server", "--stdio"],
  ".tsx": ["npx", "typescript-language-server", "--stdio"],
  ".js": ["npx", "typescript-language-server", "--stdio"],
  ".jsx": ["npx", "typescript-language-server", "--stdio"],
  ".py": ["pyright-langserver", "--stdio"],
  ".go": ["gopls", "serve"],
  ".rs": ["rust-analyzer"],
  ".java": ["jdtls"],
  ".c": ["clangd"],
  ".cpp": ["clangd"],
  ".h": ["clangd"],
  ".hpp": ["clangd"],
};

// ---------------------------------------------------------------------------
// LspTool
// ---------------------------------------------------------------------------

/** LspArgs is the parsed input for the LspTool: action, file, position, and rename target. */
/** Parsed and validated arguments for LSP tool operations. */
interface LspArgs {
  action: "definition" | "references" | "hover" | "diagnostics" | "symbols" | "rename" | "completion";
  path: string;
  line: number;
  column?: number;
  /** For rename: new name */
  newName?: string;
  /** Language hint (auto-detected from extension if omitted) */
  language?: string;
  /** Working directory */
  cwd?: string;
}

/**
 * LspTool integrates Language Server Protocol servers for deep code analysis.
 *
 * Capabilities:
 * - go_to_definition: navigate to where a symbol is defined
 * - find_references: find all locations where a symbol is used
 * - hover: get type information and documentation at a position
 * - completions: get code completion suggestions
 * - diagnostics: retrieve compiler/linter errors and warnings
 * - symbols: list document or workspace symbols
 * - rename: rename a symbol across all files
 *
 * Each language requires a configured LSP server (e.g., typescript-language-server,
 * pyright, gopls). The tool manages server lifecycle and communication.
 */
export class LspTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _lspConfig: LspServerConfig;

  /**
   * Constructs an LspTool. If lspConfig is omitted, DEFAULT_LSP_COMMANDS is
   * used (typescript-language-server, pyright, gopls, rust-analyzer, etc.).
   */
  constructor(lspConfig?: LspServerConfig) {
    this._lspConfig = lspConfig || { commands: DEFAULT_LSP_COMMANDS };
    this._params = {
      type: "object",
      properties: {
        action: {
          type: "string",
          description: "LSP operation: definition, references, hover, diagnostics, symbols, rename, completion",
        },
        path: { type: "string", description: "File path" },
        line: { type: "integer", description: "Line number (1-based)" },
        column: { type: "integer", description: "Column number (0-based, default 0)" },
        newName: { type: "string", description: "New name for rename operation" },
        language: { type: "string", description: "Language hint (auto-detected if omitted)" },
        cwd: { type: "string", description: "Working directory" },
      },
      required: ["action", "path", "line"],
    };
  }

  /** name returns the tool identifier "lsp". */
  name(): string { return "lsp"; }
  /** description returns the human-readable tool description shown to the model. */
  description(): string {
    return "Language Server Protocol operations for code navigation and analysis. " +
      "Use for: go-to-definition, find-references, hover info, diagnostics, document symbols, " +
      "rename symbol, and completions. Supports TypeScript, Python, Go, Rust, Java, C/C++.";
  }
  /** parameters returns the JSON schema for this tool inputs. */
  parameters(): Record<string, unknown> { return this._params; }
  /** metadata marks this tool as read-only (no filesystem writes). */
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  /**
   * execute validates the request, resolves the absolute path, and first
   * attempts the real LSP path (JSON-RPC over stdio). On any failure it falls
   * back to heuristic grep-based tools so the tool degrades gracefully when
   * no language server is installed.
   */
  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as LspArgs;

    if (!args.action) throw new ToolError("invalid_params", "action is required");
    if (!args.path) throw new ToolError("invalid_params", "path is required");
    if (!args.line || args.line < 1) throw new ToolError("invalid_params", "line must be >= 1");

    const absPath = resolve(args.cwd || ".", args.path);

    if (!existsSync(absPath)) {
      throw new ToolError("not_found", `file not found: ${absPath}`);
    }

    // Try real LSP first, fallback to heuristic tools
    try {
      return await this.executeWithLsp(args, absPath);
    } catch {
      return await this.executeFallback(args, absPath);
    }
  }

  // -------------------------------------------------------------------------
  // Real LSP via JSON-RPC over stdio
  // -------------------------------------------------------------------------

  /**
   * executeWithLsp spawns the configured language server, sends initialize +
   * didOpen + the requested method over JSON-RPC stdin, then parses the
   * Content-Length-framed stdout responses and formats the result for id=1.
   */
  private async executeWithLsp(args: LspArgs, absPath: string): Promise<string> {
    const ext = extname(absPath).toLowerCase();
    const cmdParts = this._lspConfig.commands[ext];
    if (!cmdParts || cmdParts.length === 0) {
      throw new ToolError("execution_error", `no LSP server configured for ${ext}`);
    }

    const [cmd, ...cmdArgs] = cmdParts;
    const cwd = args.cwd || dirname(absPath);
    const line = args.line - 1; // LSP uses 0-based lines
    const col = args.column || 0;
    const uri = `file://${absPath}`;

    // Build the JSON-RPC request
    const method = this.lspMethod(args.action);
    const params = this.lspParams(args.action, uri, line, col, absPath, args.newName);
    const request = JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method,
      params,
    });

    // Initialize + request in one shot
    const initRequest = JSON.stringify({
      jsonrpc: "2.0",
      id: 0,
      method: "initialize",
      params: {
        processId: process.pid,
        rootUri: `file://${cwd}`,
        capabilities: {
          textDocument: {
            definition: { linkSupport: false },
            references: {},
            hover: { contentFormat: ["plaintext", "markdown"] },
            documentSymbol: { hierarchicalDocumentSymbolSupport: true },
            rename: { prepareSupport: false },
            completion: { completionItem: { snippetSupport: false } },
            publishDiagnostics: { relatedInformation: true },
          },
        },
      },
    });

    const didOpen = JSON.stringify({
      jsonrpc: "2.0",
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          uri,
          languageId: args.language || ext.slice(1),
          version: 1,
          text: await readFile(absPath, "utf-8"),
        },
      },
    });

    const input = [
      initRequest,
      didOpen,
      request,
      JSON.stringify({ jsonrpc: "2.0", id: 2, method: "shutdown", params: null }),
      JSON.stringify({ jsonrpc: "2.0", method: "exit", params: null }),
    ].join("\n") + "\n";

    return new Promise<string>((resolveP, rejectP) => {
      const child = spawn(cmd!, cmdArgs, {
        cwd,
        stdio: ["pipe", "pipe", "pipe"],
        timeout: CMD_TIMEOUT,
      });

      let stdout = "";
      let stderr = "";

      child.stdout.on("data", (chunk: Buffer) => {
        stdout += chunk.toString();
      });

      child.stderr.on("data", (chunk: Buffer) => {
        stderr += chunk.toString();
      });

      child.on("error", (err) => {
        rejectP(new ToolError("execution_error", `LSP server error: ${err.message}`));
      });

      child.on("close", (code) => {
        // Parse JSON-RPC responses
        const responses = this.parseJsonRpcResponses(stdout);
        // Find our response (id: 1)
        const result = responses.find((r) => r.id === 1);

        if (result && result.result !== undefined) {
          resolveP(this.formatLspResult(args.action, result.result, absPath));
        } else if (result && result.error) {
          rejectP(new ToolError("execution_error", result.error.message || "LSP error"));
        } else {
          // Fallback if LSP didn't respond properly
          rejectP(new ToolError("execution_error", "LSP server returned no result"));
        }
      });

      // Send all requests
      child.stdin.write(input);
      child.stdin.end();

      // Safety timeout
      setTimeout(() => {
        child.kill("SIGTERM");
        rejectP(new ToolError("execution_error", "LSP request timed out"));
      }, CMD_TIMEOUT);
    });
  }

  // -------------------------------------------------------------------------
  // Fallback: heuristic tools when no LSP is available
  // -------------------------------------------------------------------------

  /**
   * executeFallback handles requests when no LSP server is available by
   * routing to language-specific shell tools (tsc/py_compile/go vet) for
   * diagnostics, and regex heuristics for symbols/definition/references/hover.
   */
  private async executeFallback(args: LspArgs, absPath: string): Promise<string> {
    switch (args.action) {
      case "diagnostics":
        return await this.fallbackDiagnostics(absPath);
      case "symbols":
        return await this.fallbackSymbols(absPath);
      case "definition":
        return await this.fallbackDefinition(args, absPath);
      case "references":
        return await this.fallbackReferences(args, absPath);
      case "hover":
        return await this.fallbackHover(args, absPath);
      default:
        return `${args.action}: no LSP server available. Install a language server for full support.`;
    }
  }

  /**
   * fallbackDiagnostics runs language-specific linters when no LSP is available:
   * tsc --noEmit for TS/JS, py_compile for Python, go vet for Go.
   */
  private async fallbackDiagnostics(absPath: string): Promise<string> {
    const ext = extname(absPath).toLowerCase();
    const cwd = dirname(absPath);

    switch (ext) {
      case ".ts":
      case ".tsx":
      case ".js":
      case ".jsx": {
        try {
          const { stdout } = await execFileAsync("npx", ["tsc", "--noEmit", "--pretty", absPath], {
            cwd, timeout: CMD_TIMEOUT, maxBuffer: MAX_OUTPUT,
          });
          return stdout || "No diagnostics found.";
        } catch (err: unknown) {
          const e = err as { stdout?: string; stderr?: string };
          return (e.stdout || e.stderr || "No diagnostics.") as string;
        }
      }
      case ".py": {
        try {
          const { stdout } = await execFileAsync("python3", ["-m", "py_compile", absPath], {
            cwd, timeout: CMD_TIMEOUT, maxBuffer: MAX_OUTPUT,
          });
          return stdout || "No diagnostics found.";
        } catch (err: unknown) {
          const e = err as { stdout?: string; stderr?: string };
          return (e.stdout || e.stderr || "No diagnostics.") as string;
        }
      }
      case ".go": {
        try {
          const { stdout } = await execFileAsync("go", ["vet", absPath], {
            cwd, timeout: CMD_TIMEOUT, maxBuffer: MAX_OUTPUT,
          });
          return stdout || "No diagnostics found.";
        } catch (err: unknown) {
          const e = err as { stdout?: string; stderr?: string };
          return (e.stdout || e.stderr || "No diagnostics.") as string;
        }
      }
      default:
        return `Diagnostics not available for ${ext} files without LSP.`;
    }
  }

  /**
   * fallbackSymbols extracts document symbols (functions, classes, interfaces,
   * types, vars, methods) via module-scope regex heuristics. Returns one line
   * per symbol with its line number.
   */
  private async fallbackSymbols(absPath: string): Promise<string> {
    const content = await readFile(absPath, "utf-8");
    const lines = content.split("\n");
    const symbols: string[] = [];

    // Heuristic symbol extraction. Uses module-scope pre-compiled regexes
    // (RE_SYM_*) so the patterns are not recompiled once per line per call.
    // On a 1000-line file this previously allocated ~5000 regex objects.
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i]!;
      const lineNum = i + 1;

      // Functions/methods
      const funcMatch = line.match(RE_SYM_FUNC);
      if (funcMatch) {
        symbols.push(`${lineNum}: function ${funcMatch[3]}`);
        continue;
      }

      // Class declarations
      const classMatch = line.match(RE_SYM_CLASS);
      if (classMatch) {
        symbols.push(`${lineNum}: class ${classMatch[3]}`);
        continue;
      }

      // Interface/type
      const ifaceMatch = line.match(RE_SYM_IFACE);
      if (ifaceMatch) {
        symbols.push(`${lineNum}: ${ifaceMatch[2]} ${ifaceMatch[3]}`);
        continue;
      }

      // const/let/var declarations (top-level only)
      const varMatch = line.match(RE_SYM_VAR);
      if (varMatch) {
        symbols.push(`${lineNum}: ${varMatch[2]} ${varMatch[3]}`);
        continue;
      }

      // Method shorthand in classes. Skip control-flow keywords by checking a
      // trimmed-line prefix set rather than 4 separate startsWith() calls.
      const methodMatch = line.match(RE_SYM_METHOD);
      if (methodMatch) {
        const trimmed = line.trim();
        if (trimmed.charCodeAt(0) === 105 /* 'i' */ && trimmed.startsWith("if")) continue;
        if (trimmed.charCodeAt(0) === 102 /* 'f' */ && trimmed.startsWith("for")) continue;
        if (trimmed.charCodeAt(0) === 119 /* 'w' */ && trimmed.startsWith("while")) continue;
        if (trimmed.charCodeAt(0) === 115 /* 's' */ && trimmed.startsWith("switch")) continue;
        symbols.push(`${lineNum}: method ${methodMatch[2]}`);
      }
    }

    if (symbols.length === 0) {
      return "No symbols found.";
    }

    return `Document symbols in ${absPath}:\n` + symbols.join("\n");
  }

  /**
   * fallbackDefinition extracts the identifier under the cursor and searches
   * the current file for lines that look like a definition of that identifier.
   */
  private async fallbackDefinition(args: LspArgs, absPath: string): Promise<string> {
    // Read the line and extract the identifier under cursor
    const content = await readFile(absPath, "utf-8");
    const lines = content.split("\n");
    const line = lines[args.line - 1] || "";
    const col = args.column || 0;

    const identifier = extractIdentifierAtPosition(line, col);
    if (!identifier) {
      return "No identifier found at the given position.";
    }

    // Search for definition patterns in the same file
    const results: string[] = [];
    for (let i = 0; i < lines.length; i++) {
      const l = lines[i]!;
      if (l.includes(identifier) && isLikelyDefinition(l, identifier)) {
        results.push(`${absPath}:${i + 1}: ${l.trim()}`);
      }
    }

    if (results.length === 0) {
      return `No definition found for "${identifier}" in current file.`;
    }

    return `Possible definitions for "${identifier}":\n` + results.join("\n");
  }

  /**
   * fallbackReferences finds all occurrences of the cursor identifier in the
   * current file (capped at 50 results) as a lightweight references lookup.
   */
  private async fallbackReferences(args: LspArgs, absPath: string): Promise<string> {
    const content = await readFile(absPath, "utf-8");
    const lines = content.split("\n");
    const line = lines[args.line - 1] || "";
    const col = args.column || 0;

    const identifier = extractIdentifierAtPosition(line, col);
    if (!identifier) {
      return "No identifier found at the given position.";
    }

    // Search in current file
    const results: string[] = [];
    const maxRefs = 50;
    for (let i = 0; i < lines.length && results.length < maxRefs; i++) {
      if (lines[i]!.includes(identifier)) {
        results.push(`${absPath}:${i + 1}: ${lines[i]!.trim()}`);
      }
    }

    if (results.length === 0) {
      return `No references found for "${identifier}".`;
    }

    const header = `References for "${identifier}" (${results.length} found in current file):\n`;
    return header + results.join("\n");
  }

  /**
   * fallbackHover reports any type annotation or declaration found for the
     cursor identifier, or falls back to a no-type-info message.
   */
  private async fallbackHover(args: LspArgs, absPath: string): Promise<string> {
    const content = await readFile(absPath, "utf-8");
    const lines = content.split("\n");
    const line = lines[args.line - 1] || "";
    const col = args.column || 0;

    const identifier = extractIdentifierAtPosition(line, col);
    if (!identifier) {
      return "No identifier found at the given position.";
    }

    // Look for type annotations or declarations
    const info: string[] = [];
    for (let i = 0; i < lines.length; i++) {
      const l = lines[i]!;
      if (l.includes(identifier)) {
        // Check for type annotations
        const typeMatch = l.match(new RegExp(`${identifier}\\s*(?::|=)\\s*(.+)`));
        if (typeMatch) {
          info.push(`Type info: ${typeMatch[1]!.trim()} (line ${i + 1})`);
        }
      }
    }

    if (info.length === 0) {
      return `Identifier: "${identifier}" at ${absPath}:${args.line}\nNo type information available without LSP.`;
    }

    return `Hover info for "${identifier}":\n` + info.join("\n");
  }

  // -------------------------------------------------------------------------
  // LSP protocol helpers
  // -------------------------------------------------------------------------

  /** lspMethod maps a high-level action to the LSP JSON-RPC method name. */
  private lspMethod(action: string): string {
    switch (action) {
      case "definition": return "textDocument/definition";
      case "references": return "textDocument/references";
      case "hover": return "textDocument/hover";
      case "diagnostics": return "textDocument/documentSymbol"; // diagnostics come via publishDiagnostics
      case "symbols": return "textDocument/documentSymbol";
      case "rename": return "textDocument/rename";
      case "completion": return "textDocument/completion";
      default: return "textDocument/" + action;
    }
  }

  /** lspParams builds the JSON-RPC params object for the given action and position. */
  private lspParams(action: string, uri: string, line: number, col: number, path: string, newName?: string): Record<string, unknown> {
    const textDoc = { uri };
    const position = { line, character: col };

    switch (action) {
      case "references":
        return { textDocument: textDoc, position, context: { includeDeclaration: true } };
      case "rename":
        return { textDocument: textDoc, position, newName: newName || "newName" };
      default:
        return { textDocument: textDoc, position };
    }
  }

  /** parseJsonRpcResponses splits Content-Length-framed stdout into parsed JSON-RPC response objects. */
  private parseJsonRpcResponses(stdout: string): Array<{ id?: number; result?: unknown; error?: { message: string } }> {
    const results: Array<{ id?: number; result?: unknown; error?: { message: string } }> = [];
    // LSP uses Content-Length header framing
    const parts = stdout.split(/Content-Length: \d+\r?\n\r?\n/);
    for (const part of parts) {
      if (!part.trim()) continue;
      try {
        const parsed = JSON.parse(part.trim());
        if (parsed.id !== undefined) {
          results.push(parsed);
        }
      } catch {
        // Not valid JSON, skip
      }
    }
    return results;
  }

  /** formatLspResult renders a raw LSP result into a human-readable string per action type. */
  private formatLspResult(action: string, result: unknown, absPath: string): string {
    if (result === null || result === undefined) {
      return "No result.";
    }

    switch (action) {
      case "definition":
      case "references": {
        const locations = Array.isArray(result) ? result : [result];
        if (locations.length === 0) return "No locations found.";
        const lines = locations.map((loc: LspLocation) => {
          const uri = loc.uri || "";
          const range = loc.range || loc.targetSelectionRange || {};
          const startLine = (range.start?.line ?? 0) + 1;
          const startCol = range.start?.character ?? 0;
          const filePath = uri.replace("file://", "");
          return `${filePath}:${startLine}:${startCol}`;
        });
        return `${action} results (${lines.length}):\n` + lines.join("\n");
      }

      case "hover": {
        const hover = result as LspHoverResult;
        if (hover?.contents) {
          if (typeof hover.contents === "string") return hover.contents;
          if (typeof hover.contents === "object" && !Array.isArray(hover.contents) && "value" in hover.contents) {
            return hover.contents.value;
          }
          if (Array.isArray(hover.contents)) {
            return hover.contents.map((c) => typeof c === "string" ? c : c.value || "").join("\n");
          }
        }
        return JSON.stringify(result, null, 2);
      }

      case "symbols":
      case "diagnostics": {
        const symbols = Array.isArray(result) ? result : [result];
        const lines = symbols.map((sym: LspSymbolInfo) => {
          const name = sym.name || "unknown";
          const kind = sym.kind ?? "";
          const range = sym.range ?? sym.selectionRange;
          const line = range ? range.start.line + 1 : 0;
          return `${line}: ${name} (kind: ${kind})`;
        });
        return `Symbols (${lines.length}):\n` + lines.join("\n");
      }

      case "rename": {
        const edit = result as LspWorkspaceEdit;
        if (edit?.changes) {
          const files = Object.keys(edit.changes);
          const lines: string[] = [];
          for (const file of files) {
            const edits = edit.changes[file] || [];
            lines.push(`${file}: ${edits.length} edit(s)`);
          }
          return `Rename plan:\n` + lines.join("\n");
        }
        return JSON.stringify(result, null, 2);
      }

      case "completion": {
        const completionResult = result as LspCompletionList | LspCompletionItem[];
        const items = Array.isArray(completionResult)
          ? completionResult
          : (completionResult?.items || []);
        const lines = items.slice(0, 20).map((item: LspCompletionItem) => {
          return `${item.label} (${item.kind ?? "unknown"})${item.detail ? " — " + item.detail : ""}`;
        });
        return `Completions (${items.length}${items.length > 20 ? ", showing 20" : ""}):\n` + lines.join("\n");
      }

      default:
        return JSON.stringify(result, null, 2);
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// Module-scope compiled regexes for the fallback symbol/reference heuristics.
// Hoisting avoids recompiling the same pattern for every line of every file
// scanned when no LSP server is available.
const RE_SYM_FUNC = /^\s*(export\s+)?(async\s+)?function\s+(\w+)/;
const RE_SYM_CLASS = /^\s*(export\s+)?(abstract\s+)?class\s+(\w+)/;
const RE_SYM_IFACE = /^\s*(export\s+)?(interface|type)\s+(\w+)/;
const RE_SYM_VAR = /^(export\s+)?(const|let|var)\s+(\w+)/;
const RE_SYM_METHOD = /^\s+(async\s+)?(\w+)\s*\(/;

/**
 * Find the identifier (JS/TS style: [a-zA-Z_$][\w$]*) that contains `col`,
 * or the word at `col` as a fallback. Used by the definition/references/
 * hover fallbacks to pick the symbol the cursor is on.
 */
/**
 * Extracts the identifier (variable/function name) at a given column position.
 * Uses word-boundary detection to find the complete identifier token.
 */
function extractIdentifierAtPosition(line: string, col: number): string {
  // Find the identifier that contains the column position
  const idRegex = /[a-zA-Z_$][\w$]*/g;
  let match;
  while ((match = idRegex.exec(line)) !== null) {
    const start = match.index;
    const end = start + match[0].length;
    if (col >= start && col <= end) {
      return match[0];
    }
  }
  // Fallback: return word at position
  const words = line.split(/\W+/);
  let offset = 0;
  for (const word of words) {
    if (col >= offset && col < offset + word.length) {
      return word;
    }
    offset += word.length + 1;
  }
  return "";
}

/**
 * Heuristic test: does this line look like a *definition* of `identifier`
 * (as opposed to a usage)? Patterns cover declarations, assignment, and
 * import/export forms. Patterns are rebuilt per call because they embed the
 * identifier; callers invoke this at most a few times per fallback lookup,
 * so the cost is negligible.
 */
/**
 * Heuristically checks whether a line contains the definition of an identifier.
 * Looks for patterns like "function foo", "class Foo", "const foo =", etc.
 */
function isLikelyDefinition(line: string, identifier: string): boolean {
  const patterns = [
    new RegExp(`(function|class|interface|type|enum|const|let|var)\\s+${identifier}\\b`),
    new RegExp(`${identifier}\\s*[:=(]`),
    new RegExp(`import.*${identifier}`),
    new RegExp(`export.*${identifier}`),
  ];
  return patterns.some((p) => p.test(line));
}
