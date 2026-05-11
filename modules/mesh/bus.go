package mesh

import (
	"sync"

	"github.com/google/uuid"
)

// MessageHandler is a callback invoked when a message is published to a topic.
type MessageHandler func(topic string, data interface{})

// subscription pairs a handler with its unique subscription ID.
type subscription struct {
	id      string
	handler MessageHandler
}

// MessageBus provides a simple pub/sub message bus. Subscribers register for
// a topic by name. Publish dispatches each message to all subscribers of the
// matching topic via goroutines so that handlers do not block each other.
type MessageBus struct {
	mu     sync.RWMutex
	topics map[string][]subscription
}

// NewMessageBus creates a new MessageBus ready for use.
func NewMessageBus() *MessageBus {
	return &MessageBus{
		topics: make(map[string][]subscription),
	}
}

// Subscribe registers a handler for the given topic and returns a unique
// subscription ID that can later be passed to Unsubscribe.
func (b *MessageBus) Subscribe(topic string, handler MessageHandler) string {
	id := uuid.New().String()
	b.mu.Lock()
	b.topics[topic] = append(b.topics[topic], subscription{id: id, handler: handler})
	b.mu.Unlock()
	return id
}

// Unsubscribe removes the handler identified by (topic, id). If no such
// subscription exists, Unsubscribe is a no-op.
func (b *MessageBus) Unsubscribe(topic string, id string) {
	b.mu.Lock()
	defer b.mu.Unlock()

	subs := b.topics[topic]
	for i, s := range subs {
		if s.id == id {
			b.topics[topic] = append(subs[:i], subs[i+1:]...)
			if len(b.topics[topic]) == 0 {
				delete(b.topics, topic)
			}
			return
		}
	}
}

// Publish sends data to every subscriber of the given topic. Each handler is
// invoked in its own goroutine so that slow handlers do not block others.
func (b *MessageBus) Publish(topic string, data interface{}) {
	b.mu.RLock()
	subs := make([]subscription, len(b.topics[topic]))
	copy(subs, b.topics[topic])
	b.mu.RUnlock()

	for _, s := range subs {
		go s.handler(topic, data)
	}
}
