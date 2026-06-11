import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  AgentId,
  EventBus,
  GuardrailDecisionEvent,
  SessionId,
  StreamChunkEvent,
  ToolCallCompletedEvent,
} from "@orangecoding/core";
import { AuditEventRecorder, AuditLog, verifyChain } from "../index.js";

describe("AuditEventRecorder", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "orangecoding-audit-"));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("persists guardrail and tool events as hash-chained audit entries", async () => {
    const log = await AuditLog.create(dir);
    const recorder = new AuditEventRecorder(log);
    const bus = new EventBus();
    const agentId = AgentId.create();
    const sessionId = SessionId.create();

    bus.subscribe(recorder);

    await bus.publish(new GuardrailDecisionEvent(
      agentId,
      sessionId,
      "pre_tool",
      "deny",
      "dangerous shell command",
      "dangerous_tool",
    ));
    await bus.publish(new ToolCallCompletedEvent(agentId, sessionId, "bash", false, 12));
    await bus.publish(new StreamChunkEvent(agentId, sessionId, "not audited"));

    const entries = await log.getEntries();

    expect(entries).toHaveLength(2);
    expect(entries.map((entry) => entry.action)).toEqual([
      "guardrail_decision",
      "tool_call_completed",
    ]);
    expect(entries.map((entry) => entry.agentId)).toEqual([
      agentId.toString(),
      agentId.toString(),
    ]);
    expect(JSON.parse(entries[0]!.details)).toMatchObject({
      type: "guardrail_decision",
      phase: "pre_tool",
      decision: "deny",
      reason: "dangerous shell command",
      guardrail_name: "dangerous_tool",
      session_id: sessionId.toString(),
    });
    expect(JSON.parse(entries[1]!.details)).toMatchObject({
      type: "tool_call_completed",
      tool_name: "bash",
      success: false,
      duration_ms: 12,
      session_id: sessionId.toString(),
    });
    expect(verifyChain(entries)).toBeNull();
  });
});
