package worker

import (
	"context"
	"fmt"
	"sync"
	"testing"
	"time"

	"github.com/woyin/OrangeCoding/modules/control-protocol"
)

func TestWorkerRuntimeStartStop(t *testing.T) {
	eventCh := make(chan controlprotocol.ServerEvent, 16)
	runtime := NewWorkerRuntime(eventCh)

	sessionID := "test-session-1"
	err := runtime.StartSession(sessionID, nil)
	if err != nil {
		t.Fatalf("StartSession() error = %v", err)
	}

	sessions := runtime.ListSessions()
	found := false
	for _, s := range sessions {
		if s == sessionID {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("ListSessions() = %v, want to contain %q", sessions, sessionID)
	}

	err = runtime.StopSession(sessionID)
	if err != nil {
		t.Fatalf("StopSession() error = %v", err)
	}

	// Give goroutine time to clean up
	time.Sleep(50 * time.Millisecond)

	sessions = runtime.ListSessions()
	for _, s := range sessions {
		if s == sessionID {
			t.Errorf("session %q still listed after stop", sessionID)
		}
	}
}

func TestWorkerRuntimeStopNonexistent(t *testing.T) {
	eventCh := make(chan controlprotocol.ServerEvent, 16)
	runtime := NewWorkerRuntime(eventCh)

	err := runtime.StopSession("nonexistent")
	if err == nil {
		t.Error("StopSession(nonexistent) should return error, got nil")
	}
}

func TestWorkerRuntimeDuplicateSession(t *testing.T) {
	eventCh := make(chan controlprotocol.ServerEvent, 16)
	runtime := NewWorkerRuntime(eventCh)

	sessionID := "dup-session"
	err := runtime.StartSession(sessionID, nil)
	if err != nil {
		t.Fatalf("first StartSession() error = %v", err)
	}

	err = runtime.StartSession(sessionID, nil)
	if err == nil {
		t.Error("duplicate StartSession() should return error, got nil")
	}

	// Cleanup
	runtime.StopSession(sessionID)
}

func TestWorkerRuntimeStatus(t *testing.T) {
	eventCh := make(chan controlprotocol.ServerEvent, 16)
	runtime := NewWorkerRuntime(eventCh)

	sessionID := "status-session"
	err := runtime.StartSession(sessionID, nil)
	if err != nil {
		t.Fatalf("StartSession() error = %v", err)
	}
	defer runtime.StopSession(sessionID)

	// With nil AgentLoop, Run should complete immediately with status "completed"
	time.Sleep(100 * time.Millisecond)

	status, ok := runtime.GetStatus(sessionID)
	if !ok {
		t.Fatal("GetStatus() returned ok=false, want true")
	}
	if status != "completed" {
		t.Errorf("GetStatus() = %q, want %q", status, "completed")
	}
}

func TestWorkerRuntimeStatusNonexistent(t *testing.T) {
	eventCh := make(chan controlprotocol.ServerEvent, 16)
	runtime := NewWorkerRuntime(eventCh)

	status, ok := runtime.GetStatus("nonexistent")
	if ok {
		t.Errorf("GetStatus(nonexistent) returned ok=true with status %q, want ok=false", status)
	}
}

func TestWorkerRuntimeListSessions(t *testing.T) {
	eventCh := make(chan controlprotocol.ServerEvent, 16)
	runtime := NewWorkerRuntime(eventCh)

	sessionIDs := []string{"session-a", "session-b"}
	for _, id := range sessionIDs {
		err := runtime.StartSession(id, nil)
		if err != nil {
			t.Fatalf("StartSession(%q) error = %v", id, err)
		}
	}

	sessions := runtime.ListSessions()
	if len(sessions) != 2 {
		t.Errorf("ListSessions() returned %d sessions, want 2", len(sessions))
	}

	set := make(map[string]bool)
	for _, s := range sessions {
		set[s] = true
	}
	for _, id := range sessionIDs {
		if !set[id] {
			t.Errorf("ListSessions() missing %q", id)
		}
	}

	// Cleanup
	for _, id := range sessionIDs {
		runtime.StopSession(id)
	}
}

func TestWorkerRuntimeConcurrentAccess(t *testing.T) {
	eventCh := make(chan controlprotocol.ServerEvent, 64)
	runtime := NewWorkerRuntime(eventCh)

	var wg sync.WaitGroup
	const numGoroutines = 10

	// Start sessions concurrently
	for i := 0; i < numGoroutines; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			id := fmt.Sprintf("concurrent-%d-%d", idx, time.Now().UnixNano())
			err := runtime.StartSession(id, nil)
			if err != nil {
				t.Errorf("StartSession(%q) error = %v", id, err)
			}
		}(i)
	}
	wg.Wait()

	sessions := runtime.ListSessions()
	if len(sessions) != numGoroutines {
		t.Errorf("ListSessions() returned %d sessions, want %d", len(sessions), numGoroutines)
	}

	// Stop all concurrently
	for _, id := range sessions {
		wg.Add(1)
		go func(sid string) {
			defer wg.Done()
			runtime.StopSession(sid)
		}(id)
	}
	wg.Wait()
}

func TestAgentExecutorNew(t *testing.T) {
	executor := NewAgentExecutor("test-exec", nil)
	if executor.SessionID() != "test-exec" {
		t.Errorf("SessionID() = %q, want %q", executor.SessionID(), "test-exec")
	}
	if executor.Status() != "pending" {
		t.Errorf("Status() = %q, want %q", executor.Status(), "pending")
	}
}

func TestAgentExecutorRunWithNilLoop(t *testing.T) {
	executor := NewAgentExecutor("test-exec", nil)
	ctx := context.Background()

	err := executor.Run(ctx)
	if err != nil {
		t.Fatalf("Run() with nil loop error = %v", err)
	}
	if executor.Status() != "completed" {
		t.Errorf("Status() after Run = %q, want %q", executor.Status(), "completed")
	}
}

func TestAgentExecutorRunCancellation(t *testing.T) {
	executor := NewAgentExecutor("cancel-exec", nil)
	ctx, cancel := context.WithCancel(context.Background())

	// Cancel before running
	cancel()
	err := executor.Run(ctx)
	if err == nil {
		t.Error("Run() with canceled context should return error")
	}
	if executor.Status() != "failed" {
		t.Errorf("Status() after canceled Run = %q, want %q", executor.Status(), "failed")
	}
}
