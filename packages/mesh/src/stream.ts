/**
 * @module stream
 * Real-time event streaming for task progress, artifacts, and logs.
 */

// ---------------------------------------------------------------------------
// StreamEventType
// ---------------------------------------------------------------------------

/** Classifies the kind of stream event. */
export enum StreamEventType {
  Progress = "progress",
  Artifact = "artifact",
  Log = "log",
}

// ---------------------------------------------------------------------------
// StreamEvent
// ---------------------------------------------------------------------------

/** Carries real-time updates from an agent. */
export interface StreamEvent {
  taskId: string;
  type: StreamEventType;
  percent: number;
  message: string;
  content?: Uint8Array;
  level: string;
}

// ---------------------------------------------------------------------------
// Stream
// ---------------------------------------------------------------------------

/** Provides pub/sub for task events using an EventEmitter pattern. */
export class Stream {
  private id: string;
  private subscribers = new Map<string, (event: StreamEvent) => void>();
  private counter = 0;

  constructor(taskID: string) {
    this.id = taskID;
  }

  /**
   * Subscribe registers for events on this stream.
   * @param taskID - Must match the stream's task ID.
   * @param handler - Callback invoked for each event.
   * @returns The subscription ID.
   */
  subscribe(taskID: string, handler: (event: StreamEvent) => void): string {
    if (taskID !== this.id) {
      throw new Error(`stream task ID mismatch: ${taskID} != ${this.id}`);
    }

    const subId = `${taskID}-${this.counter++}`;
    this.subscribers.set(subId, handler);
    return subId;
  }

  /**
   * Publish sends an event to all subscribers.
   * @throws Error if event task ID doesn't match stream task ID.
   */
  publish(event: StreamEvent): void {
    if (event.taskId !== this.id) {
      throw new Error(`event task ID mismatch: ${event.taskId} != ${this.id}`);
    }

    for (const handler of this.subscribers.values()) {
      // Non-blocking dispatch (mirrors Go's non-blocking channel send with default case).
      try {
        handler(event);
      } catch {
        // Suppress handler errors (mirrors Go's channel default case).
      }
    }
  }

  /** Close shuts down the stream and removes all subscribers. */
  close(): void {
    this.subscribers.clear();
  }
}
