/**
 * Long-term memory store backed by markdown files on disk.
 *
 * Layout under `dir`:
 *   index.md          - human-readable catalog of all topics (the "index")
 *   topics/<slug>.md  - one markdown file per topic, with content + summary
 *                       + key points + access-count metadata
 *   summaries/<date>.md - append-only daily session summaries
 *
 * Design notes:
 *   - Markdown-on-disk keeps memory human-inspectable and git-friendly.
 *   - The index is cached in-memory (_indexCache) after first read; writes
 *     update both the cache and the file.
 *   - recall() increments an access count and rewrites the topic file so
 *     frequently-used memories score higher during compact() (LRU-ish).
 *   - compact() merges the lowest-scoring quarter of topics into a single
 *     "consolidated-memories" entry once maxTopics is exceeded.
 */

import * as fs from "node:fs";
import * as path from "node:path";


/** A single stored memory topic with its full content and metadata. */
/**
 * A single entry in the agent's long-term memory.
 *
 * Contains the remembered content, importance score, source context,
 * and temporal metadata used for relevance ranking and decay.
 */
export interface MemoryEntry {
  topic: string;
  content: string;
  summary: string;
  keyPoints: string[];
  lastUpdated: Date;
  accessCount: number;
}

/** In-memory + on-disk catalog of all topics (rendered to index.md). */
export interface MemoryIndex {
  entries: MemoryIndexEntry[];
  totalTopics: number;
  lastUpdated: string;
}

/** One row in the index: enough to rebuild context without reading every topic file. */
export interface MemoryIndexEntry {
  topic: string;
  summary: string;
  keyPoints: string[];
  lastUpdated: string;
  accessCount?: number;
}

/** Tunable budgets for the long-term memory store. */
export interface LongMemoryConfig {
  dir: string;
  indexTokenBudget: number;
  topicTokenBudget: number;
  maxKeyPoints: number;
  maxTopics: number;
}

/** Default budgets: keep the index compact and topics focused. */
const DEFAULT_CONFIG: LongMemoryConfig = {
  dir: "",
  indexTokenBudget: 500,
  topicTokenBudget: 1000,
  maxKeyPoints: 5,
  maxTopics: 50,
};

/**
 * Markdown-backed long-term memory store. Lazy-initializes the directory
 * layout on first use and caches the index for fast context reads.
 */
export class LongMemoryStore {
  private _config: LongMemoryConfig;
  private _initialized = false;
  private _indexCache: MemoryIndex | null = null;
  /**
   * 访问计数回写间隔：每 N 次 recall 才重写一次 topic 文件本体，
   * 把 O(topic_size) 的字符串重渲染 + 磁盘写入分摊掉。
   * 访问计数本身每次都写入较小的 index.md，保证不会丢失多于 1 次计数。
   */
  private readonly _rewriteInterval = 5;
  /**
   * 小写化索引缓存：键为主题在 entries 数组中的下标，
   * 值为该条目各字段的小写形式。首次 search 时构建，
   * writeIndex 时失效，避免每次搜索都对同一条目重复 toLowerCase。
   */
  private _lowercasedCache: { topic: string; summary: string; keyPoints: string[] }[] | null = null;

  constructor(config: Partial<LongMemoryConfig> & { dir: string }) {
    this._config = { ...DEFAULT_CONFIG, ...config };
  }

  /** Creates the topics/ and summaries/ directories and seeds an empty index if absent. Idempotent. */
  async init(): Promise<void> {
    if (this._initialized) return;
    await fs.promises.mkdir(path.join(this._config.dir, "topics"), { recursive: true });
    await fs.promises.mkdir(path.join(this._config.dir, "summaries"), { recursive: true });
    if (!(await this.indexExists())) {
      await this.writeIndex({ entries: [], totalTopics: 0, lastUpdated: new Date().toISOString() });
    }
    this._initialized = true;
  }

