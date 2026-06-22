/**
 * Tests for the mesh streaming module — event pub/sub and stream management.
 */

import { Stream, StreamEventType } from "../stream.js";
import type { StreamEvent } from "../stream.js";

function makeEvent(overrides: Partial<StreamEvent> = {}): StreamEvent {
  return {
    taskId: "task-1",
    type: StreamEventType.Progress,
    percent: 50,
    message: "halfway done",
    level: "info",
    ...overrides,
  };
}

describe("StreamEventType", () => {
  it("has all expected event types", () => {
    expect(StreamEventType.Progress).toBe("progress");
    expect(StreamEventType.Artifact).toBe("artifact");
    expect(StreamEventType.Log).toBe("log");
  });
});

describe("Stream", () => {
  it("delivers events to subscribers", () => {
    const stream = new Stream("task-1");
    const received: StreamEvent[] = [];

    stream.subscribe("task-1", (ev) => received.push(ev));
    stream.publish(makeEvent());

    expect(received).toHaveLength(1);
    expect(received[0]!.percent).toBe(50);
  });

  it("supports multiple subscribers", () => {
    const stream = new Stream("task-1");
    let count1 = 0;
    let count2 = 0;

    stream.subscribe("task-1", () => { count1++; });
    stream.subscribe("task-1", () => { count2++; });
    stream.publish(makeEvent());

    expect(count1).toBe(1);
    expect(count2).toBe(1);
  });

  it("throws on mismatched task ID in subscribe", () => {
    const stream = new Stream("task-1");
    expect(() => stream.subscribe("task-2", () => {})).toThrow("mismatch");
  });

  it("throws on mismatched task ID in publish", () => {
    const stream = new Stream("task-1");
    stream.subscribe("task-1", () => {});
    expect(() => stream.publish(makeEvent({ taskId: "task-2" }))).toThrow("mismatch");
  });

  it("close clears all subscribers", () => {
    const stream = new Stream("task-1");
    let count = 0;

    stream.subscribe("task-1", () => { count++; });
    stream.publish(makeEvent());
    expect(count).toBe(1);

    stream.close();
    // After close, publishing should not deliver to any subscribers
    // (subscribers map is cleared)
    stream.publish(makeEvent());
    expect(count).toBe(1); // No new events after close
  });

  it("returns unique subscription IDs", () => {
    const stream = new Stream("task-1");
    const id1 = stream.subscribe("task-1", () => {});
    const id2 = stream.subscribe("task-1", () => {});

    expect(id1).not.toBe(id2);
  });

  it("suppresses handler errors without disrupting other subscribers", () => {
    const stream = new Stream("task-1");
    let secondReceived = false;

    // First handler throws
    stream.subscribe("task-1", () => { throw new Error("handler error"); });
    // Second handler should still receive the event
    stream.subscribe("task-1", () => { secondReceived = true; });

    stream.publish(makeEvent());
    expect(secondReceived).toBe(true);
  });

  it("handles publish with no subscribers without error", () => {
    const stream = new Stream("task-1");
    // Should not throw
    expect(() => stream.publish(makeEvent())).not.toThrow();
  });

  it("delivers different event types", () => {
    const stream = new Stream("task-1");
    const received: StreamEvent[] = [];

    stream.subscribe("task-1", (ev) => received.push(ev));

    stream.publish(makeEvent({ type: StreamEventType.Progress, percent: 25 }));
    stream.publish(makeEvent({ type: StreamEventType.Artifact, percent: 50 }));
    stream.publish(makeEvent({ type: StreamEventType.Log, percent: 75, message: "debug info" }));

    expect(received).toHaveLength(3);
    expect(received[0]!.type).toBe(StreamEventType.Progress);
    expect(received[1]!.type).toBe(StreamEventType.Artifact);
    expect(received[2]!.type).toBe(StreamEventType.Log);
  });
});
