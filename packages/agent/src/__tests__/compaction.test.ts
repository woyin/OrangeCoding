import { jest } from "@jest/globals";
import { Conversation, Message, Role } from "@orangecoding/core";
import { Compactor } from "../compaction.js";

describe("Compactor", () => {
  it("does not compact when under the token limit", () => {
    const compactor = new Compactor(10000);
    const conv = Conversation.create();
    conv.addMessage(new Message(Role.User, "hello", new Date()));
    conv.addMessage(new Message(Role.Assistant, "hi there", new Date()));

    const before = conv.length;
    compactor.compact(conv);
    expect(conv.length).toBe(before);
  });

  it("compacts when over the token limit", () => {
    const compactor = new Compactor(50); // Very low limit
    const conv = Conversation.createWithSystemPrompt("You are a coding agent.");

    // Add many messages to exceed the limit
    for (let i = 0; i < 20; i++) {
      conv.addMessage(new Message(Role.User, "task number " + i + " with some extra context to use more tokens", new Date()));
      conv.addMessage(new Message(Role.Assistant, "response to task " + i + " with detailed explanation and code", new Date()));
    }

    const beforeLen = conv.length;
    const beforeTokens = conv.tokenEstimate();

    compactor.compact(conv);

    expect(conv.length).toBeLessThan(beforeLen);
    // System prompt should be preserved
    const msgs = conv.messages();
    expect(msgs[0]!.role).toBe("system");
    expect(msgs[0]!.content).toBe("You are a coding agent.");
  });

  it("preserves the most recent messages during compaction", () => {
    const compactor = new Compactor(100);
    const conv = Conversation.createWithSystemPrompt("system");

    for (let i = 0; i < 15; i++) {
      conv.addMessage(new Message(Role.User, "message " + i, new Date()));
      conv.addMessage(new Message(Role.Assistant, "response " + i, new Date()));
    }

    compactor.compact(conv);

    const msgs = conv.messages();
    // The last few messages should be preserved
    const lastMsg = msgs[msgs.length - 1]!;
    expect(lastMsg.content).toBe("response 14");
  });

  it("does not compact conversations with fewer than 7 messages", () => {
    const compactor = new Compactor(1); // Extremely low limit
    const conv = Conversation.create();
    conv.addMessage(new Message(Role.User, "hello", new Date()));
    conv.addMessage(new Message(Role.Assistant, "hi", new Date()));
    conv.addMessage(new Message(Role.User, "how are you", new Date()));

    const before = conv.length;
    compactor.compact(conv);
    expect(conv.length).toBe(before); // Should not compact
  });
});