  /** Writes a topic to topics/<slug>.md and upserts its row in the index. */
  async store(entry: MemoryEntry): Promise<void> {
    await this.init();
    const slug = slugify(entry.topic);
    const topicContent = this.renderTopicMd(entry);
    await fs.promises.writeFile(
      path.join(this._config.dir, "topics", slug + ".md"),
      topicContent,
      "utf-8",
    );
    await this.updateIndex(entry);
  }

  /** Appends a session summary to the per-day summaries/<date>.md file. */
  async appendSummary(sessionId: string, content: string): Promise<void> {
    await this.init();
    const date = new Date().toISOString().split("T")[0];
    const filePath = path.join(this._config.dir, "summaries", date + ".md");
    let existing = "";
    try {
      existing = await fs.promises.readFile(filePath, "utf-8");
    } catch {
      existing = "# Session Summaries " + date + "\n\n";
    }
    const entry = "## Session " + sessionId.slice(-8) + "\n" + content + "\n\n";
    await fs.promises.writeFile(filePath, existing + entry, "utf-8");
  }

  /**
   * 读取指定主题、累加访问计数并持久化；找不到则返回 undefined。
   *
   * 性能优化：访问计数以内存索引 + 较小的 index.md 为权威来源，
   * 仅在每 {@link _rewriteInterval} 次访问后回写到 topic 文件本体，
   * 避免每次 recall 都重写整个 topic（大字符串拼接 + 同步 IO）。
   */
  async recall(topic: string): Promise<MemoryEntry | undefined> {
    await this.init();
    const slug = slugify(topic);
    try {
      const content = await fs.promises.readFile(
        path.join(this._config.dir, "topics", slug + ".md"),
        "utf-8",
      );
      const entry = this.parseTopicMd(content, topic);
      // 访问计数以索引为权威：优先取索引中的最新值，topic 文件可能滞后
      // （仅每 N 次才回写）。这样多次连续 recall 也能正确累加。
      const index = await this.readIndex();
      const indexRow = index.entries.find((e) => e.topic === topic);
      if (indexRow?.accessCount != null) {
        entry.accessCount = indexRow.accessCount;
      }
      entry.accessCount += 1;
      entry.lastUpdated = new Date();

      // 先把访问计数同步到索引缓存并写回 index.md（小文件，已缓存命中）
      await this.bumpAccessCount(topic, entry.accessCount);

      // 仅在访问计数达到回写阈值时，才重写 topic 文件本体以分摊写入开销
      if (entry.accessCount % this._rewriteInterval === 0) {
        await fs.promises.writeFile(
          path.join(this._config.dir, "topics", slug + ".md"),
          this.renderTopicMd(entry),
          "utf-8",
        );
      }
      return entry;
    } catch {
      return undefined;
    }
  }

  /** Returns the full index (cached after first read). */
  async getIndex(): Promise<MemoryIndex> {
    await this.init();
    return this.readIndex();
  }

  /**
   * Builds a token-budgeted context string from the index, most-recent first.
   * Truncates entries/key-points to fit indexTokenBudget so the harness prompt
   * stays bounded. Returns "" when the index is empty.
   */
  async getIndexContext(): Promise<string> {
    const index = await this.getIndex();
    if (index.entries.length === 0) return "";

    const budget = this._config.indexTokenBudget;
    const header = "## Long-term Memory\n\n";
    let used = estimateTokens(header);
    const entryLines: string[] = [];

    // Sort by recency — most recently updated first
    const sorted = [...index.entries].sort(
      (a, b) => b.lastUpdated.localeCompare(a.lastUpdated),
    );

    // 用字符长度增量直接换算 token，避免对不断增长的 pointLines 反复整体重算
    // （原实现对每个 point 都重算 pointLines 全长，呈 O(n²) 复杂度）。
    for (const entry of sorted) {
      const block = "### " + entry.topic + "\n" + entry.summary + "\n";
      const blockTokens = estimateTokens(block);

      // 加入该条目是否会超出预算——提前剪枝
      if (used + blockTokens > budget) break;

      let pointLines = "";
      let pointTokens = 0;
      for (const point of entry.keyPoints) {
        const ptLine = "- " + point + "\n";
        const ptTokens = estimateTokens(ptLine);
        if (used + blockTokens + pointTokens + ptTokens > budget) break;
        pointLines += ptLine;
        pointTokens += ptTokens;
      }

      entryLines.push(block + pointLines);
      used += blockTokens + pointTokens;
    }

    if (entryLines.length === 0) return "";
    return header + entryLines.join("\n");
  }

