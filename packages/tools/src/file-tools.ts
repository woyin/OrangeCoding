/**
 * File operation tools: ReadFile, WriteFile, EditFile, DeleteFile, ListDirectory.
 *
 * Ported from modules/tools/file_tools.go.
 */

import { open, readFile, writeFile, mkdir, rm, readdir, stat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";
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
}

/**
 * Reads the contents of a file, with optional offset and limit.
 */
export class ReadFileTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _pathVal: PathValidator | null = null;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string" },
        offset: { type: "integer" },
        limit: { type: "integer" },
      },
      required: ["path"],
    };
  }

  /** Sets the path validator for this tool. Returns this for chaining. */
  withPathValidator(pv: PathValidator): ReadFileTool {
    this._pathVal = pv;
    return this;
  }

  name(): string { return "read_file"; }
  description(): string { return "Read the contents of a file."; }
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

    const lines: string[] = [];
    let lineNum = 0;

    try {
      const fileStream = createReadStream(args.path, { encoding: "utf-8" });
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
      }
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
    }

    return lines.join("\n");
  }
}

// ---------------------------------------------------------------------------
// WriteFileTool
// ---------------------------------------------------------------------------

interface WriteFileArgs {
  path: string;
  content: string;
}

/**
 * Writes content to a file, creating parent directories as needed.
 */
export class WriteFileTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _pathVal: PathValidator | null = null;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string" },
        content: { type: "string" },
      },
      required: ["path", "content"],
    };
  }

  /** Sets the path validator for this tool. Returns this for chaining. */
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
// EditFileTool
// ---------------------------------------------------------------------------

interface EditFileArgs {
  path: string;
  old_string: string;
  new_string: string;
}

/**
 * Performs string replacement in a file.
 */
export class EditFileTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _pathVal: PathValidator | null = null;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string" },
        old_string: { type: "string" },
        new_string: { type: "string" },
      },
      required: ["path", "old_string", "new_string"],
    };
  }

  /** Sets the path validator for this tool. Returns this for chaining. */
  withPathValidator(pv: PathValidator): EditFileTool {
    this._pathVal = pv;
    return this;
  }

  name(): string { return "edit_file"; }
  description(): string { return "Edit a file by replacing old_string with new_string."; }
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

    let content: string;
    try {
      content = await readFile(args.path, "utf-8");
    } catch (err) {
      throw new ToolError("execution_error", (err as Error).message);
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

    return `Successfully replaced text in ${args.path}`;
  }
}

// ---------------------------------------------------------------------------
// DeleteFileTool
// ---------------------------------------------------------------------------

interface DeleteFileArgs {
  path: string;
}

/**
 * Removes a file from the filesystem.
 */
export class DeleteFileTool implements Tool {
  private readonly _params: Record<string, unknown>;
  private _pathVal: PathValidator | null = null;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string" },
      },
      required: ["path"],
    };
  }

  /** Sets the path validator for this tool. Returns this for chaining. */
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
}

/**
 * Lists the contents of a directory.
 */
export class ListDirectoryTool implements Tool {
  private readonly _params: Record<string, unknown>;

  constructor() {
    this._params = {
      type: "object",
      properties: {
        path: { type: "string" },
      },
      required: ["path"],
    };
  }

  name(): string { return "list_directory"; }
  description(): string { return "List the contents of a directory."; }
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

    const lines: string[] = [];
    for (const entry of entries) {
      try {
        const info = await stat(join(args.path, entry.name));
        const typeStr = entry.isDirectory() ? "dir" : "file";
        lines.push(`${entry.name}\t${info.size}\t${typeStr}`);
      } catch {
        // skip entries we can't stat
      }
    }

    return lines.join("\n");
  }
}
