/**
 * Tiered Memory Manager — layered memory inspired by Hermes Agent.
 *
 * Architecture:
 *   Tier 0 (Core):     MEMORY.md — always loaded, strict ~200 token limit.
 *                      Holds identity, preferences, critical facts.
 *   Tier 1 (Working):  In-memory scratchpad for current session.
 *                      Auto-expires entries after N turns.
 *   Tier 2 (Long-term): Topic-based long-term memory via LongMemoryStore.
 *                      Loaded on-demand, strict token budget per retrieval.
 *   Tier 3 (Semantic):  Optional embedding-based recall via SemanticMemoryManager.
 *
 * Token discipline:
 *   - Each tier declares a token budget.
 *   - recall() respects budgets: core always fits, working capped,
 *     long-term truncated to budget, semantic best-N.
 *   - Total memory injection into prompt is bounded by totalBudgetTokens.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import type { ContextBlock } from "./harness-state.js";
import type { LongMemoryStore, MemoryEntry } from "./long-memory.js";

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface TieredMemoryConfig {
  /** Root directory for memory files. Required. */
  dir: string;
  /** Token budget for Tier 0 (core memory). Default 200. */
  coreBudgetTokens: number;
  /** Token budget for Tier 1 (working memory). Default 300. */
  workingBudgetTokens: number;
  /** Token budget for Tier 2 (long-term index context). Default 500. */
  longTermBudgetTokens: number;
  /** Token budget for Tier 3 (semantic recall). Default 300. */
  semanticBudgetTokens: number;
  /** Maximum combined tokens injected into prompt from all tiers. Default 1200. */
  totalBudgetTokens: number;
  /** Working memory entry max age in turns. Default 10. */
  workingMaxTurns: number;
  /** Working memory max entries. Default 20. */
  workingMaxEntries: number;
}

const DEFAULT_CONFIG: TieredMemoryConfig = {
  dir: "",
  coreBudgetTokens: 200,
  workingBudgetTokens: 300,
  longTermBudgetTokens: 500,
  semanticBudgetTokens: 300,
  totalBudgetTokens: 1200,
  workingMaxTurns: 10,
  workingMaxEntries: 20,
};

// ---------------------------------------------------------------------------
// Working Memory Entry
// ---------------------------------------------------------------------------

/** A single transient working-memory record. `turn` enables age-based expiry. */
interface WorkingEntry {
  content: string;
  turn: number;
  priority: number;
}

// ---------------------------------------------------------------------------
// TieredMemoryManager
// ---------------------------------------------------------------------------

export class TieredMemoryManager {
  private _config: TieredMemoryConfig;
  private _corePath: string;
  private _working: WorkingEntry[];
  private _longMemory?: LongMemoryStore;
  private _currentTurn: number;
  private _initialized: boolean;
  private _coreCache: string | null;

  constructor(config: Partial<TieredMemoryConfig> & { dir: string }, longMemory?: LongMemoryStore) {
    this._config = { ...DEFAULT_CONFIG, ...config };
    this._corePath = path.join(this._config.dir, "MEMORY.md");
    this._working = [];
    this._longMemory = longMemory;
    this._currentTurn = 0;
    this._initialized = false;
    this._coreCache = null;
  }

  // --- Initialization ---

  /**
   * Initialize the memory directory and core memory file.
   * Idempotent: safe to call multiple times. Creates the directory tree
   * (recursive) and seeds MEMORY.md if absent, then initializes the
   * optional long-term store. Subsequent calls are a fast no-op once
   * `_initialized` is set.
   */
  async init(): Promise<void> {
    if (this._initialized) return;
    await fs.promises.mkdir(this._config.dir, { recursive: true });
    try {
      await fs.promises.access(this._corePath);
    } catch {
      await fs.promises.writeFile(this._corePath, "# Core Memory\n", "utf-8");
    }
    if (this._longMemory) {
      await this._longMemory.init();
    }
    this._initialized = true;
  }

  // --- Tier 0: Core Memory ---
  // MEMORY.md is the always-loaded identity/preferences layer.
  // A `_coreCache` field avoids repeated disk reads within a session.

  /**
   * Read the core memory file, returning a cached copy after the first
   * load. Returns an empty string if the file is missing or unreadable.
   */
  async readCore(): Promise<string> {
    await this.init();
    if (this._coreCache !== null) return this._coreCache;
    try {
      this._coreCache = await fs.promises.readFile(this._corePath, "utf-8");
      return this._coreCache;
    } catch {
      this._coreCache = "";
      return "";
    }
  }

  /**
   * Overwrite core memory, truncating to the core token budget.
   * Updates the in-memory cache so subsequent readCore() calls are consistent.
   */
  async writeCore(content: string): Promise<void> {
    await this.init();
    const trimmed = enforceTokenLimit(content, this._config.coreBudgetTokens);
    this._coreCache = trimmed;
    await fs.promises.writeFile(this._corePath, trimmed, "utf-8");
  }

