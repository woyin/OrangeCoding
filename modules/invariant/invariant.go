package invariant

import (
	"context"
	"fmt"
	"reflect"
	"sync"
)

// ---------------------------------------------------------------------------
// Invariant interface
// ---------------------------------------------------------------------------

// Invariant represents a runtime check that must hold true for the system to
// be in a valid state.
type Invariant interface {
	// Name returns a human-readable identifier for this invariant.
	Name() string
	// Check evaluates the invariant. A non-nil error means the invariant is
	// violated.
	Check(ctx context.Context) error
}

// ---------------------------------------------------------------------------
// Guard
// ---------------------------------------------------------------------------

// Guard runs a set of invariants and reports the first violation.
type Guard struct {
	invariants []Invariant
}

// NewGuard creates a Guard that will check the given invariants in order.
func NewGuard(invariants []Invariant) *Guard {
	return &Guard{invariants: invariants}
}

// Check runs all invariants sequentially and returns the first error
// encountered, formatted as "invariant {name} violated: {error}".
// Returns nil when all invariants pass.
func (g *Guard) Check(ctx context.Context) error {
	for _, inv := range g.invariants {
		if err := inv.Check(ctx); err != nil {
			return fmt.Errorf("invariant %s violated: %s", inv.Name(), err)
		}
	}
	return nil
}

// ---------------------------------------------------------------------------
// Engine (checkpoint / rollback)
// ---------------------------------------------------------------------------

// Engine stores named snapshots of state that can be restored later via
// rollback. It is safe for concurrent use.
type Engine struct {
	mu        sync.RWMutex
	snapshots map[string]interface{}
}

// NewEngine creates a new Engine with no snapshots.
func NewEngine() *Engine {
	return &Engine{
		snapshots: make(map[string]interface{}),
	}
}

// Checkpoint stores a deep copy of state under the given id. If a snapshot
// with the same id already exists it is overwritten. Reference types (maps,
// slices) are shallow-copied; value types are stored directly.
func (e *Engine) Checkpoint(id string, state interface{}) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.snapshots[id] = deepCopy(state)
}

// deepCopy creates an independent copy of v for supported reference types
// (maps and slices). For value types and unsupported kinds, it returns v
// directly since they are already copied by value when passed as interface{}.
func deepCopy(v interface{}) interface{} {
	if v == nil {
		return nil
	}
	rv := reflect.ValueOf(v)
	switch rv.Kind() {
	case reflect.Map:
		newMap := reflect.MakeMap(rv.Type())
		for _, key := range rv.MapKeys() {
			newMap.SetMapIndex(key, rv.MapIndex(key))
		}
		return newMap.Interface()
	case reflect.Slice:
		if rv.IsNil() {
			return v
		}
		newSlice := reflect.MakeSlice(rv.Type(), rv.Len(), rv.Cap())
		reflect.Copy(newSlice, rv)
		return newSlice.Interface()
	default:
		return v
	}
}

// Rollback retrieves the snapshot stored under id. Returns an error if no
// snapshot is found for the given id.
func (e *Engine) Rollback(id string) (interface{}, error) {
	e.mu.RLock()
	defer e.mu.RUnlock()

	state, ok := e.snapshots[id]
	if !ok {
		return nil, fmt.Errorf("checkpoint %q not found", id)
	}
	return state, nil
}

// ---------------------------------------------------------------------------
// SelfHealingPolicy
// ---------------------------------------------------------------------------

// SelfHealingPolicy retries a fix function up to a configured number of
// attempts.
type SelfHealingPolicy struct {
	maxAttempts int
	fix         func(ctx context.Context) error
}

// NewSelfHealingPolicy creates a policy that will call fix up to maxAttempts
// times. A non-positive maxAttempts is treated as 1.
func NewSelfHealingPolicy(maxAttempts int, fix func(ctx context.Context) error) *SelfHealingPolicy {
	if maxAttempts < 1 {
		maxAttempts = 1
	}
	return &SelfHealingPolicy{
		maxAttempts: maxAttempts,
		fix:         fix,
	}
}

// Execute runs the fix function repeatedly until it succeeds or the maximum
// number of attempts is exhausted. Returns nil on success, or the last error
// if all attempts fail.
func (p *SelfHealingPolicy) Execute(ctx context.Context) error {
	var lastErr error
	for i := 0; i < p.maxAttempts; i++ {
		lastErr = p.fix(ctx)
		if lastErr == nil {
			return nil
		}
	}
	return lastErr
}
