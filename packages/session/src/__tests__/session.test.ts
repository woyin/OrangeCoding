/**
 * Tests for the Session class — message management, metadata, token usage,
 * and session lifecycle methods.
 */

import { Session, SessionManager } from "../session.js";
import { SessionId, TokenUsage, Role, newUserMessage, newAssistantMessage } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeSession(overrides: Partial<{
  messages: any[];
  metadata: Record<string, string>;
  tokenUsage: TokenUsage;
  parentID: SessionId;
}> = {}): Session {
  const now = new Date();
  return new Session(
    SessionId.create(),
    overrides.messages ?? [],
    overrides.metadata ?? {},
    overrides.tokenUsage ?? TokenUsage.create(0, 0),
    now,
    now,
    overrides.parentID,
  );
}

// ---------------------------------------------------------------------------
// Constructor and properties
// ---------------------------------------------------------------------------

describe("Session", () => {
  it("creates a session with an ID", () => {
    const s = makeSession();
    expect(s.id).toBeTruthy();
    expect(s.messages).toHaveLength(0);
    expect(s.createdAt).toBeInstanceOf(Date);
    expect(s.updatedAt).toBeInstanceOf(Date);
  });

  it("exposes messages as readonly", () => {
    const s = makeSession();
    const msgs = s.messages;
    expect(Array.isArray(msgs)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Message management
// ---------------------------------------------------------------------------

describe("Session — messages", () => {
  it("adds messages to the session", () => {
    const s = makeSession();
    s.addMessage(newUserMessage("Hello"));
    s.addMessage(newAssistantMessage("Hi there!"));

    expect(s.messages).toHaveLength(2);
    expect(s.messages[0]!.role).toBe(Role.User);
    expect(s.messages[1]!.role).toBe(Role.Assistant);
  });

  it("replaces all messages with setMessages", () => {
    const s = makeSession();
    s.addMessage(newUserMessage("Old message"));

    s.setMessages([
      newUserMessage("New message 1"),
      newUserMessage("New message 2"),
    ]);

    expect(s.messages).toHaveLength(2);
    expect(s.messages[0]!.content).toBe("New message 1");
  });
});

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

describe("Session — metadata", () => {
  it("sets and retrieves metadata", () => {
    const s = makeSession();
    s.setMetadata("provider", "openai");
    s.setMetadata("model", "gpt-4");

    expect(s.metadata["provider"]).toBe("openai");
    expect(s.metadata["model"]).toBe("gpt-4");
  });

  it("deletes metadata", () => {
    const s = makeSession();
    s.setMetadata("key", "value");
    s.deleteMetadata("key");

    expect(s.metadata["key"]).toBeUndefined();
  });

  it("overwrites existing metadata", () => {
    const s = makeSession();
    s.setMetadata("key", "old");
    s.setMetadata("key", "new");

    expect(s.metadata["key"]).toBe("new");
  });
});

// ---------------------------------------------------------------------------
// Token usage
// ---------------------------------------------------------------------------

describe("Session — token usage", () => {
  it("starts with zero token usage", () => {
    const s = makeSession();
    expect(s.tokenUsage.totalTokens).toBe(0);
  });

  it("updates token usage", () => {
    const s = makeSession();
    s.setTokenUsage(TokenUsage.create(100, 200));

    expect(s.tokenUsage.totalTokens).toBe(300);
  });
});

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

describe("Session — lifecycle", () => {
  it("markUpdated updates the updatedAt timestamp", async () => {
    const s = makeSession();
    const before = s.updatedAt;

    // Small delay to ensure timestamp differs
    await new Promise((r) => setTimeout(r, 10));
    s.markUpdated();

    expect(s.updatedAt.getTime()).toBeGreaterThanOrEqual(before.getTime());
  });
});

// ---------------------------------------------------------------------------
// SessionManager — create
// ---------------------------------------------------------------------------

describe("SessionManager — create", () => {
  it("creates a new session with empty state", () => {
    const mgr = new SessionManager("/tmp/test-sessions");
    const session = mgr.create();

    expect(session.id).toBeTruthy();
    expect(session.messages).toHaveLength(0);
    expect(Object.keys(session.metadata)).toHaveLength(0);
    expect(session.tokenUsage.totalTokens).toBe(0);
  });

  it("creates sessions with unique IDs", () => {
    const mgr = new SessionManager("/tmp/test-sessions");
    const s1 = mgr.create();
    const s2 = mgr.create();

    expect(s1.id.toString()).not.toBe(s2.id.toString());
  });
});