  /**
   * 在主题/摘要/要点中做大小写不敏感的子串搜索。
   *
   * 性能优化：对每个条目预先小写化一次（缓存到字段），
   * 避免原实现对同一条目最多调用 3+ 次 toLowerCase（每次都分配新字符串）。
   */
  async search(query: string): Promise<MemoryIndexEntry[]> {
    const index = await this.getIndex();
    const lower = query.toLowerCase();
    const lowercased = this.getLowercasedIndex();
    return index.entries.filter((e, i) => {
      const lc = lowercased[i]!;
      return (
        lc.topic.includes(lower) ||
        lc.summary.includes(lower) ||
        lc.keyPoints.some((p) => p.includes(lower))
      );
    });
  }

  /** Removes a topic file and drops its index row. No-op if absent. */
  async delete(topic: string): Promise<void> {
    const slug = slugify(topic);
    try {
      await fs.promises.unlink(path.join(this._config.dir, "topics", slug + ".md"));
    } catch {
      /* ignore */
    }
    const index = await this.readIndex();
    index.entries = index.entries.filter((e) => e.topic !== topic);
    index.totalTopics = index.entries.length;
    await this.writeIndex(index);
  }

  /**
   * Compacts the store when entries exceed maxTopics. Scores each entry by
   * (access count × 2 + recency + key-point density), merges the lowest-
   * scoring quarter into a single "consolidated-memories" topic, and deletes
   * the originals. This is an LRU-ish eviction that preserves dense/recent/
   * popular memories.
   */
  async compact(): Promise<void> {
    const index = await this.readIndex();
    if (index.entries.length <= this._config.maxTopics) return;

    // Score entries: lower score = more likely to be merged
    // Factors: access count (higher = keep), recency (newer = keep), key points (more = keep)
    const scored = index.entries.map((entry) => {
      const access = entry.accessCount ?? 0;
      const ageDays = (Date.now() - new Date(entry.lastUpdated).getTime()) / (1000 * 60 * 60 * 24);
      const recencyScore = Math.max(0, 30 - ageDays) / 30;
      const densityScore = entry.keyPoints.length / this._config.maxKeyPoints;
      const score = access * 2 + recencyScore + densityScore;
      return { entry, score };
    });

    scored.sort((a, b) => a.score - b.score);
    const toMerge = scored.slice(0, Math.floor(scored.length / 4));
    if (toMerge.length === 0) return;

    const consolidatedPoints: string[] = [];
    for (const { entry } of toMerge) {
      consolidatedPoints.push(...entry.keyPoints);
    }

    const parts: string[] = [];
    for (const { entry } of toMerge) {
      parts.push("**" + entry.topic + "**: " + entry.summary);
    }

    const merged: MemoryEntry = {
      topic: "consolidated-memories",
      content: parts.join("\n"),
      summary: "Consolidated " + String(toMerge.length) + " low-access memories.",
      keyPoints: consolidatedPoints.slice(0, this._config.maxKeyPoints),
      lastUpdated: new Date(),
      accessCount: 0,
    };

    await this.store(merged);
    for (const { entry } of toMerge) {
      await this.delete(entry.topic);
    }
  }

  // --- Private ---

