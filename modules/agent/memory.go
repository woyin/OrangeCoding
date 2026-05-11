package agent

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// MemoryStore provides file-backed key-value storage for agent memory.
// Each key is stored as a separate text file under the configured directory.
type MemoryStore struct {
	dir string
}

// NewMemoryStore creates a new MemoryStore rooted at the given directory.
// The directory is created lazily on first write.
func NewMemoryStore(dir string) *MemoryStore {
	return &MemoryStore{dir: dir}
}

// Write stores the value under the given key. The key is used as the filename
// (with .txt extension appended) under the store directory.
func (m *MemoryStore) Write(key, value string) error {
	if err := os.MkdirAll(m.dir, 0755); err != nil {
		return fmt.Errorf("memory store: mkdir: %w", err)
	}
	path := m.keyPath(key)
	if err := os.WriteFile(path, []byte(value), 0644); err != nil {
		return fmt.Errorf("memory store: write: %w", err)
	}
	return nil
}

// Read retrieves the value for the given key.
// Returns an error if the key does not exist.
func (m *MemoryStore) Read(key string) (string, error) {
	data, err := os.ReadFile(m.keyPath(key))
	if err != nil {
		return "", fmt.Errorf("memory store: read: %w", err)
	}
	return string(data), nil
}

// List returns all stored keys (filenames without the .txt extension).
func (m *MemoryStore) List() ([]string, error) {
	entries, err := os.ReadDir(m.dir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("memory store: list: %w", err)
	}

	var keys []string
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		name := entry.Name()
		if strings.HasSuffix(name, ".txt") {
			keys = append(keys, strings.TrimSuffix(name, ".txt"))
		}
	}
	return keys, nil
}

// Recall returns keys whose names contain the query substring.
func (m *MemoryStore) Recall(query string) ([]string, error) {
	keys, err := m.List()
	if err != nil {
		return nil, err
	}

	var matches []string
	lower := strings.ToLower(query)
	for _, k := range keys {
		if strings.Contains(strings.ToLower(k), lower) {
			matches = append(matches, k)
		}
	}
	return matches, nil
}

// keyPath returns the full file path for a given key.
func (m *MemoryStore) keyPath(key string) string {
	return filepath.Join(m.dir, key+".txt")
}
