package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// TraceSchemaVersion is the version of the trace event schema.
const TraceSchemaVersion = "1.0"

// TraceEvent is a structured observation recorded during a harness run.
// It is stored independently from checkpoints for query and export.
type TraceEvent struct {
	SchemaVersion string            `json:"schema_version"`
	RunID         string            `json:"run_id"`
	SessionID     core.SessionId    `json:"session_id"`
	FromState     HarnessState      `json:"from_state"`
	ToState       HarnessState      `json:"to_state"`
	Reason        string            `json:"reason,omitempty"`
	Metadata      map[string]string `json:"metadata,omitempty"`
	CreatedAt     time.Time         `json:"created_at"`
}

// TraceQuery filters trace events for querying.
type TraceQuery struct {
	RunID     string
	SessionID core.SessionId
	FromState HarnessState
	ToState   HarnessState
	StartTime time.Time
	EndTime   time.Time
	Limit     int
}

// TraceStore persists and queries trace events independently from checkpoints.
type TraceStore interface {
	Append(ctx context.Context, event TraceEvent) error
	Query(ctx context.Context, q TraceQuery) ([]TraceEvent, error)
}

// MemoryTraceStore stores trace events in memory.
type MemoryTraceStore struct {
	mu     sync.RWMutex
	events []TraceEvent
}

// NewMemoryTraceStore creates an empty in-memory trace store.
func NewMemoryTraceStore() *MemoryTraceStore {
	return &MemoryTraceStore{}
}

// Append adds a trace event.
func (s *MemoryTraceStore) Append(ctx context.Context, event TraceEvent) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	s.events = append(s.events, event)
	return nil
}

// Query returns trace events matching the filter.
func (s *MemoryTraceStore) Query(ctx context.Context, q TraceQuery) ([]TraceEvent, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	s.mu.RLock()
	defer s.mu.RUnlock()
	var result []TraceEvent
	for _, e := range s.events {
		if !matchesTraceQuery(e, q) {
			continue
		}
		result = append(result, e)
	}
	// Sort by CreatedAt descending
	sort.Slice(result, func(i, j int) bool {
		return result[i].CreatedAt.After(result[j].CreatedAt)
	})
	if q.Limit > 0 && len(result) > q.Limit {
		result = result[:q.Limit]
	}
	return result, nil
}

// FileTraceStore persists trace events as JSON lines files per run.
type FileTraceStore struct {
	dir string
}

// NewFileTraceStore creates a file-backed trace store.
func NewFileTraceStore(dir string) *FileTraceStore {
	return &FileTraceStore{dir: dir}
}

// Append adds a trace event to the run's trace file.
func (s *FileTraceStore) Append(ctx context.Context, event TraceEvent) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if event.RunID == "" {
		return fmt.Errorf("trace store: run id is required")
	}
	if err := os.MkdirAll(s.dir, 0o755); err != nil {
		return fmt.Errorf("trace store: mkdir: %w", err)
	}
	if event.SchemaVersion == "" {
		event.SchemaVersion = TraceSchemaVersion
	}
	data, err := json.Marshal(event)
	if err != nil {
		return fmt.Errorf("trace store: marshal: %w", err)
	}
	f, err := os.OpenFile(s.pathFor(event.RunID), os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
	if err != nil {
		return fmt.Errorf("trace store: open: %w", err)
	}
	defer f.Close()
	if _, err := fmt.Fprintf(f, "%s\n", data); err != nil {
		return fmt.Errorf("trace store: write: %w", err)
	}
	return nil
}

// Query returns trace events matching the filter from file storage.
func (s *FileTraceStore) Query(ctx context.Context, q TraceQuery) ([]TraceEvent, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	var runIDs []string
	if q.RunID != "" {
		runIDs = []string{q.RunID}
	} else {
		entries, err := os.ReadDir(s.dir)
		if err != nil {
			if os.IsNotExist(err) {
				return nil, nil
			}
			return nil, fmt.Errorf("trace store: read dir: %w", err)
		}
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".ndjson") {
				runIDs = append(runIDs, strings.TrimSuffix(e.Name(), ".ndjson"))
			}
		}
	}

	var result []TraceEvent
	for _, runID := range runIDs {
		events, err := s.loadRunTrace(runID)
		if err != nil {
			continue
		}
		for _, e := range events {
			if matchesTraceQuery(e, q) {
				result = append(result, e)
			}
		}
	}

	sort.Slice(result, func(i, j int) bool {
		return result[i].CreatedAt.After(result[j].CreatedAt)
	})
	if q.Limit > 0 && len(result) > q.Limit {
		result = result[:q.Limit]
	}
	return result, nil
}

func (s *FileTraceStore) loadRunTrace(runID string) ([]TraceEvent, error) {
	data, err := os.ReadFile(s.pathFor(runID))
	if err != nil {
		return nil, err
	}
	var events []TraceEvent
	for _, line := range strings.Split(string(data), "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var e TraceEvent
		if err := json.Unmarshal([]byte(line), &e); err != nil {
			continue
		}
		events = append(events, e)
	}
	return events, nil
}

func (s *FileTraceStore) pathFor(runID string) string {
	return filepath.Join(s.dir, runID+".ndjson")
}

// matchesTraceQuery checks if an event matches the query filter.
func matchesTraceQuery(e TraceEvent, q TraceQuery) bool {
	if q.RunID != "" && e.RunID != q.RunID {
		return false
	}
	if q.SessionID != (core.SessionId{}) && e.SessionID != q.SessionID {
		return false
	}
	if q.FromState != "" && e.FromState != q.FromState {
		return false
	}
	if q.ToState != "" && e.ToState != q.ToState {
		return false
	}
	if !q.StartTime.IsZero() && e.CreatedAt.Before(q.StartTime) {
		return false
	}
	if !q.EndTime.IsZero() && e.CreatedAt.After(q.EndTime) {
		return false
	}
	return true
}

// TraceToSpans converts trace events to a simple OTLP-compatible span representation.
// This is a minimal adapter; production use would use go.opentelemetry.io/otel.
type OTLPSpan struct {
	TraceID   string            `json:"trace_id"`
	SpanID    string            `json:"span_id"`
	Name      string            `json:"name"`
	StartTime time.Time         `json:"start_time"`
	EndTime   time.Time         `json:"end_time"`
	Attrs     map[string]string `json:"attributes,omitempty"`
}

// TraceEventsToSpans converts trace events to OTLP spans for export.
func TraceEventsToSpans(events []TraceEvent) []OTLPSpan {
	spans := make([]OTLPSpan, len(events))
	for i, e := range events {
		spans[i] = OTLPSpan{
			TraceID:   e.RunID,
			SpanID:    fmt.Sprintf("%s-%d", e.RunID, i),
			Name:      string(e.FromState) + " -> " + string(e.ToState),
			StartTime: e.CreatedAt,
			EndTime:   e.CreatedAt,
			Attrs: map[string]string{
				"run_id":     e.RunID,
				"from_state": string(e.FromState),
				"to_state":   string(e.ToState),
				"reason":     e.Reason,
				"schema":     e.SchemaVersion,
			},
		}
	}
	return spans
}