  /** True if index.md already exists on disk. */
  private async indexExists(): Promise<boolean> {
    try {
      await fs.promises.access(path.join(this._config.dir, "index.md"));
      return true;
    } catch {
      return false;
    }
  }

  /** Reads index.md into _indexCache (cached). Returns an empty index on missing/corrupt file. */
  private async readIndex(): Promise<MemoryIndex> {
    if (this._indexCache !== null) return this._indexCache;
    try {
      const content = await fs.promises.readFile(
        path.join(this._config.dir, "index.md"),
        "utf-8",
      );
      this._indexCache = this.parseIndexMd(content);
      return this._indexCache;
    } catch {
      this._indexCache = { entries: [], totalTopics: 0, lastUpdated: new Date().toISOString() };
      return this._indexCache;
    }
  }

  /** 将索引渲染为 markdown 并写入磁盘，同时刷新 _indexCache（并作废小写化缓存）。 */
  private async writeIndex(index: MemoryIndex): Promise<void> {
    this._indexCache = index;
    this._lowercasedCache = null; // 索引已变更，下次 search 重建小写镜像
    const lines = [
      "# Memory Index",
      "> Updated: " + index.lastUpdated + " | Topics: " + String(index.totalTopics),
      "",
    ];
    for (const entry of index.entries) {
      lines.push("## " + entry.topic);
      lines.push(entry.summary);
      for (const point of entry.keyPoints) {
        lines.push("- " + point);
      }
      const accessLabel = entry.accessCount != null ? " | Access: " + String(entry.accessCount) : "";
      lines.push("_Updated: " + entry.lastUpdated + accessLabel + "_");
      lines.push("");
    }
    await fs.promises.writeFile(
      path.join(this._config.dir, "index.md"),
      lines.join("\n"),
      "utf-8",
    );
  }

  /** Upserts a topic's row in the index, preserving the prior access count if the new one is 0. */
  /**
   * 在索引中 upsert 一条主题记录；新条目的访问计数若为 0，
   * 则沿用已有记录的计数。仅做一次线性扫描（原实现 find+findIndex 扫描两次）。
   */
  private async updateIndex(entry: MemoryEntry): Promise<void> {
    const index = await this.readIndex();
    const indexEntry: MemoryIndexEntry = {
      topic: entry.topic,
      summary: entry.summary,
      keyPoints: entry.keyPoints,
      lastUpdated: new Date().toISOString(),
      accessCount: entry.accessCount,
    };
    // 单次扫描定位既有条目位置，同时拿到旧访问计数
    const existingIdx = index.entries.findIndex((e) => e.topic === entry.topic);
    if (existingIdx >= 0) {
      const existingAccess = index.entries[existingIdx]!.accessCount;
      if (indexEntry.accessCount === 0 && existingAccess) {
        indexEntry.accessCount = existingAccess;
      }
      index.entries[existingIdx] = indexEntry;
    } else {
      index.entries.push(indexEntry);
    }
    index.totalTopics = index.entries.length;
    index.lastUpdated = new Date().toISOString();
    await this.writeIndex(index);
  }

  /**
   * 仅更新索引中某主题的访问计数（轻量路径），用于 recall() 高频写场景，
   * 避免每次都走完整的 updateIndex（会重置 summary/keyPoints 等字段）。
   */
  private async bumpAccessCount(topic: string, accessCount: number): Promise<void> {
    const index = await this.readIndex();
    const entry = index.entries.find((e) => e.topic === topic);
    if (entry) {
      entry.accessCount = accessCount;
      index.lastUpdated = new Date().toISOString();
      await this.writeIndex(index);
    }
  }

