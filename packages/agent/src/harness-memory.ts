/**
 * HarnessMemoryManager recalls and learns stable facts for context assembly.
 * Ported from modules/agent/harness_memory.go.
 *
 * Storage-backed key/value memory: facts are extracted from agent output as
 * "FACT:" lines, written under a sanitized key, and later recalled by
 * substring-matching the query against key+value. Cheap and deterministic;
 * for semantic recall see TieredMemoryManager / SemanticMemoryManager.
 */

import type { ContextBlock } from "./harness-state.js";
import type { MemoryStore } from "./memory.js";

const FACT_LINE_PATTERN = /^[\t ]*FACT:[\t ]*(.+?)[\t ]*$/gm;

export class HarnessMemoryManager {
  private _store: MemoryStore | null;

  constructor(store: MemoryStore | null) {
    this._store = store;
  }

  /**
   * Scan every stored key and return matching entries as stable context
   * blocks. Matching is a case-insensitive substring test over key+value
   * against any recall term (>=2 codepoints); an empty query returns all.
   * O(n) over the store; fine for hundreds of facts.
   */
  async recall(_signal: AbortSignal | undefined, query: string): Promise<ContextBlock[]> {
    if (!this._store) return [];

    const keys = await this._store.list();
    const queryTerms = splitRecallTerms(query);
    const blocks: ContextBlock[] = [];

    for (const key of keys) {
      const value = await this._store.read(key);
      if (memoryMatches(key, value, queryTerms)) {
        blocks.push({
          kind: "memory",
          content: `Memory[${key}]: ${value}`,
          stable: true,
          priority: 80,
          tokenEstimate: estimateTextTokens(`Memory[${key}]: ${value}`),
        });
      }
    }
    return blocks;
  }

  /**
   * Mine `observation` for "FACT: ..." lines and persist each under a
   * sanitized key derived from the fact text. Idempotent for identical facts
   * (same text -> same key). No-op when no store is configured.
   */
  async learnObservation(_signal: AbortSignal | undefined, observation: string): Promise<void> {
    if (!this._store) return;

    const matches = [...observation.matchAll(FACT_LINE_PATTERN)];
    for (const match of matches) {
      const fact = match[1]!.trim();
      if (!fact) continue;
      const key = memoryKeyForFact(fact);
      await this._store.write(key, fact);
    }
  }
}

/** Split a query into lowercase recall terms, dropping terms shorter than 2 codepoints. */
function splitRecallTerms(query: string): string[] {
  const fields = query.toLowerCase().split(/[\s\t\n,，:：]+/);
  const terms: string[] = [];
  for (const field of fields) {
    const trimmed = field.trim();
    if ([...trimmed].length >= 2) {
      terms.push(trimmed);
    }
  }
  return terms;
}

//** True if any term is a substring of key+value (case-insensitive); empty terms = match all. */
function memoryMatches(key: string, value: string, terms: string[]): boolean {
  if (terms.length === 0) return true;
  const text = (key + "\n" + value).toLowerCase();
  for (const term of terms) {
    if (text.includes(term)) return true;
  }
  return false;
}

/** Derive a stable storage key from a fact: lowercase, separators->dashes, truncated to 32 codepoints. */
function memoryKeyForFact(fact: string): string {
  let sanitized = fact.toLowerCase();
  sanitized = sanitized.replace(/[ \t\n/\\:：，,]/g, "-");
  const runes = [...sanitized];
  const truncated = runes.slice(0, 32).join("");
  return "fact-" + truncated.replace(/-+$/g, "");
}

function estimateTextTokens(text: string): number {
  if (!text) return 0;
  const tokens = Math.floor(text.length / 4);
  return tokens === 0 ? 1 : tokens;
}