  /**
   * Append a single fact (as a bullet line) to core memory.
   * Reads from the cache when available to avoid an extra disk read,
   * then re-trims to the budget before persisting.
   */
  async appendCore(fact: string): Promise<void> {
    await this.init();
    const current = this._coreCache ?? await this.readCore();
    const line = "- " + fact.trim() + "\n";
    const updated = current + line;
    const trimmed = enforceTokenLimit(updated, this._config.coreBudgetTokens);
    this._coreCache = trimmed;
    await fs.promises.writeFile(this._corePath, trimmed, "utf-8");
  }

  // --- Tier 1: Working Memory ---
  // An in-process scratchpad of transient entries tagged with the
  // current turn and a priority. Entries auto-expire after
  // `workingMaxTurns` turns; exceeding `workingMaxEntries` triggers
  // eviction of the lowest-priority/oldest entries.

  /**
   * Push a working-memory entry at the current turn. Triggers eviction
   * if the entry count exceeds the configured maximum.
   */
  pushWorking(content: string, priority = 50): void {
    this._working.push({ content, turn: this._currentTurn, priority });
    if (this._working.length > this._config.workingMaxEntries) {
      this.evictWorking();
    }
  }

  /**
   * Advance the turn counter and prune entries older than the cutoff.
   * Called once per agent turn so stale context does not accumulate.
   */
  advanceTurn(): void {
    this._currentTurn++;
    const cutoff = this._currentTurn - this._config.workingMaxTurns;
    this._working = this._working.filter((e) => e.turn >= cutoff);
  }

  /**
   * Return working-memory entries within the age cutoff, ordered by
   * descending priority and truncated to the working token budget.
   * Does not mutate state; the most important entries win budget.
   */
  recallWorking(): string[] {
    const cutoff = this._currentTurn - this._config.workingMaxTurns;
    const valid = this._working.filter((e) => e.turn >= cutoff);
    valid.sort((a, b) => b.priority - a.priority);
    return truncateEntries(valid, this._config.workingBudgetTokens);
  }

  // --- Tier 2: Long-Term Memory ---
  // On-demand topic-based recall backed by LongMemoryStore. Bounded by
  // `longTermBudgetTokens`; falls back to the most recent index entries
  // when no search match is found.

  /**
   * Search long-term memory for entries relevant to `query` and render
   * them as markdown within the long-term token budget. Returns "" when
   * the store is absent or empty, or a recency fallback when no match.
   */
  async recallLongTerm(query: string): Promise<string> {
    if (!this._longMemory) return "";
    await this.init();

    const index = await this._longMemory.getIndex();
    if (index.entries.length === 0) return "";

    // Search for relevant entries
    const results = await this._longMemory.search(query);
    if (results.length === 0) {
      // Fallback: return top entries by recency, truncated to budget
      const entries = index.entries.slice(0, 5);
      return renderIndexEntries(entries, this._config.longTermBudgetTokens);
    }

    // Render matched entries, prioritized by relevance order
    return renderIndexEntries(results, this._config.longTermBudgetTokens);
  }

  /** Persist a new entry to the long-term store (no-op if absent). */
  async storeLongTerm(entry: MemoryEntry): Promise<void> {
    if (!this._longMemory) return;
    await this.init();
    await this._longMemory.store(entry);
  }

  // --- Tier 3: Semantic (placeholder for future embedding integration) ---

  // Semantic recall is delegated to SemanticMemoryManager externally.
  // This tier is opt-in and not managed by TieredMemoryManager directly.

  // --- Unified Recall (for prompt injection) ---
  // recall() assembles the final ContextBlock list injected into the
  // prompt. It walks tiers in priority order (core > working > long-term)
  // and stops as soon as the combined `totalBudgetTokens` is reached,
  // guaranteeing the prompt never exceeds the configured budget.

  /**
   * Recall memory across all tiers into a prioritized list of context
   * blocks. Core memory is always included first; working and long-term
   * tiers fill the remaining budget. Tier 3 (semantic) is delegated to
   * an external manager and intentionally not included here.
   */
  async recall(query: string): Promise<ContextBlock[]> {
    await this.init();
    const blocks: ContextBlock[] = [];
    let usedTokens = 0;

    // Tier 0: Core — always included, always fits within its own budget
    const coreContent = await this.readCore();
    const coreTokens = estimateTokens(coreContent);
    if (coreTokens > 0 && usedTokens + coreTokens <= this._config.totalBudgetTokens) {
      blocks.push({
        kind: "memory",
        content: coreContent,
        stable: true,
        priority: 100,
        tokenEstimate: coreTokens,
      });
      usedTokens += coreTokens;
    }

    // Tier 1: Working — recent session context
    const workingItems = this.recallWorking();
    for (const item of workingItems) {
      const tokens = estimateTokens(item);
      if (usedTokens + tokens > this._config.totalBudgetTokens) break;
      blocks.push({
        kind: "memory",
        content: item,
        stable: false,
        priority: 80,
        tokenEstimate: tokens,
      });
      usedTokens += tokens;
    }

    // Tier 2: Long-term — on-demand, budget-capped
    const remaining = this._config.totalBudgetTokens - usedTokens;
    if (remaining > 50 && query) {
      const longTermContent = await this.recallLongTerm(query);
      const ltTokens = estimateTokens(longTermContent);
      if (ltTokens > 0) {
        const capped = Math.min(ltTokens, remaining);
        const truncated = enforceTokenLimit(longTermContent, capped);
        const actualTokens = estimateTokens(truncated);
        if (usedTokens + actualTokens <= this._config.totalBudgetTokens) {
          blocks.push({
            kind: "memory",
            content: truncated,
            stable: false,
            priority: 60,
            tokenEstimate: actualTokens,
          });
          usedTokens += actualTokens;
        }
      }
    }

    return blocks;
  }

