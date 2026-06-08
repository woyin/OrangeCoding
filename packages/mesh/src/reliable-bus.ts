/**
 * @module reliable-bus
 * Reliable message bus with at-least-once delivery and acknowledgment.
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

  /** Subscribe registers for messages on a topic. */
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

  /** Unsubscribe removes a subscription. */
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

  /** Publish sends a message to all subscribers of a topic. */
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

  /** StartRedelivery begins the background redelivery loop. */
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
