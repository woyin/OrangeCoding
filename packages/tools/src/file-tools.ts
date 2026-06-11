/**
 * File operation tools: ReadFile, WriteFile, EditFile, DeleteFile, ListDirectory.
 *
 * Enhanced with:
 * - Line numbers in ReadFile output (matching claude code/opencode)
 * - Stale-read detection in EditFile (matching reference analysis.md)
 * - Hash-anchored edit (OmO-style): optional content hash verification
 * - Diff output in EditFile
 * - Binary file detection
 */

import { readFile, writeFile, mkdir, rm, readdir, stat } from "node:fs/promises";
import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";
import { dirname, join, resolve } from "node:path";
import { createHash } from "node:crypto";
import type { Tool, ToolMetadata } from "./tool.js";
import { ToolError } from "./tool.js";
import type { PathValidator } from "./security.js";
import { readOnlyMetadata, destructiveMetadata } from "./tool.js";

// ---------------------------------------------------------------------------
// ReadFileTool
// ---------------------------------------------------------------------------

interface ReadFileArgs {
  path: string;
  offset?: number;
  limit?: number;
  /** If true, omit line numbers (default: false) */
  no_line_numbers?: boolean;
}

/** Tracks when files were last read for stale-read detection. */
export class FileReadTracker {
  private _reads = new Map<string, { timestamp: number; content: string }>();

  recordRead(path: string, content: string): void {
    this._reads.set(resolve(path), { timestamp: Date.now(), content });
  }

  getLastRead(path: string): { timestamp: number; content: string } | undefined {
    return this._reads.get(resolve(path));
  }

  clear(): void {
    this._reads.clear();
  }
}

/** Shared read tracker for stale-read detection. */
export const fileReadTracker = new FileReadTracker();

/**
 * Reads the contents of a file with line numbers, offset, and limit.
 * Line numbers are shown by default (matching claude code / opencode behavior).
 */
export class ReadFileTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _pathVal: PathValidator | null = null;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string", description: "File path to read" },
        offset: { type: "integer", description: "Start reading from this line number (1-based)" },
        limit: { type: "integer", description: "Maximum number of lines to read" },
        no_line_numbers: { type: "boolean", description: "Omit line numbers from output (default: false)" },
      },
      required: ["path"],
    };
  }

  withPathValidator(pv: PathValidator): ReadFileTool {
    this._pathVal = pv;
    return this;
  }

  name(): string { return "read_file"; }
  description(): string {
    return "Read the contents of a file. Output includes line numbers by default. " +
      "Use offset/limit for large files. Tracks read timestamps for edit safety.";
  }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as ReadFileArgs;

    if (this._pathVal !== null) {
      try {
        this._pathVal.validate(args.path);
      } catch (err) {
        throw new ToolError("security_violation", (err as Error).message);
      }
    }

    // Check for binary file
    if (await isBinaryFile(args.path)) {
      return `[Binary file: ${args.path}]`;
    }

    const lines: string[] = [];
    const lineNumbers: number[] = [];
    let lineNum = 0;
    let fileStream: ReturnType<typeof createReadStream> | null = null;

    try {
      fileStream = createReadStream(args.path, { encoding: "utf-8" });
      const rl = createInterface({ input: fileStream, crlfDelay: Infinity });

      for await (const line of rl) {
        lineNum++;
        if (args.offset && args.offset > 0 && lineNum < args.offset) {
          continue;
        }
        if (args.limit && args.limit > 0 && lines.length >= args.limit) {
          break;
        }
        lines.push(line);
        lineNumbers.push(lineNum);
      }
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    } finally {
      if (fileStream) {
        fileStream.destroy();
      }
    }

    // Track for stale-read detection
    const fullContent = lines.join("\n");
    fileReadTracker.recordRead(args.path, fullContent);

    // Compute content hash for hash-anchored edits (OmO-style)
    const contentHash = computeContentHash(fullContent);

    // Format output with line numbers
    const showLineNumbers = !args.no_line_numbers;
    if (showLineNumbers) {
      const maxLineNum = lineNumbers.length > 0 ? lineNumbers[lineNumbers.length - 1]! : 0;
      const padWidth = String(maxLineNum).length;
      const formatted = lines.map((line, i) => {
        const num = String(lineNumbers[i]!).padStart(padWidth, " ");
        return `${num} | ${line}`;
      });
      return formatted.join("\n") + `\n\n[hash:${contentHash}]`;
    }

    return fullContent + `\n\n[hash:${contentHash}]`;
  }
}

// ---------------------------------------------------------------------------
// WriteFileTool
// ---------------------------------------------------------------------------

interface WriteFileArgs {
  path: string;
  content: string;
}

