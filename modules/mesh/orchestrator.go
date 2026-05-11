package mesh

import (
	"context"
	"fmt"
	"sync"

	"github.com/google/uuid"
)

// TaskFunc is the signature for a task's executable function.
type TaskFunc func(ctx context.Context) error

// Task represents a unit of work with an identifier, dependency list, and
// executable function.
type Task struct {
	ID   string
	Deps []string
	Fn   TaskFunc
}

// TaskOrchestrator stores a set of tasks and can execute them respecting
// declared dependency ordering. Tasks that share no dependency chain run
// concurrently.
type TaskOrchestrator struct {
	mu    sync.RWMutex
	tasks map[string]Task
}

// NewTaskOrchestrator creates a new TaskOrchestrator.
func NewTaskOrchestrator() *TaskOrchestrator {
	return &TaskOrchestrator{
		tasks: make(map[string]Task),
	}
}

// AddTask registers a task with the given id, dependency list, and function.
// If a task with the same id already exists it is replaced.
func (o *TaskOrchestrator) AddTask(id string, deps []string, fn TaskFunc) {
	o.mu.Lock()
	defer o.mu.Unlock()
	o.tasks[id] = Task{ID: id, Deps: deps, Fn: fn}
}

// Run performs a topological sort of all registered tasks and executes them
// in dependency order. Tasks whose dependencies have all completed are
// dispatched concurrently. Run returns the first error encountered by any
// task, or nil if all tasks succeed.
//
// If the context is canceled, no new tasks are started and Run returns
// ctx.Err() as soon as possible.
func (o *TaskOrchestrator) Run(ctx context.Context) error {
	o.mu.RLock()
	tasks := make(map[string]Task, len(o.tasks))
	for k, v := range o.tasks {
		tasks[k] = v
	}
	o.mu.RUnlock()

	// Build in-degree map and adjacency list (reverse deps: dep -> dependents).
	inDegree := make(map[string]int)
	dependents := make(map[string][]string) // dep -> list of task IDs that depend on it

	for id, t := range tasks {
		if _, exists := inDegree[id]; !exists {
			inDegree[id] = 0
		}
		for _, dep := range t.Deps {
			inDegree[id]++
			dependents[dep] = append(dependents[dep], id)
		}
	}

	// Collect tasks with no dependencies (in-degree 0).
	var ready []string
	for id, deg := range inDegree {
		if deg == 0 {
			ready = append(ready, id)
		}
	}

	var (
		mu       sync.Mutex
		firstErr error
		wg       sync.WaitGroup
	)

	execOne := func(id string) {
		defer wg.Done()

		t := tasks[id]
		if err := t.Fn(ctx); err != nil {
			mu.Lock()
			if firstErr == nil {
				firstErr = fmt.Errorf("task %s failed: %w", id, err)
			}
			mu.Unlock()
			return
		}

		// Decrement in-degree of dependents and enqueue newly ready tasks.
		mu.Lock()
		for _, depID := range dependents[id] {
			inDegree[depID]--
			if inDegree[depID] == 0 {
				ready = append(ready, depID)
			}
		}
		mu.Unlock()
	}

	// Kahn's algorithm with concurrent execution.
	for {
		mu.Lock()
		batch := make([]string, len(ready))
		copy(batch, ready)
		ready = ready[:0]
		mu.Unlock()

		if len(batch) == 0 {
			break
		}

		for _, id := range batch {
			// Check for context cancellation before dispatching.
			if ctx.Err() != nil {
				return ctx.Err()
			}

			mu.Lock()
			err := firstErr
			mu.Unlock()
			if err != nil {
				break
			}

			wg.Add(1)
			go execOne(id)
		}

		wg.Wait()

		mu.Lock()
		err := firstErr
		mu.Unlock()
		if err != nil {
			return err
		}
	}

	// Check for cycles — if there are still tasks with non-zero in-degree,
	// they form a cycle.
	for id, deg := range inDegree {
		if deg > 0 {
			return fmt.Errorf("dependency cycle detected involving task %q", id)
		}
	}

	return nil
}

// newUUID generates a UUID string for internal use.
func newUUID() string {
	return uuid.New().String()
}
