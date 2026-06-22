/**
 * EditFileTool 单元测试：覆盖 indexOf 唯一性判定与成功替换路径。
 *
 * 重构背景：原实现用 content.split(old_string) 计数再 replace，
 * 大文件下会构造大数组；改为 indexOf 二次查找 + slice 拼接。
 * 此测试固化“未找到 / 多次出现 / 唯一替换”三类语义。
 */

import { writeFile, mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { EditFileTool, fileReadTracker } from "../file-tools.js";
import { ToolError } from "../tool.js";

describe("EditFileTool", () => {
  let tempDir: string;
  let tool: EditFileTool;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), "edit-file-"));
    tool = new EditFileTool();
    fileReadTracker.clear();
  });
  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it("成功替换唯一匹配的 old_string", async () => {
    const f = join(tempDir, "a.txt");
    await writeFile(f, "alpha beta gamma\n", "utf-8");
    const out = await tool.execute(null, {
      path: f,
      old_string: "beta",
      new_string: "BETA",
    });
    expect(out).toContain("Successfully edited");
    // 直接校验落盘后的真实内容（diff 渲染对“非整行” old_string 有已知局限）
    const after = await readFile(f, "utf-8");
    expect(after).toBe("alpha BETA gamma\n");
  });

  it("old_string 未找到时报错", async () => {
    const f = join(tempDir, "b.txt");
    await writeFile(f, "hello\n", "utf-8");
    await expect(
      tool.execute(null, { path: f, old_string: "missing", new_string: "x" }),
    ).rejects.toThrow("not found");
  });

  it("old_string 多次出现时报错（indexOf 二次查找生效）", async () => {
    const f = join(tempDir, "c.txt");
    await writeFile(f, "dup line\ndup again\n", "utf-8");
    await expect(
      tool.execute(null, { path: f, old_string: "dup", new_string: "ONE" }),
    ).rejects.toThrow("unique");
  });

  it("expected_hash 不匹配时报错", async () => {
    const f = join(tempDir, "d.txt");
    await writeFile(f, "content here\n", "utf-8");
    await expect(
      tool.execute(null, {
        path: f,
        old_string: "content",
        new_string: "X",
        expected_hash: "deadbeefdead",
      }),
    ).rejects.toThrow("Hash mismatch");
  });
});