export class WriteFileTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _pathVal: PathValidator | null = null;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string", description: "File path to write to" },
        content: { type: "string", description: "Content to write" },
      },
      required: ["path", "content"],
    };
  }

  withPathValidator(pv: PathValidator): WriteFileTool {
    this._pathVal = pv;
    return this;
  }

  name(): string { return "write_file"; }
  description(): string { return "Write content to a file, creating parent directories as needed."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return destructiveMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as WriteFileArgs;

    if (!args.path) {
      throw new ToolError("invalid_params", "path is required");
    }

    if (this._pathVal !== null) {
      try {
        this._pathVal.validate(args.path);
      } catch (err) {
        throw new ToolError("security_violation", (err as Error).message);
      }
    }

    try {
      await mkdir(dirname(args.path), { recursive: true });
      await writeFile(args.path, args.content, "utf-8");
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    return `Successfully wrote ${args.content.length} bytes to ${args.path}`;
  }
}

// ---------------------------------------------------------------------------
// EditFileTool — enhanced with stale-read detection, hash-anchored edit,
// and diff output
// ---------------------------------------------------------------------------

interface EditFileArgs {
  path: string;
  old_string: string;
  new_string: string;
  /** OmO-style hash-anchored edit: SHA-256 of the expected file content */
  expected_hash?: string;
}

/**
 * Compute a short SHA-256 hash of file content for hash-anchored edits.
 * Returns first 12 hex characters (48 bits) — enough for collision detection.
 */
export function computeContentHash(content: string): string {
  return createHash("sha256").update(content).digest("hex").slice(0, 12);
}

export class EditFileTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _pathVal: PathValidator | null = null;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string", description: "File path to edit" },
        old_string: { type: "string", description: "Text to find (must be unique and exact)" },
        new_string: { type: "string", description: "Replacement text" },
        expected_hash: {
          type: "string",
          description: "SHA-256 hash (12 hex chars) of the expected file content. " +
            "If provided, the edit is rejected if the current file hash does not match. " +
            "Use computeContentHash() on the file content you read to get this value.",
        },
      },
      required: ["path", "old_string", "new_string"],
    };
  }

  withPathValidator(pv: PathValidator): EditFileTool {
    this._pathVal = pv;
    return this;
  }

  name(): string { return "edit_file"; }
  description(): string {
    return "Edit a file by replacing old_string with new_string. " +
      "Uses stale-read detection to prevent editing files that changed since last read. " +
      "Returns a unified diff of the change.";
  }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return destructiveMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as EditFileArgs;

    if (this._pathVal !== null) {
      try {
        this._pathVal.validate(args.path);
      } catch (err) {
        throw new ToolError("security_violation", (err as Error).message);
      }
    }

    // Stale-read detection: check if file was modified since last read
    const lastRead = fileReadTracker.getLastRead(args.path);
    if (lastRead) {
      try {
        const currentStat = await stat(args.path);
        const mtime = currentStat.mtimeMs;
        if (mtime > lastRead.timestamp) {
          // File was modified since last read — check if content actually changed
          const currentContent = await readFile(args.path, "utf-8");
          if (currentContent !== lastRead.content) {
            throw new ToolError("execution_error",
              `File "${args.path}" has been modified since last read. Please read it again before editing.`);
          }
        }
      } catch (err) {
        if (err instanceof ToolError) throw err;
        // stat failure — let the main read below handle it
      }
    }

    let content: string;
    try {
      content = await readFile(args.path, "utf-8");
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    // OmO-style hash-anchored edit: verify file content hash
    if (args.expected_hash) {
      const actualHash = computeContentHash(content);
      if (actualHash !== args.expected_hash) {
        throw new ToolError("execution_error",
          `Hash mismatch for "${args.path}": expected ${args.expected_hash}, got ${actualHash}. ` +
          `The file has changed since you read it. Please read it again and retry the edit.`);
      }
    }

    const count = content.split(args.old_string).length - 1;
    if (count === 0) {
      throw new ToolError("execution_error", "old_string not found in file");
    }
    if (count > 1) {
      throw new ToolError("execution_error", `old_string found ${count} times; it must be unique`);
    }

    const newContent = content.replace(args.old_string, args.new_string);
    try {
      await writeFile(args.path, newContent, "utf-8");
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    // Track the new content
    fileReadTracker.recordRead(args.path, newContent);

    // Generate unified diff output
    const diff = generateUnifiedDiff(args.path, args.old_string, args.new_string, content);
    return `Successfully edited ${args.path}\n\n${diff}`;
  }
}

// ---------------------------------------------------------------------------
// DeleteFileTool
// ---------------------------------------------------------------------------

interface DeleteFileArgs {
  path: string;
}

