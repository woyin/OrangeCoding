/**
 * InMemoryMessageStore topic 索引正确性测试。
 *
 * 重构背景：pending(topic) 原遍历全部消息做 topic 过滤；
 * 现引入 _byTopic 索引，需验证：
 * - pending 只返回指定 topic 的未投递消息
 * - markDelivered 后该消息不再出现在 pending
 * - 多 topic 互不干扰
 */

import { InMemoryMessageStore, type MeshMessage } from "../message-store.js";

function mk(id: string, topic: string, payload: unknown): MeshMessage {
  return { id, topic, payload, timestamp: new Date() };
}

describe("InMemoryMessageStore topic 索引", () => {
  it("pending 只返回指定 topic 的消息", async () => {
    const s = new InMemoryMessageStore();
    await s.store(mk("1", "alpha", { n: 1 }));
    await s.store(mk("2", "beta", { n: 2 }));
    await s.store(mk("3", "alpha", { n: 3 }));

    const alpha = await s.pending("alpha");
    const beta = await s.pending("beta");
    expect(alpha.map((m) => m.id)).toEqual(["1", "3"]);
    expect(beta.map((m) => m.id)).toEqual(["2"]);
  });

  it("markDelivered 后该消息不再出现在 pending", async () => {
    const s = new InMemoryMessageStore();
    await s.store(mk("1", "alpha", 1));
    await s.store(mk("2", "alpha", 2));
    await s.markDelivered("1");

    const pending = await s.pending("alpha");
    expect(pending.map((m) => m.id)).toEqual(["2"]);
  });

  it("未注册的 topic 返回空数组（索引缺失不报错）", async () => {
    const s = new InMemoryMessageStore();
    await s.store(mk("1", "alpha", 1));
    expect(await s.pending("nonexistent")).toEqual([]);
  });

  it("大量消息 + 单 topic 查询只命中目标 topic", async () => {
    const s = new InMemoryMessageStore();
    for (let i = 0; i < 100; i++) await s.store(mk(`a-${i}`, "alpha", i));
    for (let i = 0; i < 100; i++) await s.store(mk(`b-${i}`, "beta", i));
    const alpha = await s.pending("alpha");
    expect(alpha).toHaveLength(100);
    expect(alpha.every((m) => m.topic === "alpha")).toBe(true);
  });
});
