/**
 * @module bus
 *
 * Message bus for inter-agent communication in the mesh network.
 *
 * The bus provides pub/sub messaging between agents:
 * - Topic-based message routing
 * - Multiple subscribers per topic
 * - Fire-and-forget delivery semantics
 * - Message serialization/deserialization
 *
 * The bus is the backbone of the mesh network, enabling agents
 * to communicate without direct coupling.
 */

import { v4 as uuidv4 } from "uuid";

// ---------------------------------------------------------------------------
// MessageHandler
// ---------------------------------------------------------------------------

/** Callback invoked when a message is published to a topic. */
export type MessageHandler = (topic: string, data: unknown) => void;

// ---------------------------------------------------------------------------
// Internal subscription
// ---------------------------------------------------------------------------

interface Subscription {
  id: string;
  handler: MessageHandler;
}

// ---------------------------------------------------------------------------
// MessageBus
// ---------------------------------------------------------------------------

/**
 * Simple pub/sub message bus. Subscribers register for a topic by name.
 * Publish dispatches each message to all subscribers of the matching topic
 * asynchronously so that handlers do not block each other.
 */
export class MessageBus {
  private topics = new Map<string, Subscription[]>();

  /**
   * Subscribe registers a handler for the given topic and returns a unique
   * subscription ID that can later be passed to unsubscribe.
   */
  subscribe(topic: string, handler: MessageHandler): string {
    const id = uuidv4();
    const subs = this.topics.get(topic) ?? [];
    subs.push({ id, handler });
    this.topics.set(topic, subs);
    return id;
  }

  /**
   * Unsubscribe removes the handler identified by (topic, id). If no such
   * subscription exists, unsubscribe is a no-op.
   */
  unsubscribe(topic: string, id: string): void {
    const subs = this.topics.get(topic);
    if (!subs) return;

    const idx = subs.findIndex((s) => s.id === id);
    if (idx === -1) return;

    subs.splice(idx, 1);
    if (subs.length === 0) {
      this.topics.delete(topic);
    }
  }

  /**
   * Publish sends data to every subscriber of the given topic. Each handler is
   * invoked asynchronously so that slow handlers do not block others.
   * Handler panics are caught and suppressed.
   */
  publish(topic: string, data: unknown): void {
    const subs = this.topics.get(topic);
    if (!subs) return;

    // Snapshot to avoid mutation during iteration.
    const snapshot = subs.slice();

    for (const sub of snapshot) {
      // Fire-and-forget async dispatch (mirrors goroutine behavior).
      Promise.resolve()
        .then(() => sub.handler(topic, data))
        .catch(() => {
          // Handler threw; suppress to keep bus alive.
        });
    }
  }
}
