/**
 * SemanticMemoryManager manages memories with embedding-based semantic retrieval.
 * Ported from modules/agent/harness_embedding.go.
 */

// ---------------------------------------------------------------------------
// EmbeddingVector
// ---------------------------------------------------------------------------

/** EmbeddingVector is a dense float vector returned by an EmbeddingProvider. */
export type EmbeddingVector = Float32Array;

// ---------------------------------------------------------------------------
// EmbeddingProvider
// ---------------------------------------------------------------------------

/**
 * EmbeddingProvider is the abstraction over embedding backends (OpenAI,
 * local models, etc.). embed() returns a vector for the text; dimension()
 * reports the fixed vector length.
 */
export interface EmbeddingProvider {
  embed(signal: AbortSignal | undefined, text: string): Promise<EmbeddingVector>;
  dimension(): number;
}

// ---------------------------------------------------------------------------
// SemanticMemoryEntry
// ---------------------------------------------------------------------------

/** SemanticMemoryEntry is a single stored memory with its embedding and access metadata. */
export interface SemanticMemoryEntry {
  key: string;
  content: string;
  embedding: EmbeddingVector | null;
  createdAt: Date;
  accessedAt: Date;
  accessCount: number;
  ttlMs: number; // 0 = no expiry
}

// ---------------------------------------------------------------------------
// SemanticMemoryConfig
// ---------------------------------------------------------------------------

/** SemanticMemoryConfig tunes the semantic memory manager: provider, capacity, TTL, and dedup threshold. */
export interface SemanticMemoryConfig {
  provider: EmbeddingProvider | null;
  maxEntries: number; // 0 = unlimited
  defaultTTL: number; // 0 = no expiry (milliseconds)
  similarityThreshold: number; // cosine similarity threshold for dedup
}

// ---------------------------------------------------------------------------
// SemanticMemoryManager
// ---------------------------------------------------------------------------

export class SemanticMemoryManager {
  private _config: SemanticMemoryConfig;
  private _entries: Map<string, SemanticMemoryEntry>;

  /**
   * Constructs a manager applying defaults (no provider, unlimited entries,
   * no TTL, 0.95 cosine dedup threshold) overridden by any provided partial config.
   */
  constructor(config?: Partial<SemanticMemoryConfig>) {
    const similarityThreshold = config?.similarityThreshold ?? 0.95;
    this._config = {
      provider: config?.provider ?? null,
      maxEntries: config?.maxEntries ?? 0,
      defaultTTL: config?.defaultTTL ?? 0,
      similarityThreshold,
    };
    this._entries = new Map();
  }

  /** Store saves a memory entry with its embedding. */
  async store(signal: AbortSignal | undefined, key: string, content: string): Promise<void> {
    // Generate embedding outside the lock (single-threaded JS, but maintains pattern)
    let embedding: EmbeddingVector | null = null;
    if (this._config.provider) {
      try {
        embedding = await this._config.provider.embed(signal, content);
      } catch {
        embedding = null;
      }
    }

    if (this._config.maxEntries > 0 && this._entries.size >= this._config.maxEntries) {
      this.evictOldest();
    }

    const now = new Date();
    const entry: SemanticMemoryEntry = {
      key,
      content,
      embedding,
      createdAt: now,
      accessedAt: now,
      accessCount: 0,
      ttlMs: this._config.defaultTTL,
    };

    // Check for near-duplicate
    if (this._config.provider && embedding) {
      const existing = this.findSimilar(embedding, this._config.similarityThreshold);
      if (existing !== "") {
        this._entries.delete(existing);
      }
    }

    this._entries.set(key, entry);
  }

  /** Recall returns memories ranked by relevance to the query. */
  async recall(signal: AbortSignal | undefined, query: string, maxResults: number): Promise<SemanticMemoryEntry[]> {
    // Generate query embedding
    let queryEmb: EmbeddingVector | null = null;
    if (this._config.provider) {
      try {
        queryEmb = await this._config.provider.embed(signal, query);
      } catch {
        queryEmb = null;
      }
    }

    // Score all non-expired entries
    type Scored = { entry: SemanticMemoryEntry; score: number };
    const results: Scored[] = [];
    const now = new Date();

    for (const entry of this._entries.values()) {
      // Check TTL
      if (entry.ttlMs > 0 && now.getTime() - entry.createdAt.getTime() > entry.ttlMs) {
        continue;
      }

      let score = 0;
      if (queryEmb && entry.embedding) {
        score = cosineSimilarity(queryEmb, entry.embedding);
      } else {
        score = keywordScore(query, entry.content);
      }

      if (score > 0) {
        results.push({ entry, score });
      }
    }

    // Sort by score descending
    results.sort((a, b) => b.score - a.score);

    if (maxResults > 0 && results.length > maxResults) {
      results.length = maxResults;
    }

    return results.map((r) => r.entry);
  }

  /** delete removes a memory entry by key. */
  delete(key: string): void {
    this._entries.delete(key);
  }

  /** Len returns the number of stored entries. */
  get length(): number {
    return this._entries.size;
  }

  /** cleanupExpired removes all entries whose TTL has elapsed and returns the count removed. */
  cleanupExpired(): number {
    const now = new Date();
    let cleaned = 0;
    for (const [key, entry] of this._entries) {
      if (entry.ttlMs > 0 && now.getTime() - entry.createdAt.getTime() > entry.ttlMs) {
        this._entries.delete(key);
        cleaned++;
      }
    }
    return cleaned;
  }

  /** evictOldest finds and removes the entry with the earliest createdAt (LRU-style eviction). */
  private evictOldest(): void {
    let oldestKey = "";
    let oldestTime = Infinity;
    for (const [k, e] of this._entries) {
      if (e.createdAt.getTime() < oldestTime) {
        oldestKey = k;
        oldestTime = e.createdAt.getTime();
      }
    }
    if (oldestKey) this._entries.delete(oldestKey);
  }

  /** findSimilar returns the key of an existing entry whose embedding cosine-similarity >= threshold, else "". */
  private findSimilar(emb: EmbeddingVector, threshold: number): string {
    for (const [key, entry] of this._entries) {
      if (!entry.embedding) continue;
      if (cosineSimilarity(emb, entry.embedding) >= threshold) {
        return key;
      }
    }
    return "";
  }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/** cosineSimilarity computes the cosine of the angle between two vectors; returns 0 for empty/mismatched lengths. */
function cosineSimilarity(a: EmbeddingVector, b: EmbeddingVector): number {
  if (a.length !== b.length || a.length === 0) return 0;
  let dot = 0;
  let normA = 0;
  let normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i]! * b[i]!;
    normA += a[i]! * a[i]!;
    normB += b[i]! * b[i]!;
  }
  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

/** keywordScore computes a 0..1 overlap ratio of query terms found in content (fallback when no embeddings). */
function keywordScore(query: string, content: string): number {
  if (!query || !content) return 0;
  const queryTerms = tokenize(query);
  const contentLower = content.toLowerCase();
  let matches = 0;
  for (const term of queryTerms) {
    if (contentLower.includes(term)) matches++;
  }
  if (queryTerms.length === 0) return 0;
  return matches / queryTerms.length;
}

/** tokenize splits a string on whitespace into non-empty lowercased terms. */
function tokenize(s: string): string[] {
  return s.split(/[\s]+/).filter((t) => t.length > 0);
}