  // --- Learning ---

  /**
   * Ingest free-form agent output: any line starting with "FACT:" is
   * extracted as a short core-memory fact (legacy convention), and the
   * first few non-empty lines are mirrored into working memory so the
   * next turn has immediate context. Facts are length-validated to keep
   * core memory compact.
   */
  async learn(content: string): Promise<void> {
    await this.init();

    // Extract facts from FACT: lines (backward compatible)
    const factPattern = /^[\t ]*FACT:[\t ]*(.+?)[\t ]*$/gm;
    let match: RegExpExecArray | null;
    while ((match = factPattern.exec(content)) !== null) {
      const fact = match[1]!.trim();
      if (fact.length > 5 && fact.length < 200) {
        await this.appendCore(fact);
      }
    }

    // Add to working memory
    const lines = content.split("\n").filter((l) => l.trim().length > 0);
    for (const line of lines.slice(0, 5)) {
      if (line.length > 10) {
        this.pushWorking(line.trim().slice(0, 200), 50);
      }
    }
  }

  // --- Cleanup ---

  /** Delegate compaction of the long-term store (drop stale entries). */
  async compact(): Promise<void> {
    if (this._longMemory) {
      await this._longMemory.compact();
    }
  }

  // --- Stats ---

  /**
   * Return a synchronous snapshot of memory usage. `coreTokens` is 0
   * because an accurate core read is async; callers needing the precise
   * figure should `await readCore()` then estimateTokens() it. Working
   * stats iterate the current entries in O(n).
   */
  getStats(): { coreTokens: number; workingEntries: number; workingTokens: number } {
    const coreTokens = 0; // Approximate — actual read requires async
    return {
      coreTokens,
      workingEntries: this._working.length,
      workingTokens: this._working.reduce((sum, e) => sum + estimateTokens(e.content), 0),
    };
  }

  // --- Private ---

  /**
   * Evict working entries until the count fits the configured maximum.
   * Sorts by descending priority then descending turn (most recent wins),
   * then drops from the front of the sorted array. The comparator is
   * stable for equal-priority entries so newer context is retained.
   */
  private evictWorking(): void {
    // Remove oldest, lowest priority entries
    this._working.sort((a, b) => b.priority - a.priority || b.turn - a.turn);
    while (this._working.length > this._config.workingMaxEntries) {
      this._working.shift();
    }
  }
}

// ---------------------------------------------------------------------------
// Utility Functions
// ---------------------------------------------------------------------------

/** Fast heuristic token estimator: ~4 chars/token (UTF-16 code units). */
function estimateTokens(text: string): number {
  if (!text) return 0;
  // Use text.length (UTF-16 code units) as fast approximation.
  // Acceptable for token estimation — avoids O(n) array spread.
  const tokens = Math.floor(text.length / 4);
  return tokens === 0 ? 1 : tokens;
}

function enforceTokenLimit(content: string, budget: number): string {
  const maxChars = budget * 4;
  if (content.length <= maxChars) return content;
  // Find the last newline within 100 chars of the limit
  const searchStart = Math.max(0, maxChars - 100);
  let cutAt = maxChars;
  for (let i = maxChars; i >= searchStart; i--) {
    if (content.charCodeAt(i) === 10) { // newline
      cutAt = i;
      break;
    }
  }
  return content.slice(0, cutAt) + "\n";
}

/**
 * Greedily pack entry contents into `budget` tokens, preserving the
 * caller-provided order. Stops at the first entry that would overflow.
 */
function truncateEntries(entries: WorkingEntry[], budget: number): string[] {
  const result: string[] = [];
  let used = 0;
  for (const entry of entries) {
    const tokens = estimateTokens(entry.content);
    if (used + tokens > budget) break;
    result.push(entry.content);
    used += tokens;
  }
  return result;
}

/**
 * Render long-term index entries as compact markdown ("## topic" + summary
 * + bullet key points), stopping as soon as the cumulative token budget
 * is reached.
 */
function renderIndexEntries(
  entries: ReadonlyArray<{ topic: string; summary: string; keyPoints: string[] }>,
  budget: number,
): string {
  const lines: string[] = [];
  let used = 0;
  for (const entry of entries) {
    const header = "## " + entry.topic;
    const body = header + "\n" + entry.summary;
    const tokens = estimateTokens(body);
    if (used + tokens > budget) break;
    lines.push(body);
    for (const point of entry.keyPoints) {
      const ptTokens = estimateTokens(point);
      if (used + tokens + ptTokens > budget) break;
      lines.push("- " + point);
      used += ptTokens;
    }
    used += tokens;
    lines.push("");
  }
  return lines.join("\n");
}
