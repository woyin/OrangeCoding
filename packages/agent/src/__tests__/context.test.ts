/**
 * Tests for the AgentContext module — conversation management,
 * message appending, and system prompt handling.
 */

import { AgentContext } from "../context.js";
import { SessionId, Role } from "@orangecoding/core";
import type { ToolResult } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeContext(): AgentContext {
  const sid = SessionId.create();
  return new AgentContext(sid, "/tmp/workspace");
}

// ---------------------------------------------------------------------------
// Constructor and basic properties
// ---------------------------------------------------------------------------

describe("AgentContext", () => {
  it("initializes with an empty conversation", () => {
    const ctx = makeContext();
    expect(ctx.conversation.length).toBe(0);
    expect(ctx.conversation.isEmpty()).toBe(true);
  });

  it("stores the session ID", () => {
    const sid = SessionId.create();
    const ctx = new AgentContext(sid, "/workspace");
    expect(ctx.sessionID).toBe(sid);
  });

  it("stores the working directory", () => {
    const ctx = new AgentContext(SessionId.create(), "/my/project");
    expect(ctx.workDir).toBe("/my/project");
  });
});

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

describe("AgentContext — system prompt", () => {
  it("sets the system prompt", () => {
    const ctx = makeContext();
    ctx.setSystemPrompt("You are a helpful assistant.");

    expect(ctx.conversation.systemPrompt()).toBe("You are a helpful assistant.");
    expect(ctx.conversation.length).toBe(1);
  });

  it("replaces the system prompt", () => {
    const ctx = makeContext();
    ctx.setSystemPrompt("First prompt");
    ctx.setSystemPrompt("Second prompt");

    expect(ctx.conversation.systemPrompt()).toBe("Second prompt");
    expect(ctx.conversation.length).toBe(1); // Still only 1 message
  });
});

// ---------------------------------------------------------------------------
// Message appending
// ---------------------------------------------------------------------------

describe("AgentContext — messages", () => {
  it("appends user messages", () => {
    const ctx = makeContext();
    ctx.addUserMessage("Hello");
    ctx.addUserMessage("How are you?");

    expect(ctx.conversation.length).toBe(2);
    const msgs = ctx.conversation.messages();
    expect(msgs[0]!.role).toBe(Role.User);
    expect(msgs[0]!.content).toBe("Hello");
    expect(msgs[1]!.content).toBe("How are you?");
  });

  it("appends assistant messages", () => {
    const ctx = makeContext();
    ctx.addAssistantMessage("I'm doing well.");

    expect(ctx.conversation.length).toBe(1);
    const msg = ctx.conversation.messages()[0]!;
    expect(msg.role).toBe(Role.Assistant);
    expect(msg.content).toBe("I'm doing well.");
  });

  it("appends tool results", () => {
    const ctx = makeContext();
    const result: ToolResult = {
      toolCallID: "tc-1",
      content: "output",
      isError: false,
      toMessage: () => ({
        role: Role.Tool,
        content: "output",
        createdAt: new Date(),
        toolCallID: "tc-1",
        hasToolCalls: () => false,
        toJSON: () => ({
          role: "tool",
          content: "output",
          tool_call_id: "tc-1",
          created_at: new Date().toISOString(),
        }),
      }),
    };

    ctx.addToolResult(result);
    expect(ctx.conversation.length).toBe(1);
    const msg = ctx.conversation.messages()[0]!;
    expect(msg.role).toBe(Role.Tool);
  });

  it("maintains message order across types", () => {
    const ctx = makeContext();
    ctx.setSystemPrompt("System prompt");
    ctx.addUserMessage("User 1");
    ctx.addAssistantMessage("Assistant 1");
    ctx.addUserMessage("User 2");

    expect(ctx.conversation.length).toBe(4);
    const msgs = ctx.conversation.messages();
    expect(msgs[0]!.role).toBe(Role.System);
    expect(msgs[1]!.role).toBe(Role.User);
    expect(msgs[2]!.role).toBe(Role.Assistant);
    expect(msgs[3]!.role).toBe(Role.User);
  });
});

// ---------------------------------------------------------------------------
// applyHarnessProfile
// ---------------------------------------------------------------------------

describe("AgentContext — applyHarnessProfile", () => {
  it("applies harness profile addendum to system prompt", () => {
    const ctx = makeContext();
    ctx.setSystemPrompt("Original prompt");

    const profile = {
      systemPromptAddendum: () => "\n\n[OrangeCoding Harness]\nAdditional instructions.",
    };

    ctx.applyHarnessProfile(profile as any);
    const prompt = ctx.conversation.systemPrompt();
    expect(prompt).toContain("Original prompt");
    expect(prompt).toContain("[OrangeCoding Harness]");
  });

  it("does not double-apply the harness profile", () => {
    const ctx = makeContext();
    ctx.setSystemPrompt("Original prompt");

    const profile = {
      systemPromptAddendum: () => "\n\n[OrangeCoding Harness]\nAdditional instructions.",
    };

    ctx.applyHarnessProfile(profile as any);
    ctx.applyHarnessProfile(profile as any);

    const prompt = ctx.conversation.systemPrompt()!;
    // Count occurrences of the marker
    const count = (prompt.match(/\[OrangeCoding Harness\]/g) || []).length;
    expect(count).toBe(1);
  });

  it("creates system prompt if none exists", () => {
    const ctx = makeContext();
    // No system prompt set yet

    const profile = {
      systemPromptAddendum: () => "[OrangeCoding Harness]\nInstructions.",
    };

    ctx.applyHarnessProfile(profile as any);
    expect(ctx.conversation.systemPrompt()).toContain("[OrangeCoding Harness]");
  });
});
