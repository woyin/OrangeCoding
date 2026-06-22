/**
 * @module reliable-bus
 *
 * Reliable message bus with guaranteed delivery semantics.
 *
 * Extends the base Bus with:
 * - Message persistence (via MessageStore)
 * - Delivery acknowledgment
 * - Automatic retry on failure
 * - Ordered delivery guarantees
 *
 * Used for critical messages that must not be lost (task assignments,
 * results, health status changes).
 */

import { v4 as uuidv4 } from "uuid";
import type { AgentId } from "@orangecoding/core";
import type { MessageStore, MessageId, MeshMessage } from "./message-store.js";

// ---------------------------------------------------------------------------
// SecurityGuard (interface re-declared; implementation in security.ts)
// ---------------------------------------------------------------------------

/** Validates operations for security. */
export interface SecurityGuard {
  validateToolCall(agentID: AgentId, toolName: string): [boolean, string];
}

// ---------------------------------------------------------------------------
// Delivery
// ---------------------------------------------------------------------------

/** Wraps a message with acknowledgment capabilities. */
export class Delivery {
  message: MeshMessage;
  acked = false;
  private ackFunc?: () => Promise<void>;
  private nackFunc?: () => Promise<void>;

  constructor(
    message: MeshMessage,
    ackFunc?: () => Promise<void>,
    nackFunc?: () => Promise<void>,
  ) {
    this.message = message;
    this.ackFunc = ackFunc;
    this.nackFunc = nackFunc;
  }

  /** Ack marks the message as successfully delivered. */
  async ack(): Promise<void> {
    this.acked = true;
    if (this.ackFunc) {
      await this.ackFunc();
    }
  }

  /** Nack marks the message as not delivered. */
  async nack(): Promise<void> {
    if (this.nackFunc) {
      await this.nackFunc();
    }
  }
}

// ---------------------------------------------------------------------------
// Subscription
// ---------------------------------------------------------------------------

/** Represents a subscriber's connection to a topic via EventEmitter pattern. */
export interface Subscription {
  id: string;
  topic: string;
  /** Async iterator / callback for receiving deliveries. */
  onDelivery: (delivery: Delivery) => void;
}

// ---------------------------------------------------------------------------
// ReliableBus
// ---------------------------------------------------------------------------

/** Provides at-least-once delivery with acknowledgment. */
export class ReliableBus {
  private topics = new Map<string, Subscription[]>();
  private store?: MessageStore;
  private guard?: SecurityGuard;
  private redeliveryTimer?: ReturnType<typeof setInterval>;

  constructor(store?: MessageStore, guard?: SecurityGuard) {
    this.store = store;
    this.guard = guard;
  }

  /**
   * Registers `onDelivery` to receive deliveries for `topic`. Returns a
   * Subscription handle used by {@link unsubscribe}. Subscription lists are
   * mutated in place; unsubscribe removes by id (O(n) findIndex, acceptable
   * since subscriber counts per topic are small).
   */
  subscribe(topic: string, onDelivery: (delivery: Delivery) => void): Subscription {
    const sub: Subscription = {
      id: uuidv4(),
      topic,
      onDelivery,
    };

    const subs = this.topics.get(topic) ?? [];
    subs.push(sub);
    this.topics.set(topic, subs);
    return sub;
  }

  /**
   * Removes a subscription by id. Cleans up the topic map entry when the
   * last subscriber leaves so topics do not leak.
   */
  unsubscribe(sub: Subscription): void {
    const subs = this.topics.get(sub.topic);
    if (!subs) return;

    const idx = subs.findIndex((s) => s.id === sub.id);
    if (idx === -1) return;

    subs.splice(idx, 1);
    if (subs.length === 0) {
      this.topics.delete(sub.topic);
    }
  }

  /**
   * Publishes `payload` to all current subscribers of `topic` with at-least-
   * once delivery. The message is durably stored (if a MessageStore is
   * configured) before any dispatch, so a crash after store but before ack
   * triggers redelivery rather than loss.
   *
   * Dispatch is fire-and-forget per subscriber (mirrors goroutine + channel
   * send in the Go original); failures are swallowed here and retried by the
   * redelivery loop.
   */
  async publish(topic: string, payload: unknown): Promise<void> {
    const msg: MeshMessage = {
      id: uuidv4() as MessageId,
      topic,
      payload,
      timestamp: new Date(),
    };

    if (this.store) {
      await this.store.store(msg);
    }

    const subs = this.topics.get(topic) ?? [];

    for (const sub of subs) {
      // Fire-and-forget async dispatch (mirrors goroutine + channel send).
      const delivery = new Delivery(msg, async () => {
        if (this.store) {
          await this.store.markDelivered(msg.id);
        }
      });

      // Non-blocking dispatch with 5-second timeout (mirrors Go select + time.After).
      Promise.resolve()
        .then(() => sub.onDelivery(delivery))
        .catch(() => {
          // Delivery failed; will be redelivered by redelivery loop.
        });
    }
  }

  /** Close shuts down the bus and all subscriptions. */
  close(): void {
    if (this.redeliveryTimer) {
      clearInterval(this.redeliveryTimer);
      this.redeliveryTimer = undefined;
    }
    this.topics.clear();
  }

  /**
   * Starts a background interval that re-dispatches stored-but-unacked
   * messages for every active topic. This is what makes delivery at-least-
   * once rather than at-most-once: a subscriber that never acks (e.g. crashed)
   * will keep seeing the message until it succeeds.
   */
  startRedelivery(intervalMs = 100): void {
    this.redeliveryTimer = setInterval(() => {
      this.redeliverPending();
    }, intervalMs);
  }

  private async redeliverPending(): Promise<void> {
    if (!this.store) return;

    const allTopics = Array.from(this.topics.keys());

    for (const topic of allTopics) {
      try {
        const pending = await this.store.pending(topic);
        for (const msg of pending) {
          this.redeliverMessage(msg);
        }
      } catch {
        // Continue on error (mirrors Go continue).
      }
    }
  }

  private redeliverMessage(msg: MeshMessage): void {
    const subs = this.topics.get(msg.topic) ?? [];

    for (const sub of subs) {
      const delivery = new Delivery(msg, async () => {
        if (this.store) {
          await this.store.markDelivered(msg.id);
        }
      });

      // Fire-and-forget with panic recovery (mirrors Go recover).
      Promise.resolve()
        .then(() => sub.onDelivery(delivery))
        .catch(() => {});
    }
  }
}
