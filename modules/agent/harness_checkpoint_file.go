package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// FileCheckpointStore persists harness checkpoints as JSON files.
type FileCheckpointStore struct {
	dir string
}

// NewFileCheckpointStore creates a file-backed checkpoint store.
func NewFileCheckpointStore(dir string) *FileCheckpointStore {
	return &FileCheckpointStore{dir: dir}
}

// Save writes a checkpoint to <dir>/<runID>.json.
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
	data, err := json.MarshalIndent(cp, "", "  ")
	if err != nil {
		return fmt.Errorf("file checkpoint store: marshal: %w", err)
	}
	if err := os.WriteFile(s.pathFor(cp.RunID), data, 0o644); err != nil {
		return fmt.Errorf("file checkpoint store: write: %w", err)
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

func (s *FileCheckpointStore) pathFor(runID string) string {
	return filepath.Join(s.dir, runID+".json")
}
