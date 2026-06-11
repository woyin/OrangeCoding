import { jest } from "@jest/globals";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { SessionId, Message, Role, TokenUsage } from "@orangecoding/core";
import { Session, SessionManager } from "@orangecoding/session";
import { restoreConversation } from "../resume-helper.js";

describe("Session Resume", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "orangecoding-resume-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("restores conversation messages from a saved session", async () => {
    const mgr = new SessionManager(dir);
    const session = mgr.create();
    session.addMessage(new Message(Role.System, "You are a coding agent.", new Date()));
    session.addMessage(new Message(Role.User, "fix the bug", new Date()));
    session.addMessage(new Message(Role.Assistant, "I fixed it.", new Date()));
    await mgr.update(session);

    // Restore the conversation
    const { messages, sessionId } = await restoreConversation(dir, session.id);

    expect(sessionId.toString()).toBe(session.id.toString());
    expect(messages.length).toBe(3);
    expect(messages[0]!.role).toBe("system");
    expect(messages[0]!.content).toBe("You are a coding agent.");
    expect(messages[1]!.role).toBe("user");
    expect(messages[1]!.content).toBe("fix the bug");
    expect(messages[2]!.role).toBe("assistant");
    expect(messages[2]!.content).toBe("I fixed it.");
  });

  it("throws when session is not found", async () => {
    const badId = SessionId.create();
    await expect(restoreConversation(dir, badId)).rejects.toThrow();
  });

  it("restores sessions with tool call history", async () => {
    const mgr = new SessionManager(dir);
    const session = mgr.create();
    session.addMessage(new Message(Role.User, "run tests", new Date()));
    session.addMessage(new Message(Role.Assistant, "", new Date(), undefined, [
      { id: "tc-1", function_name: "bash", arguments: { command: "npm test" } },
    ]));
    session.addMessage(new Message(Role.Tool, "all pass", new Date(), undefined, undefined, "tc-1"));
    session.addMessage(new Message(Role.Assistant, "Tests passed!", new Date()));
    await mgr.update(session);

    const { messages } = await restoreConversation(dir, session.id);
    expect(messages.length).toBe(4);
    expect(messages[1]!.toolCalls).toBeDefined();
    expect(messages[1]!.toolCalls![0]!.function_name).toBe("bash");
    expect(messages[2]!.toolCallID).toBe("tc-1");
  });
});
