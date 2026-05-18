package agent

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// HarnessState is the explicit state-machine phase for a harness run.
type HarnessState string

const (
	HarnessStateInit           HarnessState = "init"
	HarnessStateBuildContext   HarnessState = "build_context"
	HarnessStateModelCall      HarnessState = "model_call"
	HarnessStateGuardrailCheck HarnessState = "guardrail_check"
	HarnessStateToolDispatch   HarnessState = "tool_dispatch"
	HarnessStateObserve        HarnessState = "observe"
	HarnessStateMemoryUpdate   HarnessState = "memory_update"
	HarnessStateCheckpoint     HarnessState = "checkpoint"
	HarnessStateDecideNext     HarnessState = "decide_next"
	HarnessStateCompleted      HarnessState = "completed"
	HarnessStateStopped        HarnessState = "stopped"
	HarnessStateFailed         HarnessState = "failed"
)

// HarnessTraceEvent records a single state transition.
type HarnessTraceEvent struct {
	From      HarnessState      `json:"from"`
	To        HarnessState      `json:"to"`
	Reason    string            `json:"reason,omitempty"`
	Metadata  map[string]string `json:"metadata,omitempty"`
	CreatedAt time.Time         `json:"created_at"`
}

// HarnessCheckpoint is the durable state snapshot for a harness run.
type HarnessCheckpoint struct {
	RunID            string              `json:"run_id"`
	SessionID        core.SessionId      `json:"session_id"`
	Task             string              `json:"task"`
	State            HarnessState        `json:"state"`
	Iteration        uint32              `json:"iteration"`
	ToolCallsMade    uint32              `json:"tool_calls_made"`
	TokenUsage       core.TokenUsage     `json:"token_usage"`
	StopReason       StopReason          `json:"stop_reason,omitempty"`
	ContextBlocks    []ContextBlock      `json:"context_blocks,omitempty"`
	MemoryKeys       []string            `json:"memory_keys,omitempty"`
	RecentToolKeys   []string            `json:"recent_tool_keys,omitempty"`
	Trace            []HarnessTraceEvent `json:"trace,omitempty"`
	UpdatedAt        time.Time           `json:"updated_at"`
	LastErrorMessage string              `json:"last_error_message,omitempty"`
}

// CheckpointSummary is a lightweight view for listing runs.
type CheckpointSummary struct {
	RunID         string         `json:"run_id"`
	SessionID     core.SessionId `json:"session_id"`
	Task          string         `json:"task"`
	State         HarnessState   `json:"state"`
	StopReason    StopReason     `json:"stop_reason,omitempty"`
	Iteration     uint32         `json:"iteration"`
	ToolCallsMade uint32         `json:"tool_calls_made"`
	UpdatedAt     time.Time      `json:"updated_at"`
}

// Summary returns a lightweight summary of the checkpoint.
func (cp HarnessCheckpoint) Summary() CheckpointSummary {
	return CheckpointSummary{
		RunID:         cp.RunID,
		SessionID:     cp.SessionID,
		Task:          cp.Task,
		State:         cp.State,
		StopReason:    cp.StopReason,
		Iteration:     cp.Iteration,
		ToolCallsMade: cp.ToolCallsMade,
		UpdatedAt:     cp.UpdatedAt,
	}
}

// CheckpointStore persists and retrieves harness checkpoints.
type CheckpointStore interface {
	Save(ctx context.Context, cp HarnessCheckpoint) error
	Load(ctx context.Context, runID string) (HarnessCheckpoint, error)
	// Phase 14: Extended operations
	List(ctx context.Context, prefix string) ([]CheckpointSummary, error)
	Delete(ctx context.Context, runID string) error
}

// MemoryCheckpointStore stores checkpoints in memory for tests and short runs.
type MemoryCheckpointStore struct {
	mu          sync.RWMutex
	checkpoints map[string]HarnessCheckpoint
}

// NewMemoryCheckpointStore creates an empty in-memory checkpoint store.
func NewMemoryCheckpointStore() *MemoryCheckpointStore {
	return &MemoryCheckpointStore{checkpoints: make(map[string]HarnessCheckpoint)}
}

// Save stores a copy of the checkpoint.
func (s *MemoryCheckpointStore) Save(ctx context.Context, cp HarnessCheckpoint) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	cp.UpdatedAt = time.Now().UTC()
	s.checkpoints[cp.RunID] = cloneHarnessCheckpoint(cp)
	return nil
}

// Load retrieves a checkpoint by run ID.
func (s *MemoryCheckpointStore) Load(ctx context.Context, runID string) (HarnessCheckpoint, error) {
	if err := ctx.Err(); err != nil {
		return HarnessCheckpoint{}, err
	}
	s.mu.RLock()
	defer s.mu.RUnlock()
	cp, ok := s.checkpoints[runID]
	if !ok {
		return HarnessCheckpoint{}, fmt.Errorf("checkpoint %q not found", runID)
	}
	return cloneHarnessCheckpoint(cp), nil
}

// List returns summaries for all checkpoints matching the prefix.
func (s *MemoryCheckpointStore) List(ctx context.Context, prefix string) ([]CheckpointSummary, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	s.mu.RLock()
	defer s.mu.RUnlock()
	var summaries []CheckpointSummary
	for _, cp := range s.checkpoints {
		if prefix != "" && !hasPrefix(cp.RunID, prefix) {
			continue
		}
		summaries = append(summaries, cp.Summary())
	}
	return summaries, nil
}

// Delete removes a checkpoint by run ID.
func (s *MemoryCheckpointStore) Delete(ctx context.Context, runID string) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, ok := s.checkpoints[runID]; !ok {
		return fmt.Errorf("checkpoint %q not found", runID)
	}
	delete(s.checkpoints, runID)
	return nil
}

func cloneHarnessCheckpoint(cp HarnessCheckpoint) HarnessCheckpoint {
	cp.ContextBlocks = append([]ContextBlock(nil), cp.ContextBlocks...)
	cp.MemoryKeys = append([]string(nil), cp.MemoryKeys...)
	cp.RecentToolKeys = append([]string(nil), cp.RecentToolKeys...)
	cp.Trace = append([]HarnessTraceEvent(nil), cp.Trace...)
	return cp
}

// hasPrefix checks if s has the given prefix.
func hasPrefix(s, prefix string) bool {
	return len(s) >= len(prefix) && s[:len(prefix)] == prefix
}
