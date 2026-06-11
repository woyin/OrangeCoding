import {
  SendTaskCommand,
  ApproveCommand,
  CancelCommand,
  TaskUpdateEvent,
  ToolCallEvent,
  ApprovalRequestEvent,
  ErrorEvent,
  AgentStreamEvent,
  AgentCompletedEvent,
  GuardrailEvent,
} from "../messages.js";

// ---------------------------------------------------------------------------
// ClientCommand
// ---------------------------------------------------------------------------

describe("ClientCommand", () => {
  describe("SendTaskCommand", () => {
    it("returns correct command type", () => {
      const cmd = new SendTaskCommand("sess-1", "do something");
      expect(cmd.commandType()).toBe("send_task");
    });

    it("stores sessionId and task", () => {
      const cmd = new SendTaskCommand("sess-1", "do something");
      expect(cmd.sessionId).toBe("sess-1");
      expect(cmd.task).toBe("do something");
    });
  });

  describe("ApproveCommand", () => {
    it("returns correct command type", () => {
      const cmd = new ApproveCommand("req-1", true);
      expect(cmd.commandType()).toBe("approve");
    });

    it("stores requestId and approved", () => {
      const cmd = new ApproveCommand("req-1", false);
      expect(cmd.requestId).toBe("req-1");
      expect(cmd.approved).toBe(false);
    });
  });

  describe("CancelCommand", () => {
    it("returns correct command type", () => {
      const cmd = new CancelCommand("sess-1");
      expect(cmd.commandType()).toBe("cancel");
    });

    it("stores sessionId", () => {
      const cmd = new CancelCommand("sess-2");
      expect(cmd.sessionId).toBe("sess-2");
    });
  });
});

// ---------------------------------------------------------------------------
// ServerEvent
// ---------------------------------------------------------------------------

describe("ServerEvent", () => {
  describe("TaskUpdateEvent", () => {
    it("returns correct event type", () => {
      const evt = new TaskUpdateEvent("s1", "running", "processing");
      expect(evt.eventType()).toBe("task_update");
    });

    it("stores all fields", () => {
      const evt = new TaskUpdateEvent("s1", "completed", "done");
      expect(evt.sessionId).toBe("s1");
      expect(evt.status).toBe("completed");
      expect(evt.message).toBe("done");
    });
  });

  describe("ToolCallEvent", () => {
    it("returns correct event type", () => {
      const evt = new ToolCallEvent("s1", "bash", "ls", "ok", false);
      expect(evt.eventType()).toBe("tool_call");
    });

    it("stores isError flag", () => {
      const evt = new ToolCallEvent("s1", "bash", "bad", "error", true);
      expect(evt.isError).toBe(true);
    });
  });

  describe("ApprovalRequestEvent", () => {
    it("returns correct event type", () => {
      const evt = new ApprovalRequestEvent("r1", "bash", "rm -rf", "approve?");
      expect(evt.eventType()).toBe("approval_request");
    });
  });

  describe("ErrorEvent", () => {
    it("returns correct event type", () => {
      const evt = new ErrorEvent("s1", "something went wrong");
      expect(evt.eventType()).toBe("error");
    });
  });

  describe("AgentStreamEvent", () => {
    it("returns correct event type", () => {
      const evt = new AgentStreamEvent("s1", "chunk");
      expect(evt.eventType()).toBe("agent_stream");
    });
  });

  describe("AgentCompletedEvent", () => {
    it("returns correct event type", () => {
      const evt = new AgentCompletedEvent("s1", "done", 5, 1000, 2000, "completed");
      expect(evt.eventType()).toBe("agent_completed");
    });

    it("stores metrics", () => {
      const evt = new AgentCompletedEvent("s1", "answer", 3, 500, 1200, "stop");
      expect(evt.toolCallsMade).toBe(3);
      expect(evt.tokensUsed).toBe(500);
      expect(evt.durationMs).toBe(1200);
      expect(evt.stopReason).toBe("stop");
    });
  });

  describe("GuardrailEvent", () => {
    it("returns correct event type", () => {
      const evt = new GuardrailEvent("s1", "pre", "allow", "ok", "safety");
      expect(evt.eventType()).toBe("guardrail");
    });

    it("stores decision and reason", () => {
      const evt = new GuardrailEvent("s1", "post", "deny", "too dangerous", "safety");
      expect(evt.decision).toBe("deny");
      expect(evt.reason).toBe("too dangerous");
      expect(evt.guardrailName).toBe("safety");
    });
  });
});
