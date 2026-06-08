/**
 * @module message-store
 * Message persistence for reliable delivery.
 */

// ---------------------------------------------------------------------------
// Message / MessageId (re-exported types shared with reliable-bus)
// ---------------------------------------------------------------------------

/** Uniquely identifies a message. */
export type MessageId = string;

/** A unit of data published to a topic. */
export interface MeshMessage {
  id: MessageId;
  topic: string;
  payload: unknown;
  timestamp: Date;
}

// ---------------------------------------------------------------------------
// MessageStore
// ---------------------------------------------------------------------------

/** Persists messages for reliable delivery. */
export interface MessageStore {
  store(msg: MeshMessage): Promise<void>;
  pending(topic: string): Promise<MeshMessage[]>;
  markDelivered(id: MessageId): Promise<void>;
  deadLetters(): Promise<MeshMessage[]>;
  close(): Promise<void>;
}

// ---------------------------------------------------------------------------
// InMemoryMessageStore
// ---------------------------------------------------------------------------

/** Non-persistent MessageStore for testing. */
export class InMemoryMessageStore implements MessageStore {
  private messages = new Map<MessageId, MeshMessage>();
  private delivered = new Set<MessageId>();
  private deadLetterList: MeshMessage[] = [];

  async store(msg: MeshMessage): Promise<void> {
    this.messages.set(msg.id, msg);
  }

  async pending(topic: string): Promise<MeshMessage[]> {
    const result: MeshMessage[] = [];
    for (const [id, msg] of this.messages) {
      if (msg.topic === topic && !this.delivered.has(id)) {
        result.push(msg);
      }
    }
    return result;
  }

  async markDelivered(id: MessageId): Promise<void> {
    if (!this.messages.has(id)) {
      throw new Error(`message ${id} not found`);
    }
    this.delivered.add(id);
  }

  async deadLetters(): Promise<MeshMessage[]> {
    return [...this.deadLetterList];
  }

  async close(): Promise<void> {
    // No-op.
  }
}
