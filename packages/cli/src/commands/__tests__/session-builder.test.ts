import { jest } from "@jest/globals";
import { randomUUID } from "node:crypto";
import { defaultConfig } from "@orangecoding/config";
import { SessionBuilder } from "../session-builder.js";

describe("SessionBuilder", () => {
  it("builds an AgentLoop with tool definitions from the default registry", () => {
    const cfg = defaultConfig();
    cfg.providers["openai"] = {
      api_key: "test-key",
    };

    const uuid = randomUUID();
    const builder = new SessionBuilder(cfg);
    const loop = builder.build({
      sessionId: uuid,
      workDir: "/tmp",
    });

    // Verify the AgentLoop was assembled with all required components
    expect(loop.agentID).toBeDefined();
    expect(loop.context).toBeDefined();
    expect(loop.context.sessionID.toString()).toBe(`session-${uuid}`);
    expect(loop.context.workDir).toBe("/tmp");
    expect(loop.provider).toBeDefined();
    expect(loop.provider.name()).toBe("openai");
    expect(loop.executor).toBeDefined();
    expect(loop.toolDefs.length).toBeGreaterThan(0);

    // Verify key tools are registered
    const toolNames = loop.toolDefs.map((t) => t.function.name);
    expect(toolNames).toContain("bash");
    expect(toolNames).toContain("read_file");
    expect(toolNames).toContain("write_file");
    expect(toolNames).toContain("edit_file");
  });

  it("applies harness config overrides to the loop config", () => {
    const cfg = defaultConfig();
    cfg.providers["openai"] = { api_key: "test-key" };
    cfg.harness.reasoning_effort = "low";
    cfg.harness.reasoning_budget_tokens = 2048;

    const builder = new SessionBuilder(cfg);
    const loop = builder.build({
      sessionId: randomUUID(),
    });

    expect(loop.config.reasoning.effort).toBe("low");
    expect(loop.config.reasoning.budgetTokens).toBe(2048);
  });

  it("sets a custom system prompt when provided", () => {
    const cfg = defaultConfig();
    cfg.providers["openai"] = { api_key: "test-key" };

    const builder = new SessionBuilder(cfg);
    const loop = builder.build({
      sessionId: randomUUID(),
      systemPrompt: "You are a helpful coding assistant.",
    });

    const sysPrompt = loop.context.conversation.systemPrompt();
    expect(sysPrompt).toContain("helpful coding assistant");
  });
});
