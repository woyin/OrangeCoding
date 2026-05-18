package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// FileCheckpointStore persists harness checkpoints as JSON files.
type FileCheckpointStore struct {
	dir string
	ttl time.Duration // Phase 14: optional TTL for automatic cleanup
}

// NewFileCheckpointStore creates a file-backed checkpoint store.
func NewFileCheckpointStore(dir string) *FileCheckpointStore {
	return &FileCheckpointStore{dir: dir}
}

// NewFileCheckpointStoreWithTTL creates a file-backed store with automatic TTL cleanup.
func NewFileCheckpointStoreWithTTL(dir string, ttl time.Duration) *FileCheckpointStore {
	return &FileCheckpointStore{dir: dir, ttl: ttl}
}

// Save writes a checkpoint atomically using write-to-temp + rename.
func (s *FileCheckpointStore) Save(ctx context.Context, cp HarnessCheckpoint) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if cp.RunID == "" {
		return fmt.Errorf("file checkpoint store: run id is required")
	}
	if err := os.MkdirAll(s.dir, 0o755); err != nil {
		return fmt.Errorf("file checkpoint store: mkdir: %w", err)
	}
	cp = cloneHarnessCheckpoint(cp)
	cp.UpdatedAt = time.Now().UTC()
	data, err := json.MarshalIndent(cp, "", "  ")
	if err != nil {
		return fmt.Errorf("file checkpoint store: marshal: %w", err)
	}

	// Phase 14: Atomic write via temp file + rename
	tmpPath := s.pathFor(cp.RunID) + ".tmp"
	if err := os.WriteFile(tmpPath, data, 0o644); err != nil {
		return fmt.Errorf("file checkpoint store: write temp: %w", err)
	}
	if err := os.Rename(tmpPath, s.pathFor(cp.RunID)); err != nil {
		_ = os.Remove(tmpPath) // cleanup temp file
		return fmt.Errorf("file checkpoint store: rename: %w", err)
	}
	return nil
}

// Load reads a checkpoint by run ID.
func (s *FileCheckpointStore) Load(ctx context.Context, runID string) (HarnessCheckpoint, error) {
	if err := ctx.Err(); err != nil {
		return HarnessCheckpoint{}, err
	}
	data, err := os.ReadFile(s.pathFor(runID))
	if err != nil {
		return HarnessCheckpoint{}, fmt.Errorf("file checkpoint store: read: %w", err)
	}
	var cp HarnessCheckpoint
	if err := json.Unmarshal(data, &cp); err != nil {
		return HarnessCheckpoint{}, fmt.Errorf("file checkpoint store: unmarshal: %w", err)
	}
	return cloneHarnessCheckpoint(cp), nil
}

// List returns summaries for checkpoints matching the given prefix.
func (s *FileCheckpointStore) List(ctx context.Context, prefix string) ([]CheckpointSummary, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	entries, err := os.ReadDir(s.dir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("file checkpoint store: read dir: %w", err)
	}

	var summaries []CheckpointSummary
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}
		runID := strings.TrimSuffix(entry.Name(), ".json")
		if prefix != "" && !hasPrefix(runID, prefix) {
			continue
		}

		// Read only enough to build summary
		data, err := os.ReadFile(s.pathFor(runID))
		if err != nil {
			continue
		}
		var cp HarnessCheckpoint
		if err := json.Unmarshal(data, &cp); err != nil {
			continue
		}

		// TTL check
		if s.ttl > 0 && time.Since(cp.UpdatedAt) > s.ttl {
			_ = s.Delete(ctx, runID)
			continue
		}

		summaries = append(summaries, cp.Summary())
	}

	// Sort by UpdatedAt descending (most recent first)
	sort.Slice(summaries, func(i, j int) bool {
		return summaries[i].UpdatedAt.After(summaries[j].UpdatedAt)
	})
	return summaries, nil
}

// Delete removes a checkpoint file by run ID.
func (s *FileCheckpointStore) Delete(ctx context.Context, runID string) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	path := s.pathFor(runID)
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("file checkpoint store: delete: %w", err)
	}
	return nil
}

// CleanupExpired removes all checkpoints older than the configured TTL.
func (s *FileCheckpointStore) CleanupExpired(ctx context.Context) (int, error) {
	if s.ttl <= 0 {
		return 0, nil
	}
	entries, err := os.ReadDir(s.dir)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, fmt.Errorf("file checkpoint store: cleanup read dir: %w", err)
	}
	cleaned := 0
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}
		runID := strings.TrimSuffix(entry.Name(), ".json")
		data, err := os.ReadFile(s.pathFor(runID))
		if err != nil {
			continue
		}
		var cp HarnessCheckpoint
		if err := json.Unmarshal(data, &cp); err != nil {
			continue
		}
		if time.Since(cp.UpdatedAt) > s.ttl {
			if err := s.Delete(ctx, runID); err != nil {
				return cleaned, err
			}
			cleaned++
		}
	}
	return cleaned, nil
}

func (s *FileCheckpointStore) pathFor(runID string) string {
	return filepath.Join(s.dir, runID+".json")
}
