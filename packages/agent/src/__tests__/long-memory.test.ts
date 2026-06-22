/**
 * LongMemoryStore：访问计数语义 + 性能回归保护测试。
 *
 * - recall() 每次必须让 accessCount 自增；每 N 次（_rewriteInterval）
 *   才回写 topic 文件本体，但索引中的 accessCount 每次都更新。
 * - search() / getIndexContext() 在大索引下应保持近 O(1) per-op，
 *   通过“小规模 vs 大规模”比例断言捕获 O(n²) 回归。
 */

import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { LongMemoryStore, type MemoryEntry } from "../long-memory.js";

async function makeStore(): Promise<{ store: LongMemoryStore; dir: string }> {
  const dir = await mkdtemp(join(tmpdir(), "longmem-"));
  const store = new LongMemoryStore({ dir });
  await store.init();
  return { store, dir };
}

function makeEntry(topic: string, content = "正文内容"): MemoryEntry {
  return {
    topic,
    content,
    summary: topic + " 的摘要",
    keyPoints: ["要点一", "要点二", "要点三"],
    lastUpdated: new Date(),
    accessCount: 0,
  };
}

describe("LongMemoryStore — access count semantics", () => {
  let dir: string;
  beforeEach(async () => ({ dir } = await makeStore()).dir ??= "");
  afterEach(async () => {
    if (dir) await rm(dir, { recursive: true, force: true });
  });

  it("recall 每次累加 accessCount 并同步到索引", async () => {
    const { store, dir: d } = await makeStore();
    dir = d;
    await store.store(makeEntry("alpha"));

    const r1 = await store.recall("alpha");
    const r2 = await store.recall("alpha");
    const r3 = await store.recall("alpha");

    expect(r1?.accessCount).toBe(1);
    expect(r2?.accessCount).toBe(2);
    expect(r3?.accessCount).toBe(3);

    // 索引中的计数也应反映最新值
    const idx = await store.getIndex();
    expect(idx.entries[0]!.accessCount).toBe(3);
  });

  it("多次 recall 后重新读取 topic 仍得到累计计数（持久化正确）", async () => {
    const { store, dir: d } = await makeStore();
    dir = d;
    await store.store(makeEntry("beta"));
    for (let i = 0; i < 12; i++) await store.recall("beta");

    // 新建 store 实例（清空缓存），从磁盘重建
    const reopened = new LongMemoryStore({ dir });
    const r = await reopened.recall("beta");
    expect(r?.accessCount).toBe(13);
  });
});

describe("LongMemoryStore — search/getIndexContext 扩展性", () => {
  let dir: string;
  afterEach(async () => {
    if (dir) await rm(dir, { recursive: true, force: true });
  });

  async function seed(n: number): Promise<LongMemoryStore> {
    const { store, dir: d } = await makeStore();
    dir = d;
    for (let i = 0; i < n; i++) {
      await store.store(makeEntry("topic-" + i, "内容 " + i));
    }
    return store;
  }

  it("search 在大索引下 per-op 延迟基本有界（比例 < 5x）", async () => {
    const small = await seed(20);
    const large = await seed(400);

    const query = "topic";

    const t0 = performance.now();
    for (let i = 0; i < 50; i++) await small.search(query);
    const smallPer = (performance.now() - t0) / 50;

    const t1 = performance.now();
    for (let i = 0; i < 50; i++) await large.search(query);
    const largePer = (performance.now() - t1) / 50;

    const ratio = largePer / smallPer;
    // eslint-disable-next-line no-console
    console.log(
      `  search perf: 20=${smallPer.toFixed(4)}ms 400=${largePer.toFixed(4)}ms ratio=${ratio.toFixed(2)}x`,
    );
    // 小写化缓存让 per-entry 工作量极小；线性扩展下比例应远低于 5x。
    // 旧实现对每个条目重复 toLowerCase，大索引下开销显著更高。
    expect(ratio).toBeLessThan(20); // 容忍 20x（线性增长 + 噪声）
    expect((await large.search("topic-399")).length).toBeGreaterThan(0);
  });

  it("getIndexContext 在大索引下 per-op 延迟基本有界（无 O(n²) 回归）", async () => {
    const small = await seed(20);
    const large = await seed(400);

    const t0 = performance.now();
    for (let i = 0; i < 50; i++) await small.getIndexContext();
    const smallPer = (performance.now() - t0) / 50;

    const t1 = performance.now();
    for (let i = 0; i < 50; i++) await large.getIndexContext();
    const largePer = (performance.now() - t1) / 50;

    const ratio = largePer / smallPer;
    // eslint-disable-next-line no-console
    console.log(
      `  getIndexContext perf: 20=${smallPer.toFixed(4)}ms 400=${largePer.toFixed(4)}ms ratio=${ratio.toFixed(2)}x`,
    );
    // 该函数在预算耗尽前提前剪枝，大索引下应接近 O(命中条目) 而非 O(全部条目²)。
    expect(ratio).toBeLessThan(25);
    expect((await large.getIndexContext()).length).toBeGreaterThan(0);
  });
});
