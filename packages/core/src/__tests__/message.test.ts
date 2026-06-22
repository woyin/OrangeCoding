import {
  Conversation,
  newSystemMessage,
  newUserMessage,
  newAssistantMessage,
  newAssistantMessageWithToolCalls,
  newToolResultMessage,
} from "../message.js";

describe("Conversation.tokenEstimate", () => {
  test("empty conversation is zero tokens", () => {
    expect(Conversation.create().tokenEstimate()).toBe(0);
  });

  test("pure ASCII ≈ 1 token per 4 chars", () => {
    const conv = Conversation.create();
    // 40 ASCII chars → floor(40/4) = 10 tokens.
    conv.addMessage(newUserMessage("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    expect(conv.tokenEstimate()).toBe(10);
  });

  test("CJK characters count as 2 tokens each", () => {
    const conv = Conversation.create();
    // 3 CJK chars → 3*2 = 6 tokens.
    conv.addMessage(newUserMessage("中文字"));
    expect(conv.tokenEstimate()).toBe(6);
  });

  test("mixed CJK + ASCII is weighted correctly", () => {
    const conv = Conversation.create();
    // 4 CJK (8 tokens) + 8 ASCII (2 tokens) = 10.
    conv.addMessage(newUserMessage("测试中文abcdefgh"));
    expect(conv.tokenEstimate()).toBe(10);
  });

  test("tool-call arguments are counted (string args)", () => {
    const conv = Conversation.create();
    conv.addMessage(newAssistantMessageWithToolCalls("", [
      { id: "c1", function_name: "read_file", arguments: '{"path":"/a.ts"}' },
    ]));
    // function_name "read_file" = 9 ascii → floor(9/4)=2
    // arguments '{"path":"/a.ts"}' = 16 ascii → floor(16/4)=4
    // total = 6
    expect(conv.tokenEstimate()).toBe(6);
  });

  test("tool-call arguments are JSON-serialized when object", () => {
    const conv = Conversation.create();
    conv.addMessage(newAssistantMessageWithToolCalls("", [
      { id: "c1", function_name: "fn", arguments: { path: "/a.ts" } },
    ]));
    // The serialized form must match a string arg with the same JSON.
    const conv2 = Conversation.create();
    conv2.addMessage(newAssistantMessageWithToolCalls("", [
      { id: "c1", function_name: "fn", arguments: JSON.stringify({ path: "/a.ts" }) },
    ]));
    expect(conv.tokenEstimate()).toBe(conv2.tokenEstimate());
  });

  test("estimate is additive across messages", () => {
    const conv = Conversation.create();
    conv.addMessage(newUserMessage("abcd"));          // 1 token
    conv.addMessage(newUserMessage("efgh"));          // 1 token
    conv.addMessage(newUserMessage("中文"));          // 4 tokens
    expect(conv.tokenEstimate()).toBe(6);
  });

  test("accumulates across a realistic multi-message conversation", () => {
    const conv = Conversation.createWithSystemPrompt("system");
    conv.addMessage(newUserMessage("hello world"));
    conv.addMessage(newAssistantMessage("hi there"));
    conv.addMessage(newToolResultMessage("t1", "result", false));
    // Smoke: estimate is positive and stable.
    const e1 = conv.tokenEstimate();
    expect(e1).toBeGreaterThan(0);
    expect(conv.tokenEstimate()).toBe(e1);
  });
});

describe("Conversation basics (regression)", () => {
  test("messagesUnsafe returns backing array without copying", () => {
    const conv = Conversation.create();
    conv.addMessage(newUserMessage("a"));
    const before = conv.messagesUnsafe();
    conv.addMessage(newUserMessage("b"));
    // Same reference grows as we append.
    expect(conv.messagesUnsafe()).toBe(before);
    expect(conv.messagesUnsafe().length).toBe(2);
  });

  test("messages() returns a defensive copy", () => {
    const conv = Conversation.create();
    conv.addMessage(newUserMessage("a"));
    const snap = conv.messages();
    conv.addMessage(newUserMessage("b"));
    expect(snap.length).toBe(1);
    expect(conv.length).toBe(2);
  });
});
