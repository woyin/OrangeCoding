/**
 * @module message-store
 *
 * Persistent message storage for the mesh bus.
 *
 * Stores messages published on the bus for:
 * - Late-joining subscribers (catch-up on missed messages)
 * - Audit trail and debugging
 * - Message replay after peer recovery
 *
 * Uses an append-only log with configurable retention.
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
  /**
   * topic -> 该 topic 下所有消息 id 的索引。
   * 性能优化：原 pending(topic) 遍历全部消息做 topic 过滤，是 O(总消息数)；
   * 建索引后 pending 只需扫描该 topic 的消息，大幅降低可靠总线重投递的开销。
   */
  private readonly _byTopic = new Map<string, MessageId[]>();

  async store(msg: MeshMessage): Promise<void> {
    this.messages.set(msg.id, msg);
    // 维护 topic 索引：把消息 id 追加到对应 topic 的列表
    let bucket = this._byTopic.get(msg.topic);
    if (bucket === undefined) {
      bucket = [];
      this._byTopic.set(msg.topic, bucket);
    }
    bucket.push(msg.id);
  }

  async pending(topic: string): Promise<MeshMessage[]> {
    // 只扫描该 topic 的消息 id（不再遍历全部消息）
    const bucket = this._byTopic.get(topic);
    if (bucket === undefined) return [];
    const result: MeshMessage[] = [];
    for (const id of bucket) {
      if (this.delivered.has(id)) continue;
      const msg = this.messages.get(id);
      if (msg) result.push(msg);
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
