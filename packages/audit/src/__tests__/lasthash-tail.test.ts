/**
 * lastHash 尾部读取正确性测试。
 *
 * 重构背景：原 lastHash() 为拿最后一行而读取并解析整个日志；
 * 现改为只读取文件尾部 64KiB。需保证：
 * - 在已有大量 entry 的日志上首次 append，链仍然连续（verifyChain 通过）
 * - 空 / 单条 / 多条 场景都正确
 * - 损坏的尾行不抛异常（回退到空哈希）
 */

import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { AuditLog, AuditEntry, verifyChain } from "../index.js";

describe("AuditLog.lastHash 尾部读取", () => {
  let dir: string;
  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "audit-lasthash-"));
  });
  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("在已有大量 entry 的日志上 reopen 后 append，链保持完整", async () => {
    // 先用一个 store 写入 N 条，建立合法链
    const seed = await AuditLog.create(dir);
    const N = 500;
    for (let i = 0; i < N; i++) {
      await seed.append("tool_call_completed", "agent", `{"i":${i}}`);
    }

    // 重新打开（清掉 _lastHash 缓存），首次 append 触发 lastHash 尾部读取
    const reopened = await AuditLog.create(dir);
    await reopened.append("guardrail_decision", "agent", '{"after":"reopen"}');

    // 整条链必须仍然连续
    const { readFile } = await import("node:fs/promises");
    const raw = await readFile(join(dir, "audit.jsonl"), "utf-8");
    const entries = raw
      .split("\n")
      .filter((l) => l.trim())
      .map((l) => AuditEntry.fromJSON(JSON.parse(l)));
    expect(entries.length).toBe(N + 1);
    expect(verifyChain(entries)).toBeNull();
  });

  it("空日志首次 append 仍正确（创世 entry 基于空哈希）", async () => {
    const log = await AuditLog.create(dir);
    await log.append("action", "agent", "{}");
    const raw = await (await import("node:fs/promises")).readFile(join(dir, "audit.jsonl"), "utf-8");
    const entries = raw.split("\n").filter((l) => l.trim()).map((l) => AuditEntry.fromJSON(JSON.parse(l)));
    expect(entries.length).toBe(1);
    expect(verifyChain(entries)).toBeNull();
  });
});
