/**
 * Tests for MultiEditTool and PatchEditTool.
 */

import { writeFile, readFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { MultiEditTool, PatchEditTool } from "../multi-edit-tool.js";
import { ToolError } from "../tool.js";

describe("MultiEditTool", () => {
  let tempDir: string;
  let tool: MultiEditTool;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), "multi-edit-"));
    tool = new MultiEditTool();
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it("has correct name and description", () => {
    expect(tool.name()).toBe("multi_edit");
    expect(tool.description()).toContain("multiple text replacements");
  });

  it("has correct metadata", () => {
    const meta = tool.metadata();
    expect(meta.isReadOnly).toBe(false);
    expect(meta.isDestructive).toBe(true);
    expect(meta.isEnabled).toBe(true);
  });

  it("applies a single edit", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "hello world\nfoo bar\n", "utf-8");

    const result = await tool.execute(null, {
      path: filePath,
      edits: [{ old_string: "hello world", new_string: "hi world" }],
    });

    expect(result).toContain("1 edit(s)");
    const content = await readFile(filePath, "utf-8");
    expect(content).toBe("hi world\nfoo bar\n");
  });

  it("applies multiple edits atomically", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "line1\nline2\nline3\nline4\n", "utf-8");

    const result = await tool.execute(null, {
      path: filePath,
      edits: [
        { old_string: "line1", new_string: "LINE1" },
        { old_string: "line3", new_string: "LINE3" },
      ],
    });

    expect(result).toContain("2 edit(s)");
    const content = await readFile(filePath, "utf-8");
    expect(content).toBe("LINE1\nline2\nLINE3\nline4\n");
  });

  it("fails if old_string not found", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "hello world\n", "utf-8");

    await expect(
      tool.execute(null, {
        path: filePath,
        edits: [{ old_string: "nonexistent", new_string: "replacement" }],
      })
    ).rejects.toThrow("not found");
  });

  it("fails if old_string is not unique", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "dup\ndup\nother\n", "utf-8");

    await expect(
      tool.execute(null, {
        path: filePath,
        edits: [{ old_string: "dup", new_string: "unique" }],
      })
    ).rejects.toThrow("found 2 times");
  });

  it("is atomic — no changes if any edit fails", async () => {
    const filePath = join(tempDir, "test.txt");
    const original = "alpha\nbeta\ngamma\n";
    await writeFile(filePath, original, "utf-8");

    await expect(
      tool.execute(null, {
        path: filePath,
        edits: [
          { old_string: "alpha", new_string: "ALPHA" },
          { old_string: "nonexistent", new_string: "FAIL" },
        ],
      })
    ).rejects.toThrow("not found");

    const content = await readFile(filePath, "utf-8");
    expect(content).toBe(original);
  });

  it("rejects empty edits array", async () => {
    await expect(
      tool.execute(null, { path: "/tmp/x", edits: [] })
    ).rejects.toThrow("non-empty array");
  });

  it("rejects empty old_string", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "content\n", "utf-8");

    await expect(
      tool.execute(null, {
        path: filePath,
        edits: [{ old_string: "", new_string: "x" }],
      })
    ).rejects.toThrow("cannot be empty");
  });

  it("rejects missing path", async () => {
    await expect(
      tool.execute(null, { edits: [{ old_string: "a", new_string: "b" }] })
    ).rejects.toThrow("path is required");
  });

  it("handles multiline replacements", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "function foo() {\n  return 1;\n}\n", "utf-8");

    const result = await tool.execute(null, {
      path: filePath,
      edits: [
        {
          old_string: "function foo() {\n  return 1;\n}",
          new_string: "function foo() {\n  return 42;\n}",
        },
      ],
    });

    expect(result).toContain("1 edit(s)");
    const content = await readFile(filePath, "utf-8");
    expect(content).toBe("function foo() {\n  return 42;\n}\n");
  });
});

describe("PatchEditTool", () => {
  let tempDir: string;
  let tool: PatchEditTool;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), "patch-edit-"));
    tool = new PatchEditTool();
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it("has correct name and description", () => {
    expect(tool.name()).toBe("patch_edit");
    expect(tool.description()).toContain("unified diff");
  });

  it("applies a simple single-hunk patch", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "line1\nline2\nline3\n", "utf-8");

    const diff = `--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,3 @@
 line1
-line2
+line2_modified
 line3`;

    const result = await tool.execute(null, { path: filePath, diff });
    expect(result).toContain("Successfully applied patch");

    const content = await readFile(filePath, "utf-8");
    expect(content).toBe("line1\nline2_modified\nline3\n");
  });

  it("applies a patch that adds lines", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "line1\nline2\n", "utf-8");

    const diff = `--- a/test.txt
+++ b/test.txt
@@ -1,2 +1,3 @@
 line1
+line1.5
 line2`;

    await tool.execute(null, { path: filePath, diff });
    const content = await readFile(filePath, "utf-8");
    expect(content).toBe("line1\nline1.5\nline2\n");
  });

  it("applies a patch that removes lines", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "line1\nline2\nline3\n", "utf-8");

    const diff = `--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,2 @@
 line1
-line2
 line3`;

    await tool.execute(null, { path: filePath, diff });
    const content = await readFile(filePath, "utf-8");
    expect(content).toBe("line1\nline3\n");
  });

  it("fails on context mismatch", async () => {
    const filePath = join(tempDir, "test.txt");
    await writeFile(filePath, "line1\nWRONG\nline3\n", "utf-8");

    const diff = `--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,3 @@
 line1
-line2
+line2_modified
 line3`;

    await expect(
      tool.execute(null, { path: filePath, diff })
    ).rejects.toThrow("context mismatch");
  });

  it("rejects empty diff", async () => {
    await expect(
      tool.execute(null, { path: "/tmp/x", diff: "" })
    ).rejects.toThrow("diff is required");
  });

  it("rejects missing path", async () => {
    await expect(
      tool.execute(null, { diff: "some diff" })
    ).rejects.toThrow("path is required");
  });
});
