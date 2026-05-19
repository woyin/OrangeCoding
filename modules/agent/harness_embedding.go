package agent

import (
	"context"
	"math"
	"sort"
	"sync"
	"time"
)

// EmbeddingVector is a dense float32 vector for semantic search.
type EmbeddingVector []float32

// EmbeddingProvider generates embeddings for text content.
type EmbeddingProvider interface {
	Embed(ctx context.Context, text string) (EmbeddingVector, error)
	Dimension() int
}

// SemanticMemoryEntry is a memory record with an embedding for similarity search.
type SemanticMemoryEntry struct {
	Key         string
	Content     string
	Embedding   EmbeddingVector
	CreatedAt   time.Time
	AccessAt    time.Time
	AccessCount uint32
	TTL         time.Duration // 0 = no expiry
}

// SemanticMemoryConfig configures the semantic memory manager.
type SemanticMemoryConfig struct {
	Provider            EmbeddingProvider
	MaxEntries          int           // 0 = unlimited
	DefaultTTL          time.Duration // 0 = no expiry
	SimilarityThreshold float32       // cosine similarity threshold for dedup (0.95 = near identical)
}

// SemanticMemoryManager manages memories with embedding-based semantic retrieval.
type SemanticMemoryManager struct {
	config  SemanticMemoryConfig
	mu      sync.RWMutex
	entries map[string]SemanticMemoryEntry
}

// NewSemanticMemoryManager creates a semantic memory manager.
func NewSemanticMemoryManager(config SemanticMemoryConfig) *SemanticMemoryManager {
	if config.SimilarityThreshold == 0 {
		config.SimilarityThreshold = 0.95
	}
	return &SemanticMemoryManager{
		config:  config,
		entries: make(map[string]SemanticMemoryEntry),
	}
}

