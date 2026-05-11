package mesh

import (
	"context"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// ---------------------------------------------------------------------------
// MessageBus tests
// ---------------------------------------------------------------------------

func TestMessageBus(t *testing.T) {
	bus := NewMessageBus()

	var receivedTopic string
	var receivedData interface{}
	var mu sync.Mutex
	var called int32

	bus.Subscribe("test.topic", func(topic string, data interface{}) {
		mu.Lock()
		receivedTopic = topic
		receivedData = data
		atomic.AddInt32(&called, 1)
		mu.Unlock()
	})

	bus.Publish("test.topic", "hello")

	// Wait for async dispatch.
	deadline := time.After(2 * time.Second)
	for atomic.LoadInt32(&called) == 0 {
		select {
		case <-deadline:
			t.Fatal("timed out waiting for handler to be called")
		default:
		}
	}

	mu.Lock()
	defer mu.Unlock()
	if receivedTopic != "test.topic" {
		t.Errorf("expected topic %q, got %q", "test.topic", receivedTopic)
	}
	if receivedData != "hello" {
		t.Errorf("expected data %v, got %v", "hello", receivedData)
	}
}

func TestMessageBusMultipleSubscribers(t *testing.T) {
	bus := NewMessageBus()

	var called1, called2 int32

	bus.Subscribe("multi", func(topic string, data interface{}) {
		atomic.AddInt32(&called1, 1)
	})
	bus.Subscribe("multi", func(topic string, data interface{}) {
		atomic.AddInt32(&called2, 1)
	})

	bus.Publish("multi", 42)

	deadline := time.After(2 * time.Second)
	for atomic.LoadInt32(&called1) == 0 || atomic.LoadInt32(&called2) == 0 {
		select {
		case <-deadline:
			t.Fatalf("timed out: called1=%d called2=%d", called1, called2)
		default:
		}
	}

	if atomic.LoadInt32(&called1) != 1 {
		t.Errorf("expected handler1 called 1 time, got %d", called1)
	}
	if atomic.LoadInt32(&called2) != 1 {
		t.Errorf("expected handler2 called 1 time, got %d", called2)
	}
}

func TestMessageBusUnsubscribe(t *testing.T) {
	bus := NewMessageBus()

	var called int32

	id := bus.Subscribe("unsub", func(topic string, data interface{}) {
		atomic.AddInt32(&called, 1)
	})

	bus.Unsubscribe("unsub", id)
	bus.Publish("unsub", "data")

	// Give a brief window for any erroneous dispatch.
	time.Sleep(100 * time.Millisecond)

	if atomic.LoadInt32(&called) != 0 {
		t.Errorf("expected handler not to be called after unsubscribe, got %d calls", called)
	}
}

// ---------------------------------------------------------------------------
// TaskOrchestrator tests
// ---------------------------------------------------------------------------

func TestTaskOrchestrator(t *testing.T) {
	o := NewTaskOrchestrator()

	var order []string
	var mu sync.Mutex

	record := func(name string) TaskFunc {
		return func(ctx context.Context) error {
			mu.Lock()
			order = append(order, name)
			mu.Unlock()
			return nil
		}
	}

	// a -> b -> c  (a must finish before b, b before c)
	o.AddTask("a", []string{}, record("a"))
	o.AddTask("b", []string{"a"}, record("b"))
	o.AddTask("c", []string{"b"}, record("c"))

	if err := o.Run(context.Background()); err != nil {
		t.Fatalf("Run returned error: %v", err)
	}

	mu.Lock()
	defer mu.Unlock()
	if len(order) != 3 {
		t.Fatalf("expected 3 tasks executed, got %d: %v", len(order), order)
	}
	// Verify order: a before b, b before c.
	if order[0] != "a" || order[1] != "b" || order[2] != "c" {
		t.Errorf("expected order [a b c], got %v", order)
	}
}

func TestTaskOrchestratorParallel(t *testing.T) {
	o := NewTaskOrchestrator()

	var started sync.WaitGroup
	var mu sync.Mutex
	var concurrency int
	var maxConcurrency int

	parallelTask := func(name string) TaskFunc {
		return func(ctx context.Context) error {
			started.Done()
			mu.Lock()
			concurrency++
			if concurrency > maxConcurrency {
				maxConcurrency = concurrency
			}
			mu.Unlock()

			// Hold long enough for other tasks to start concurrently.
			time.Sleep(100 * time.Millisecond)

			mu.Lock()
			concurrency--
			mu.Unlock()
			return nil
		}
	}

	// Three independent tasks with no deps — they should run concurrently.
	o.AddTask("x", []string{}, parallelTask("x"))
	o.AddTask("y", []string{}, parallelTask("y"))
	o.AddTask("z", []string{}, parallelTask("z"))

	started.Add(3)
	if err := o.Run(context.Background()); err != nil {
		t.Fatalf("Run returned error: %v", err)
	}

	mu.Lock()
	defer mu.Unlock()
	if maxConcurrency < 2 {
		t.Errorf("expected at least 2 tasks running concurrently, max was %d", maxConcurrency)
	}
}

// ---------------------------------------------------------------------------
// AgentRegistry tests
// ---------------------------------------------------------------------------

func TestAgentRegistry(t *testing.T) {
	r := NewAgentRegistry()

	id1 := core.NewAgentId()
	id2 := core.NewAgentId()
	id3 := core.NewAgentId()

	cap1 := core.AgentCapability{Name: "coding", Description: "code generation"}
	cap2 := core.AgentCapability{Name: "review", Description: "code review"}
	cap3 := core.AgentCapability{Name: "coding", Description: "code generation v2"}

	r.Register(AgentInfo{
		ID:           id1,
		Role:         core.RoleCoder,
		Capabilities: []core.AgentCapability{cap1},
		Status:       core.StatusIdle,
	})
	r.Register(AgentInfo{
		ID:           id2,
		Role:         core.RoleReviewer,
		Capabilities: []core.AgentCapability{cap2},
		Status:       core.StatusRunning,
	})
	r.Register(AgentInfo{
		ID:           id3,
		Role:         core.RoleCoder,
		Capabilities: []core.AgentCapability{cap3},
		Status:       core.StatusIdle,
	})

	// Get existing agent.
	info, ok := r.Get(id1)
	if !ok {
		t.Fatal("expected to find id1")
	}
	if info.Role != core.RoleCoder {
		t.Errorf("expected RoleCoder, got %v", info.Role)
	}

	// Get non-existing agent.
	_, ok = r.Get(core.NewAgentId())
	if ok {
		t.Error("expected not to find random agent")
	}

	// FindByRole.
	coders := r.FindByRole(core.RoleCoder)
	if len(coders) != 2 {
		t.Errorf("expected 2 coders, got %d", len(coders))
	}

	reviewers := r.FindByRole(core.RoleReviewer)
	if len(reviewers) != 1 {
		t.Errorf("expected 1 reviewer, got %d", len(reviewers))
	}

	// FindByCapability.
	codingAgents := r.FindByCapability("coding")
	if len(codingAgents) != 2 {
		t.Errorf("expected 2 coding agents, got %d", len(codingAgents))
	}

	reviewAgents := r.FindByCapability("review")
	if len(reviewAgents) != 1 {
		t.Errorf("expected 1 review agent, got %d", len(reviewAgents))
	}

	unknownAgents := r.FindByCapability("unknown")
	if len(unknownAgents) != 0 {
		t.Errorf("expected 0 unknown agents, got %d", len(unknownAgents))
	}

	// Unregister.
	r.Unregister(id2)
	_, ok = r.Get(id2)
	if ok {
		t.Error("expected id2 to be gone after unregister")
	}
}

// ---------------------------------------------------------------------------
// Negotiator + BuddyObserver tests
// ---------------------------------------------------------------------------

func TestNegotiatorHandoff(t *testing.T) {
	bus := NewMessageBus()
	registry := NewAgentRegistry()
	n := NewNegotiator(registry, bus)

	from := core.NewAgentId()
	to := core.NewAgentId()

	// Both agents must be registered for Handoff to succeed.
	registry.Register(AgentInfo{ID: from, Role: core.RoleCoder, Status: core.StatusIdle})
	registry.Register(AgentInfo{ID: to, Role: core.RoleExecutor, Status: core.StatusIdle})

	var receivedTopic string
	var receivedData interface{}
	var mu sync.Mutex
	var called int32

	bus.Subscribe("agent.handoff", func(topic string, data interface{}) {
		mu.Lock()
		receivedTopic = topic
		receivedData = data
		atomic.AddInt32(&called, 1)
		mu.Unlock()
	})

	err := n.Handoff(context.Background(), from, to, "implement feature X")
	if err != nil {
		t.Fatalf("Handoff returned error: %v", err)
	}

	deadline := time.After(2 * time.Second)
	for atomic.LoadInt32(&called) == 0 {
		select {
		case <-deadline:
			t.Fatal("timed out waiting for handoff message")
		default:
		}
	}

	mu.Lock()
	defer mu.Unlock()
	if receivedTopic != "agent.handoff" {
		t.Errorf("expected topic %q, got %q", "agent.handoff", receivedTopic)
	}
	handoff, ok := receivedData.(HandoffMessage)
	if !ok {
		t.Fatalf("expected HandoffMessage, got %T", receivedData)
	}
	if handoff.From != from {
		t.Error("From field mismatch")
	}
	if handoff.To != to {
		t.Error("To field mismatch")
	}
	if handoff.Task != "implement feature X" {
		t.Errorf("expected task %q, got %q", "implement feature X", handoff.Task)
	}
}

func TestBuddyObserver(t *testing.T) {
	bus := NewMessageBus()
	obs := NewBuddyObserver(bus)

	var received interface{}
	var mu sync.Mutex
	var called int32

	obs.Watch("error", func(data interface{}) {
		mu.Lock()
		received = data
		atomic.AddInt32(&called, 1)
		mu.Unlock()
	})

	bus.Publish("error", "something went wrong")

	deadline := time.After(2 * time.Second)
	for atomic.LoadInt32(&called) == 0 {
		select {
		case <-deadline:
			t.Fatal("timed out waiting for observer handler")
		default:
		}
	}

	mu.Lock()
	defer mu.Unlock()
	if received != "something went wrong" {
		t.Errorf("expected %v, got %v", "something went wrong", received)
	}
}
