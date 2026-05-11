package mesh

import (
	"context"
	"fmt"

	"github.com/woyin/OrangeCoding/modules/core"
)

// HandoffMessage is the data payload published when one agent hands off a
// task to another agent.
type HandoffMessage struct {
	From core.AgentId
	To   core.AgentId
	Task string
}

// Negotiator coordinates task handoffs between agents. It validates that both
// the source and target agents exist in the registry before publishing the
// handoff message on the bus.
type Negotiator struct {
	registry *AgentRegistry
	bus      *MessageBus
}

// NewNegotiator creates a Negotiator wired to the given registry and bus.
func NewNegotiator(registry *AgentRegistry, bus *MessageBus) *Negotiator {
	return &Negotiator{
		registry: registry,
		bus:      bus,
	}
}

// Handoff publishes a task handoff message from one agent to another. Both
// agents must be registered in the registry; otherwise an error is returned.
func (n *Negotiator) Handoff(ctx context.Context, fromID, toID core.AgentId, task string) error {
	if ctx.Err() != nil {
		return ctx.Err()
	}

	if _, ok := n.registry.Get(fromID); !ok {
		return fmt.Errorf("handoff: source agent %s not registered", fromID)
	}
	if _, ok := n.registry.Get(toID); !ok {
		return fmt.Errorf("handoff: target agent %s not registered", toID)
	}

	n.bus.Publish("agent.handoff", HandoffMessage{
		From: fromID,
		To:   toID,
		Task: task,
	})
	return nil
}

// BuddyObserver watches for events on the bus and invokes registered handlers.
type BuddyObserver struct {
	bus      *MessageBus
	handlers map[string]func(data interface{})
}

// NewBuddyObserver creates a BuddyObserver that listens on the given bus.
func NewBuddyObserver(bus *MessageBus) *BuddyObserver {
	return &BuddyObserver{
		bus:      bus,
		handlers: make(map[string]func(data interface{})),
	}
}

// Watch subscribes to events of the given type on the bus and calls handler
// whenever a message arrives on that topic.
func (o *BuddyObserver) Watch(eventType string, handler func(data interface{})) {
	o.handlers[eventType] = handler
	o.bus.Subscribe(eventType, func(topic string, data interface{}) {
		handler(data)
	})
}