// Store saves a memory entry with its embedding.
func (m *SemanticMemoryManager) Store(ctx context.Context, key, content string) error {
	// Generate embedding outside the lock to avoid blocking other operations.
	var embedding EmbeddingVector
	if m.config.Provider != nil {
		var err error
		embedding, err = m.config.Provider.Embed(ctx, content)
		if err != nil {
			embedding = nil
		}
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	if m.config.MaxEntries > 0 && len(m.entries) >= m.config.MaxEntries {
		m.evictOldest()
	}

	now := time.Now().UTC()
	entry := SemanticMemoryEntry{
		Key:       key,
		Content:   content,
		Embedding: embedding,
		CreatedAt: now,
		AccessAt:  now,
		TTL:       m.config.DefaultTTL,
	}

	// Check for near-duplicate
	if m.config.Provider != nil && embedding != nil {
		if existing := m.findSimilar(ctx, embedding, m.config.SimilarityThreshold); existing != "" {
			// Replace existing with newer version
			delete(m.entries, existing)
		}
	}

	m.entries[key] = entry
	return nil
}

// Recall returns memories ranked by relevance to the query.
// It combines semantic similarity (if embeddings available) with keyword matching.
func (m *SemanticMemoryManager) Recall(ctx context.Context, query string, maxResults int) ([]SemanticMemoryEntry, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	// Generate query embedding
	var queryEmb EmbeddingVector
	if m.config.Provider != nil {
		emb, err := m.config.Provider.Embed(ctx, query)
		if err == nil {
			queryEmb = emb
		}
	}

	// Score all non-expired entries
	type scored struct {
		entry SemanticMemoryEntry
		score float32
	}
	var results []scored
	now := time.Now().UTC()

	for _, entry := range m.entries {
		// Check TTL
		if entry.TTL > 0 && now.Sub(entry.CreatedAt) > entry.TTL {
			continue
		}

		var score float32
		if queryEmb != nil && entry.Embedding != nil {
			score = cosineSimilarity(queryEmb, entry.Embedding)
		} else {
			// Fallback: simple keyword match score
			score = keywordScore(query, entry.Content)
		}

		if score > 0 {
			results = append(results, scored{entry: entry, score: score})
		}
	}

	// Sort by score descending
	sort.Slice(results, func(i, j int) bool {
		return results[i].score > results[j].score
	})

	if maxResults > 0 && len(results) > maxResults {
		results = results[:maxResults]
	}

	entries := make([]SemanticMemoryEntry, len(results))
	for i, r := range results {
		entries[i] = r.entry
	}
	return entries, nil
}

// Delete removes a memory entry by key.
func (m *SemanticMemoryManager) Delete(key string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	delete(m.entries, key)
}

// Len returns the number of stored entries.
func (m *SemanticMemoryManager) Len() int {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return len(m.entries)
}

// CleanupExpired removes all entries that have exceeded their TTL.
func (m *SemanticMemoryManager) CleanupExpired() int {
	m.mu.Lock()
	defer m.mu.Unlock()
	now := time.Now().UTC()
	cleaned := 0
	for key, entry := range m.entries {
		if entry.TTL > 0 && now.Sub(entry.CreatedAt) > entry.TTL {
			delete(m.entries, key)
			cleaned++
		}
	}
	return cleaned
}

func (m *SemanticMemoryManager) evictOldest() {
	var oldestKey string
	var oldestTime time.Time
	for k, e := range m.entries {
		if oldestKey == "" || e.CreatedAt.Before(oldestTime) {
			oldestKey = k
			oldestTime = e.CreatedAt
		}
	}
	if oldestKey != "" {
		delete(m.entries, oldestKey)
	}
}

func (m *SemanticMemoryManager) findSimilar(ctx context.Context, emb EmbeddingVector, threshold float32) string {
	for key, entry := range m.entries {
		if entry.Embedding == nil {
			continue
		}
		if cosineSimilarity(emb, entry.Embedding) >= threshold {
			return key
		}
	}
	return ""
}

// cosineSimilarity computes the cosine similarity between two vectors.
func cosineSimilarity(a, b EmbeddingVector) float32 {
	if len(a) != len(b) || len(a) == 0 {
		return 0
	}
	var dot, normA, normB float32
	for i := range a {
		dot += a[i] * b[i]
		normA += a[i] * a[i]
		normB += b[i] * b[i]
	}
	if normA == 0 || normB == 0 {
		return 0
	}
	return dot / (float32(math.Sqrt(float64(normA))) * float32(math.Sqrt(float64(normB))))
}

// keywordScore returns a simple relevance score based on term overlap.
func keywordScore(query, content string) float32 {
	if query == "" || content == "" {
		return 0
	}
	// Simple: check if query terms appear in content
	queryTerms := tokenize(query)
	contentLower := toLower(content)
	matches := 0
	for _, term := range queryTerms {
		if containsSubstring(contentLower, term) {
			matches++
		}
	}
	if len(queryTerms) == 0 {
		return 0
	}
	return float32(matches) / float32(len(queryTerms))
}

func tokenize(s string) []string {
	var tokens []string
	current := make([]rune, 0)
	for _, r := range s {
		if r == ' ' || r == '\t' || r == '\n' {
			if len(current) > 0 {
				tokens = append(tokens, string(current))
				current = current[:0]
			}
		} else {
			current = append(current, r)
		}
	}
	if len(current) > 0 {
		tokens = append(tokens, string(current))
	}
	return tokens
}

func toLower(s string) string {
	runes := []rune(s)
	for i, r := range runes {
		if r >= 'A' && r <= 'Z' {
			runes[i] = r + ('a' - 'A')
		}
	}
	return string(runes)
}

func containsSubstring(s, substr string) bool {
	return len(s) >= len(substr) && findSubstring(s, substr)
}

func findSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		match := true
		for j := 0; j < len(substr); j++ {
			if s[i+j] != substr[j] {
				match = false
				break
			}
		}
		if match {
			return true
		}
	}
	return false
}
