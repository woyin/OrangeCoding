import { MessageBus } from "../bus.js";

describe("MessageBus", () => {
  let bus: MessageBus;

  beforeEach(() => {
    bus = new MessageBus();
  });

  describe("subscribe", () => {
    it("returns a subscription ID", () => {
      const id = bus.subscribe("topic", () => {});
      expect(typeof id).toBe("string");
      expect(id.length).toBeGreaterThan(0);
    });
  });

  describe("publish", () => {
    it("invokes handler with topic and data", (done) => {
      bus.subscribe("my-topic", (topic, data) => {
        expect(topic).toBe("my-topic");
        expect(data).toEqual({ key: "value" });
        done();
      });
      bus.publish("my-topic", { key: "value" });
    });

    it("invokes multiple handlers for the same topic", (done) => {
      let callCount = 0;
      bus.subscribe("t", () => { callCount++; });
      bus.subscribe("t", () => {
        callCount++;
        expect(callCount).toBe(2);
        done();
      });
      bus.publish("t", null);
    });

    it("does not invoke handlers for other topics", () => {
      let called = false;
      bus.subscribe("topic-a", () => { called = true; });
      bus.publish("topic-b", "data");
      expect(called).toBe(false);
    });

    it("does nothing when publishing to a topic with no subscribers", () => {
      expect(() => bus.publish("no-subs", "data")).not.toThrow();
    });

    it("catches handler errors without affecting other handlers", (done) => {
      bus.subscribe("t", () => { throw new Error("boom"); });
      bus.subscribe("t", () => {
        // This handler should still be called
        done();
      });
      bus.publish("t", null);
    });
  });

  describe("unsubscribe", () => {
    it("removes a specific handler", () => {
      let called = false;
      const id = bus.subscribe("t", () => { called = true; });
      bus.unsubscribe("t", id);
      bus.publish("t", null);
      expect(called).toBe(false);
    });

    it("does not affect other handlers on the same topic", (done) => {
      let called1 = false;
      const id1 = bus.subscribe("t", () => { called1 = true; });
      bus.subscribe("t", () => {
        expect(called1).toBe(false);
        done();
      });
      bus.unsubscribe("t", id1);
      bus.publish("t", null);
    });

    it("is a no-op when unsubscribing non-existent ID", () => {
      expect(() => bus.unsubscribe("t", "non-existent")).not.toThrow();
    });

    it("is a no-op when unsubscribing from non-existent topic", () => {
      expect(() => bus.unsubscribe("no-topic", "id")).not.toThrow();
    });
  });
});
