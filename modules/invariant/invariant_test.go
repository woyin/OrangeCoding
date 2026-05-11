package invariant

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"testing"
)

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

type alwaysPass struct {
	name string
}

func (a *alwaysPass) Name() string                  { return a.name }
func (a *alwaysPass) Check(_ context.Context) error { return nil }

type alwaysFail struct {
	name string
}

func (a *alwaysFail) Name() string                  { return a.name }
func (a *alwaysFail) Check(_ context.Context) error { return errors.New("invariant violated") }

// ---------------------------------------------------------------------------
// Guard tests
// ---------------------------------------------------------------------------

func TestGuardPasses(t *testing.T) {
	g := NewGuard([]Invariant{
		&alwaysPass{name: "pass-1"},
		&alwaysPass{name: "pass-2"},
	})

	if err := g.Check(context.Background()); err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
}

func TestGuardFails(t *testing.T) {
	g := NewGuard([]Invariant{
		&alwaysPass{name: "pass-1"},
		&alwaysFail{name: "fail-1"},
		&alwaysPass{name: "pass-2"},
	})

	err := g.Check(context.Background())
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	want := "invariant fail-1 violated: invariant violated"
	if err.Error() != want {
		t.Fatalf("expected error message %q, got %q", want, err.Error())
	}
}

// ---------------------------------------------------------------------------
// Engine (checkpoint/rollback) tests
// ---------------------------------------------------------------------------

func TestCheckpointRollback(t *testing.T) {
	e := NewEngine()

	// Store a snapshot of a map.
	original := map[string]int{"a": 1, "b": 2}
	e.Checkpoint("snap-1", original)

	// Mutate the original map.
	original["c"] = 3

	// Rollback should return the snapshot (before mutation).
	got, err := e.Rollback("snap-1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// The snapshot is stored as interface{}, so we type-assert.
	snapshot, ok := got.(map[string]int)
	if !ok {
		t.Fatalf("expected map[string]int, got %T", got)
	}

	// The snapshot should have only the original keys.
	if len(snapshot) != 2 {
		t.Fatalf("expected 2 keys in snapshot, got %d", len(snapshot))
	}
	if snapshot["a"] != 1 || snapshot["b"] != 2 {
		t.Fatalf("unexpected snapshot content: %v", snapshot)
	}
	// Key "c" should NOT be present.
	if _, exists := snapshot["c"]; exists {
		t.Fatal("key 'c' should not exist in snapshot")
	}
}

func TestRollbackNotFound(t *testing.T) {
	e := NewEngine()

	_, err := e.Rollback("nonexistent")
	if err == nil {
		t.Fatal("expected error for unknown checkpoint id, got nil")
	}

	if !strings.Contains(err.Error(), "nonexistent") {
		t.Fatalf("error should mention the checkpoint id, got: %v", err)
	}
}

// ---------------------------------------------------------------------------
// SelfHealingPolicy tests
// ---------------------------------------------------------------------------

func TestSelfHealingPolicy(t *testing.T) {
	var attempts int

	p := NewSelfHealingPolicy(5, func(_ context.Context) error {
		attempts++
		if attempts < 3 {
			return fmt.Errorf("attempt %d failed", attempts)
		}
		return nil
	})

	if err := p.Execute(context.Background()); err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if attempts != 3 {
		t.Fatalf("expected 3 attempts, got %d", attempts)
	}
}

func TestSelfHealingPolicyExhausted(t *testing.T) {
	var attempts int

	p := NewSelfHealingPolicy(4, func(_ context.Context) error {
		attempts++
		return fmt.Errorf("attempt %d failed", attempts)
	})

	err := p.Execute(context.Background())
	if err == nil {
		t.Fatal("expected error when all attempts exhausted, got nil")
	}

	// Should have tried exactly maxAttempts times.
	if attempts != 4 {
		t.Fatalf("expected 4 attempts, got %d", attempts)
	}

	// The returned error should be from the last attempt.
	want := "attempt 4 failed"
	if err.Error() != want {
		t.Fatalf("expected error %q, got %q", want, err.Error())
	}
}
