import { mkdtempSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ReadFileTool, GrepTool, FindTool } from "../index.js";

describe("ReadFileTool", () => {
  let dir: string;
  let tool: ReadFileTool;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "rf-"));
    tool = new ReadFileTool();
  });

  test("reads a full file with line numbers by default", async () => {
    const f = join(dir, "a.txt");
    writeFileSync(f, "one\ntwo\nthree\n");
    const out = await tool.execute(null, { path: f });
    expect(out).toContain("1 | one");
    expect(out).toContain("2 | two");
    expect(out).toContain("3 | three");
    expect(out).toMatch(/\[hash:[0-9a-f]+\]/);
  });

  test("no_line_numbers returns raw content", async () => {
    const f = join(dir, "a.txt");
    writeFileSync(f, "alpha\nbeta\n");
    const out = await tool.execute(null, { path: f, no_line_numbers: true });
    expect(out.startsWith("alpha\nbeta")).toBe(true);
  });

  test("offset/limit returns a windowed slice with correct line numbers", async () => {
    const f = join(dir, "a.txt");
    writeFileSync(f, "l1\nl2\nl3\nl4\nl5\n");
    const out = await tool.execute(null, { path: f, offset: 2, limit: 2 });
    // Only lines 2 and 3 should appear.
    expect(out).toContain("2 | l2");
    expect(out).toContain("3 | l3");
    expect(out).not.toContain("l1");
    expect(out).not.toContain("l4");
  });

  test("empty file returns empty content with hash", async () => {
    const f = join(dir, "empty.txt");
    writeFileSync(f, "");
    const out = await tool.execute(null, { path: f });
    expect(out).toMatch(/\[hash:[0-9a-f]+\]/);
  });

  test("binary file is reported as binary", async () => {
    const f = join(dir, "bin");
    // 8 KiB of NUL bytes → triggers binary detection.
    writeFileSync(f, Buffer.alloc(8192, 0));
    const out = await tool.execute(null, { path: f });
    expect(out).toBe(`[Binary file: ${f}]`);
  });
});

describe("GrepTool", () => {
  let dir: string;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "grep-"));
    // Build a small tree with nested directories.
    mkdirSync(join(dir, "sub"));
    writeFileSync(join(dir, "a.ts"), "export const X = 1;\nimport { z } from 'z';\n");
    writeFileSync(join(dir, "b.md"), "# Title\nsome prose\n");
    writeFileSync(join(dir, "sub", "c.ts"), "export function f() { return 2; }\n");
  });

  test("matches across nested directories", async () => {
    const tool = new GrepTool();
    const out = await tool.execute(null, { pattern: "export", path: dir });
    // Both a.ts and sub/c.ts contain "export".
    expect(out).toContain("a.ts");
    expect(out).toContain(join("sub", "c.ts"));
    expect(out.split("\n").length).toBe(2);
  });

  test("include filter restricts file types", async () => {
    const tool = new GrepTool();
    const out = await tool.execute(null, { pattern: "export|Title", path: dir, include: "\\.ts$" });
    // Only .ts files matched; b.md excluded.
    expect(out).toContain("a.ts");
    expect(out).not.toContain("b.md");
  });

  test("no matches returns a friendly message", async () => {
    const tool = new GrepTool();
    const out = await tool.execute(null, { pattern: "zzz_no_such_token", path: dir });
    expect(out).toBe("No matches found.");
  });

  test("invalid regex raises an invalid_params error", async () => {
    const tool = new GrepTool();
    await expect(tool.execute(null, { pattern: "(", path: dir })).rejects.toThrow();
  });
});

describe("FindTool", () => {
  let dir: string;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "find-"));
    mkdirSync(join(dir, "x"));
    writeFileSync(join(dir, "a.ts"), "");
    writeFileSync(join(dir, "b.md"), "");
    writeFileSync(join(dir, "x", "c.ts"), "");
  });

  test("finds files by glob name", async () => {
    const tool = new FindTool();
    const out = await tool.execute(null, { path: dir, name: "*.ts" });
    expect(out).toContain("a.ts");
    expect(out).toContain(join("x", "c.ts"));
    expect(out).not.toContain("b.md");
  });

  test("type=file excludes directories", async () => {
    const tool = new FindTool();
    const out = await tool.execute(null, { path: dir, type: "file" });
    expect(out).toContain("a.ts");
    expect(out).toContain("b.md");
  });
});
