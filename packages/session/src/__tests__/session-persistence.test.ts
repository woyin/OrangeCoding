import { jest } from "@jest/globals";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { SessionId, TokenUsage, Message, Role } from "@orangecoding/core";
import { Session, SessionManager, writeSession, readSession } from "../index.js";

describe("Session Persistence", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "orangecoding-session-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("saves and loads a session with messages", async () => {
    const sid = SessionId.create();
    const now = new Date();
    const messages = [
      new Message(Role.System, "You are a coding agent.", now),
      new Message(Role.User, "fix the bug", now),
      new Message(Role.Assistant, "I will look into it.", now),
    ];

    const session = new Session(
      sid,
      messages,
      { task: "fix the bug" },
      TokenUsage.create(100, 50),
      now,
      now,
    );

    await writeSession(dir, session);
    const loaded = await readSession(dir, sid);

    expect(loaded.id.toString()).toBe(sid.toString());
    expect(loaded.messages.length).toBe(3);
    expect(loaded.messages[0]!.role).toBe("system");
    expect(loaded.messages[1]!.content).toBe("fix the bug");
    expect(loaded.messages[2]!.content).toBe("I will look into it.");
    expect(loaded.tokenUsage.totalTokens).toBe(150);
    expect(loaded.metadata["task"]).toBe("fix the bug");
  });

  it("lists sessions sorted by updatedAt descending", async () => {
    const mgr = new SessionManager(dir);

    for (let i = 0; i < 3; i++) {
      const s = mgr.create();
      s.addMessage(new Message(Role.User, "task " + i, new Date()));
      s.setMetadata("task", "task " + i);
      await mgr.update(s);
    }

    const listed = await mgr.list();
    expect(listed.length).toBe(3);
    expect(listed[0]!.updatedAt.getTime()).toBeGreaterThanOrEqual(listed[1]!.updatedAt.getTime());
  });

  it("round-trips tool call messages", async () => {
    const sid = SessionId.create();
    const now = new Date();
    const messages = [
      new Message(Role.User, "run tests", now),
      new Message(Role.Assistant, "", now, undefined, [
        { id: "tc-1", function_name: "bash", arguments: { command: "npm test" } },
      ]),
      new Message(Role.Tool, "all tests pass", now, undefined, undefined, "tc-1"),
    ];

    const session = new Session(sid, messages, {}, new TokenUsage(0, 0, 0), now, now);
    await writeSession(dir, session);

    const loaded = await readSession(dir, sid);
    expect(loaded.messages.length).toBe(3);
    expect(loaded.messages[1]!.toolCalls).toBeDefined();
    expect(loaded.messages[1]!.toolCalls!.length).toBe(1);
    expect(loaded.messages[1]!.toolCalls![0]!.function_name).toBe("bash");
    expect(loaded.messages[2]!.toolCallID).toBe("tc-1");
  });
});
