/**
 * Search tools: Grep, Find, Glob.
 *
 * Ported from modules/tools/search_tools.go.
 */

import { opendir, readFile, glob } from "node:fs/promises";
import { join, relative } from "node:path";
import type { Tool, ToolMetadata } from "./tool.js";
import { ToolError } from "./tool.js";
import { readOnlyMetadata } from "./tool.js";

// ---------------------------------------------------------------------------
// GrepTool
// ---------------------------------------------------------------------------

interface GrepArgs {
  pattern: string;
  path?: string;
  include?: string;
}

const MAX_GREP_MATCHES = 1000;

/**
 * Searches for a regex pattern in files within a directory.
 */
export class GrepTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        pattern: { type: "string" },
        path: { type: "string" },
        include: { type: "string" },
      },
      required: ["pattern"],
    };
  }

  name(): string { return "grep"; }
  description(): string { return "Search for a regex pattern in files."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as GrepArgs;

    if (!args.pattern) {
      throw new ToolError("invalid_params", "pattern is required");
    }

    const searchPath = args.path || ".";

    let re: RegExp;
    try {
      re = new RegExp(args.pattern);
    } catch (err) {
      throw new ToolError("invalid_params", "invalid regex: " + (err as Error).message);
    }

    let includeRe: RegExp | null = null;
    if (args.include) {
      try {
        includeRe = new RegExp(args.include);
      } catch (err) {
        throw new ToolError("invalid_params", "invalid include pattern: " + (err as Error).message);
      }
    }

    const matches: string[] = [];

    try {
      await walkDir(searchPath, async (filePath, entryName) => {
        if (matches.length >= MAX_GREP_MATCHES) return;

        // Apply include filter
        if (includeRe !== null && !includeRe.test(entryName)) return;

        // Skip directories
        try {
          const info = await stat(filePath);
          if (info.isDirectory()) return;
        } catch {
          return;
        }

        try {
          const content = await readFile(filePath, "utf-8");
          const lines = content.split("\n");
          for (let i = 0; i < lines.length; i++) {
            if (matches.length >= MAX_GREP_MATCHES) break;
            if (re.test(lines[i]!)) {
              let relPath = filePath;
              try {
                relPath = relative(searchPath, filePath);
              } catch {
                // keep absolute
              }
              matches.push(`${relPath}:${i + 1}: ${lines[i]}`);
            }
          }
        } catch {
          // Skip files we can't read
        }
      });
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    if (matches.length === 0) {
      return "No matches found.";
    }

    return matches.join("\n");
  }
}

// ---------------------------------------------------------------------------
// FindTool
// ---------------------------------------------------------------------------

interface FindArgs {
  path: string;
  name?: string;
  type?: string; // "file" or "dir"
}

/**
 * Walks a directory tree and finds files/directories matching criteria.
 */
export class FindTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string" },
        name: { type: "string" },
        type: { type: "string" },
      },
      required: ["path"],
    };
  }

  name(): string { return "find"; }
  description(): string { return "Find files and directories matching criteria."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as FindArgs;

    let nameRe: RegExp | null = null;
    if (args.name) {
      const pattern = globToRegex(args.name);
      try {
        nameRe = new RegExp(pattern);
      } catch (err) {
        throw new ToolError("invalid_params", "invalid name pattern: " + (err as Error).message);
      }
    }

    const results: string[] = [];

    try {
      await walkDir(args.path, async (filePath, entryName, isDir) => {
        // Type filter
        if (args.type === "file" && isDir) return;
        if (args.type === "dir" && !isDir) return;

        // Name filter
        if (nameRe !== null && !nameRe.test(entryName)) return;

        results.push(filePath);
      });
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    if (results.length === 0) {
      return "No results found.";
    }

    return results.join("\n");
  }
}

// ---------------------------------------------------------------------------
// GlobTool
// ---------------------------------------------------------------------------

interface GlobArgs {
  pattern: string;
  path?: string;
}

/**
 * Finds files matching a glob pattern.
 */
export class GlobTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        pattern: { type: "string" },
        path: { type: "string" },
      },
      required: ["pattern"],
    };
  }

  name(): string { return "glob"; }
  description(): string { return "Find files matching a glob pattern."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as GlobArgs;

    if (!args.pattern) {
      throw new ToolError("invalid_params", "pattern is required");
    }

    // If path is given, make pattern relative to it
    let pattern = args.pattern;
    if (args.path) {
      pattern = join(args.path, args.pattern);
    }

    let matches: string[];
    try {
      const globIter = glob(pattern);
      matches = [];
      for await (const p of globIter) {
        matches.push(p);
      }
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    if (matches.length === 0) {
      return "No matches found.";
    }

    return matches.join("\n");
  }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

import { stat as statFn } from "node:fs/promises";

/** Polyfill stat helper for use inside walkDir callbacks. */
async function stat(path: string) {
  return statFn(path);
}

/** Converts a simple glob pattern (e.g. "*.go") to a regex pattern. */
function globToRegex(glob: string): string {
  let buf = "";
  for (const ch of glob) {
    switch (ch) {
      case "*":
        buf += ".*";
        break;
      case "?":
        buf += ".";
        break;
      case ".":
      case "(":
      case ")":
      case "+":
      case "|":
      case "^":
      case "$":
      case "@":
      case "%":
      case "{":
      case "}":
      case "[":
      case "]":
        buf += "\\" + ch;
        break;
      default:
        buf += ch;
    }
  }
  return "^" + buf + "$";
}

/** Walks a directory tree, calling the callback for each entry. */
async function walkDir(
  dir: string,
  callback: (path: string, name: string, isDir: boolean) => Promise<void>,
): Promise<void> {
  let dirHandle;
  try {
    dirHandle = await opendir(dir);
  } catch {
    return;
  }

  for await (const entry of dirHandle) {
    const fullPath = join(dir, entry.name);
    const isDir = entry.isDirectory();

    await callback(fullPath, entry.name, isDir);

    if (isDir) {
      await walkDir(fullPath, callback);
    }
  }
}
