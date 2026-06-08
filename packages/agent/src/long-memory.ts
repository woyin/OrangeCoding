import * as fs from "node:fs";
import * as path from "node:path";

export interface MemoryEntry {
  topic: string;
  content: string;
  summary: string;
  keyPoints: string[];
  lastUpdated: Date;
  accessCount: number;
}

export interface MemoryIndex {
  entries: MemoryIndexEntry[];
  totalTopics: number;
  lastUpdated: string;
}

export interface MemoryIndexEntry {
  topic: string;
  summary: string;
  keyPoints: string[];
  lastUpdated: string;
  accessCount?: number;
}

export interface LongMemoryConfig {
  dir: string;
  indexTokenBudget: number;
  topicTokenBudget: number;
  maxKeyPoints: number;
  maxTopics: number;
}

const DEFAULT_CONFIG: LongMemoryConfig = {
  dir: "",
  indexTokenBudget: 500,
  topicTokenBudget: 1000,
  maxKeyPoints: 5,
  maxTopics: 50,
};

export class LongMemoryStore {
  private _config: LongMemoryConfig;
  private _initialized = false;
  private _indexCache: MemoryIndex | null = null;

  constructor(config: Partial<LongMemoryConfig> & { dir: string }) {
    this._config = { ...DEFAULT_CONFIG, ...config };
  }

  async init(): Promise<void> {
    if (this._initialized) return;
    await fs.promises.mkdir(path.join(this._config.dir, "topics"), { recursive: true });
    await fs.promises.mkdir(path.join(this._config.dir, "summaries"), { recursive: true });
    if (!(await this.indexExists())) {
      await this.writeIndex({ entries: [], totalTopics: 0, lastUpdated: new Date().toISOString() });
    }
    this._initialized = true;
  }

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

  async recall(topic: string): Promise<MemoryEntry | undefined> {
    await this.init();
    const slug = slugify(topic);
    try {
      const content = await fs.promises.readFile(
        path.join(this._config.dir, "topics", slug + ".md"),
        "utf-8",
      );
      const entry = this.parseTopicMd(content, topic);
      // Increment access count and persist
      entry.accessCount += 1;
      entry.lastUpdated = new Date();
      await fs.promises.writeFile(
        path.join(this._config.dir, "topics", slug + ".md"),
        this.renderTopicMd(entry),
        "utf-8",
      );
      return entry;
    } catch {
      return undefined;
    }
  }

  async getIndex(): Promise<MemoryIndex> {
    await this.init();
    return this.readIndex();
  }

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

    for (const entry of sorted) {
      const block = "### " + entry.topic + "\n" + entry.summary + "\n";
      const blockTokens = estimateTokens(block);

      // Check if adding this entry would exceed budget
      if (used + blockTokens > budget) break;

      let pointLines = "";
      for (const point of entry.keyPoints) {
        const ptLine = "- " + point + "\n";
        const ptTokens = estimateTokens(ptLine);
        if (used + blockTokens + estimateTokens(pointLines) + ptTokens > budget) break;
        pointLines += ptLine;
      }

      entryLines.push(block + pointLines);
      used += blockTokens + estimateTokens(pointLines);
    }

    if (entryLines.length === 0) return "";
    return header + entryLines.join("\n");
  }

  async search(query: string): Promise<MemoryIndexEntry[]> {
    const index = await this.getIndex();
    const lower = query.toLowerCase();
    return index.entries.filter(
      (e) =>
        e.topic.toLowerCase().includes(lower) ||
        e.summary.toLowerCase().includes(lower) ||
        e.keyPoints.some((p) => p.toLowerCase().includes(lower)),
    );
  }

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

  private async indexExists(): Promise<boolean> {
    try {
      await fs.promises.access(path.join(this._config.dir, "index.md"));
      return true;
    } catch {
      return false;
    }
  }

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

  private async writeIndex(index: MemoryIndex): Promise<void> {
    this._indexCache = index;
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

  private async updateIndex(entry: MemoryEntry): Promise<void> {
    const index = await this.readIndex();
    const existingEntry = index.entries.find((e) => e.topic === entry.topic);
    const indexEntry: MemoryIndexEntry = {
      topic: entry.topic,
      summary: entry.summary,
      keyPoints: entry.keyPoints,
      lastUpdated: new Date().toISOString(),
      accessCount: entry.accessCount,
    };
    const existingIdx = index.entries.findIndex((e) => e.topic === entry.topic);
    if (existingIdx >= 0) {
      // Preserve existing access count if not overwritten
      if (indexEntry.accessCount === 0 && existingEntry?.accessCount) {
        indexEntry.accessCount = existingEntry.accessCount;
      }
      index.entries[existingIdx] = indexEntry;
    } else {
      index.entries.push(indexEntry);
    }
    index.totalTopics = index.entries.length;
    index.lastUpdated = new Date().toISOString();
    await this.writeIndex(index);
  }

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

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9一-鿿]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 64);
}

function estimateTokens(text: string): number {
  if (!text) return 0;
  const tokens = Math.floor(text.length / 4);
  return tokens === 0 ? 1 : tokens;
}