export class DeleteFileTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _pathVal: PathValidator | null = null;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string", description: "File path to delete" },
      },
      required: ["path"],
    };
  }

  withPathValidator(pv: PathValidator): DeleteFileTool {
    this._pathVal = pv;
    return this;
  }

  name(): string { return "delete_file"; }
  description(): string { return "Delete a file."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return destructiveMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as DeleteFileArgs;

    if (this._pathVal !== null) {
      try {
        this._pathVal.validate(args.path);
      } catch (err) {
        throw new ToolError("security_violation", (err as Error).message);
      }
    }

    try {
      await rm(args.path);
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    return `Successfully deleted ${args.path}`;
  }
}

// ---------------------------------------------------------------------------
// ListDirectoryTool
// ---------------------------------------------------------------------------

interface ListDirectoryArgs {
  path: string;
  /** If true, show tree-like output with sizes */
  detailed?: boolean;
}

export class ListDirectoryTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string", description: "Directory path to list" },
        detailed: { type: "boolean", description: "Show detailed listing with sizes and dates" },
      },
      required: ["path"],
    };
  }

  name(): string { return "list_directory"; }
  description(): string { return "List the contents of a directory with file types and sizes."; }
  parameters(): Record<string, unknown> { return this._params; }
  metadata(): ToolMetadata { return readOnlyMetadata(); }

  async execute(_ctx: unknown, input: unknown): Promise<string> {
    const args = input as ListDirectoryArgs;

    let entries;
    try {
      entries = await readdir(args.path, { withFileTypes: true });
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    // Sort: directories first, then files, both alphabetically
    const sorted = [...entries].sort((a, b) => {
      if (a.isDirectory() && !b.isDirectory()) return -1;
      if (!a.isDirectory() && b.isDirectory()) return 1;
      return a.name.localeCompare(b.name);
    });

    const lines: string[] = [];
    for (const entry of sorted) {
      try {
        const info = await stat(join(args.path, entry.name));
        const isDir = entry.isDirectory();
        const typeStr = isDir ? "📁" : "📄";
        const sizeStr = isDir ? "" : formatBytes(info.size);
        const nameStr = isDir ? `${entry.name}/` : entry.name;

        if (args.detailed) {
          const date = info.mtime.toISOString().slice(0, 10);
          lines.push(`${typeStr} ${nameStr.padEnd(40)} ${sizeStr.padStart(10)} ${date}`);
        } else {
          lines.push(`${typeStr} ${nameStr}\t${sizeStr}`);
        }
      } catch {
        lines.push(`❓ ${entry.name}`);
      }
    }

    const header = `Directory: ${args.path} (${entries.length} entries)\n`;
    return header + lines.join("\n");
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function isBinaryFile(path: string): Promise<boolean> {
  try {
    const buf = Buffer.alloc(8192);
    const { open } = await import("node:fs/promises");
    const fh = await open(path, "r");
    try {
      const { bytesRead } = await fh.read(buf, 0, 8192, 0);
      // Check for null bytes (common binary indicator)
      for (let i = 0; i < bytesRead; i++) {
        if (buf[i] === 0) return true;
      }
      return false;
    } finally {
      await fh.close();
    }
  } catch {
    return false;
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

/** Generate a simple unified diff for an edit operation. */
function generateUnifiedDiff(
  filePath: string,
  oldStr: string,
  newStr: string,
  fullContent: string,
): string {
  const oldLines = oldStr.split("\n");
  const newLines = newStr.split("\n");
  const fullLines = fullContent.split("\n");

  // Find the start line of oldStr in fullContent
  let startLine = -1;
  for (let i = 0; i < fullLines.length; i++) {
    let match = true;
    for (let j = 0; j < oldLines.length; j++) {
      if (fullLines[i + j] !== oldLines[j]) {
        match = false;
        break;
      }
    }
    if (match) {
      startLine = i;
      break;
    }
  }

  if (startLine < 0) {
    return "Edit applied successfully.";
  }

  const contextLines = 3;
  const contextStart = Math.max(0, startLine - contextLines);
  const contextEnd = Math.min(fullLines.length, startLine + oldLines.length + contextLines);

  const lines: string[] = [];
  lines.push(`--- a/${filePath}`);
  lines.push(`+++ b/${filePath}`);
  lines.push(`@@ -${contextStart + 1},${contextEnd - contextStart} +${contextStart + 1},${contextEnd - contextStart - oldLines.length + newLines.length} @@`);

  // Context before
  for (let i = contextStart; i < startLine; i++) {
    lines.push(` ${fullLines[i]}`);
  }

  // Removed lines
  for (const line of oldLines) {
    lines.push(`-${line}`);
  }

  // Added lines
  for (const line of newLines) {
    lines.push(`+${line}`);
  }

  // Context after
  for (let i = startLine + oldLines.length; i < contextEnd; i++) {
    lines.push(` ${fullLines[i]}`);
  }

  return lines.join("\n");
}