  /**
   * 返回（必要时构建）条目的小写化镜像。任何 writeIndex 都会把缓存置空，
   * 因此这里能安全假定其与当前 index 同步。
   */
  private getLowercasedIndex(): { topic: string; summary: string; keyPoints: string[] }[] {
    if (this._lowercasedCache !== null) return this._lowercasedCache;
    const idx = this._indexCache;
    if (idx === null) return [];
    this._lowercasedCache = idx.entries.map((e) => ({
      topic: e.topic.toLowerCase(),
      summary: e.summary.toLowerCase(),
      keyPoints: e.keyPoints.map((p) => p.toLowerCase()),
    }));
    return this._lowercasedCache;
  }

  /** Serializes a MemoryEntry to its on-disk markdown format (content + summary + key points + metadata). */
  private renderTopicMd(entry: MemoryEntry): string {
    const lines = [
      "# " + entry.topic,
      "",
      entry.content,
      "",
      "## Summary",
      entry.summary,
      "",
      "## Key Points",
    ];
    for (const p of entry.keyPoints) {
      lines.push("- " + p);
    }
    lines.push("");
    lines.push("---");
    lines.push("_Last updated: " + entry.lastUpdated.toISOString() + " | Access count: " + String(entry.accessCount) + "_");
    return lines.join("\n");
  }

  /** Parses a topic markdown file back into a MemoryEntry. Regex-extracts summary, key points, and access count. */
  private parseTopicMd(content: string, topic: string): MemoryEntry {
    const summaryMatch = content.match(/## Summary\n([\s\S]*?)(?=\n##|\n---|$)/);
    const keyPointsMatch = content.match(/## Key Points\n([\s\S]*?)(?=\n##|\n---|$)/);
    const bodyMatch = content.match(/^# .+\n\n([\s\S]*?)\n## Summary/);
    const points: string[] = [];
    if (keyPointsMatch?.[1]) {
      for (const line of keyPointsMatch[1].split("\n")) {
        if (line.startsWith("- ")) points.push(line.slice(2));
      }
    }
    const accessMatch = content.match(/Access count: (\d+)/);
    return {
      topic,
      content: bodyMatch?.[1]?.trim() ?? "",
      summary: summaryMatch?.[1]?.trim() ?? "",
      keyPoints: points,
      lastUpdated: new Date(),
      accessCount: accessMatch?.[1] ? parseInt(accessMatch[1], 10) : 0,
    };
  }

  /** Parses index.md into a MemoryIndex by splitting on `## ` section headers. */
  private parseIndexMd(content: string): MemoryIndex {
    const entries: MemoryIndexEntry[] = [];
    const sections = content.split(/^## /m).slice(1);
    for (const section of sections) {
      const titleLine = section.split("\n")[0]?.trim() ?? "";
      if (titleLine === "") continue;
      const summaryMatch = section.match(/^(.+?)(?=\n-|\n_|\n##|\n*$)/s);
      const pointMatches = [...section.matchAll(/^- (.+)$/gm)];
      const points = pointMatches.map((m) => m[1]!);
      const updatedMatch = section.match(/_Updated: (.+?)(?: \| Access: (\d+))?_/);
      entries.push({
        topic: titleLine,
        summary: summaryMatch?.[1]?.trim() ?? "",
        keyPoints: points,
        lastUpdated: updatedMatch?.[1] ?? new Date().toISOString(),
        accessCount: updatedMatch?.[2] ? parseInt(updatedMatch[2], 10) : 0,
      });
    }
    return {
      entries,
      totalTopics: entries.length,
      lastUpdated: new Date().toISOString(),
    };
  }
}

/**
 * Slugifies a topic name for use as a filename. Lowercases, replaces
 * non-alphanumeric (incl. CJK via the 一-鿿 range) runs with `-`, trims,
 * and caps length at 64 chars.
 */
function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9一-鿿]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 64);
}

/**
 * Rough token estimate: ~4 chars/token, minimum 1. Used only for budgeting
 * context windows (not billing), so the approximation is acceptable.
 */
function estimateTokens(text: string): number {
  if (!text) return 0;
  const tokens = Math.floor(text.length / 4);
  return tokens === 0 ? 1 : tokens;
}
