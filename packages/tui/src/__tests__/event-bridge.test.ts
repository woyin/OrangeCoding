import { describe, it, expect } from "@jest/globals";
import { AgentId, SessionId } from "@orangecoding/core";
import { App } from "../app.js";
import { TuiEventBridge } from "../event-bridge.js";
import {
  StreamChunkEvent,
  CompletedEvent,
  ToolCallRequestedEvent,
  ToolCallCompletedEvent,
  ErrorEvent,
  GuardrailDecisionEvent,
} from "@orangecoding/core";

const aid = AgentId.create();
const sid = SessionId.create();

describe("TuiEventBridge", () => {
  it("creates a handler function", () => {
    const app = new App();
    const bridge = new TuiEventBridge(app);
    const handler = bridge.getHandler();
    expect(typeof handler).toBe("function");
  });

  it("accumulates stream chunks", () => {
    const app = new App();
    const bridge = new TuiEventBridge(app);

    bridge.handleEvent(new StreamChunkEvent(aid, sid, "Hello "));
    bridge.handleEvent(new StreamChunkEvent(aid, sid, "World"));

    expect(app.currentStream).toBe("Hello World");
  });

  it("finalizes stream on CompletedEvent", () => {
    const app = new App();
    const bridge = new TuiEventBridge(app);

    bridge.handleEvent(new StreamChunkEvent(aid, sid, "response text"));
    bridge.handleEvent(new CompletedEvent(aid, sid, "done summary"));

    expect(app.currentStream).toBe("");
    // Should have added a message
    expect(app.model.messages.length).toBe(1);
    expect(app.model.messages[0]!.content).toBe("response text");
    // Status should show completion
    expect(app.model.status).toContain("done");
  });

  it("shows tool call status on ToolCallRequestedEvent", () => {
    const app = new App();
    const bridge = new TuiEventBridge(app);

    bridge.handleEvent(
      new ToolCallRequestedEvent(aid, sid, {
        id: "tc1",
        function_name: "bash",
        arguments: { command: "ls" },
      } as any),
    );

    expect(app.model.status).toContain("bash");
  });

  it("shows tool completion on ToolCallCompletedEvent", () => {
    const app = new App();
    const bridge = new TuiEventBridge(app);

    bridge.handleEvent(new ToolCallCompletedEvent(aid, sid, "bash", true, 150));
    expect(app.model.status).toContain("✅");

    bridge.handleEvent(new ToolCallCompletedEvent(aid, sid, "file", false, 200));
    expect(app.model.status).toContain("❌");
  });

  it("shows error on ErrorEvent", () => {
    const app = new App();
    const bridge = new TuiEventBridge(app);

    bridge.handleEvent(new ErrorEvent(aid, sid, "something broke"));
    expect(app.model.status).toContain("something broke");
  });

  it("shows guardrail blocks", () => {
    const app = new App();
    const bridge = new TuiEventBridge(app);

    bridge.handleEvent(
      new GuardrailDecisionEvent(aid, sid, "tool", "deny", "dangerous command", "safety"),
    );
    expect(app.model.status).toContain("blocked");
  });

  it("ignores allow guardrail decisions", () => {
    const app = new App();
    const bridge = new TuiEventBridge(app);

    const initialStatus = app.model.status;
    bridge.handleEvent(
      new GuardrailDecisionEvent(aid, sid, "tool", "allow", "safe", "safety"),
    );
    expect(app.model.status).toBe(initialStatus);
  });
});
