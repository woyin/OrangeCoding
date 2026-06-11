import type { AiProvider, ChatMessage, ChatOptions, StreamEvent, ToolDefinition } from "@orangecoding/ai";
import type { ToolCall } from "@orangecoding/core";
import { AgentId, SessionId } from "@orangecoding/core";
import type { ExecuteResult } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import type { ToolExecutor } from "../executor.js";
import type { Guardrail, GuardrailContext, GuardrailResult } from "../harness-guardrail.js";
import { GuardrailLogger, GuardrailPipeline } from "../harness-guardrail.js";
import { AgentLoop, defaultLoopConfig } from "../loop.js";

class StaticProvider implements AiProvider {
  private readonly events: StreamEvent[];

  constructor(events: StreamEvent[]) {
    this.events = events;
  }

  name(): string { return "static"; }

  async chatCompletion(): Promise<never> {
    throw new Error("not used");
  }

  async chatCompletionStream(
    _messages: ChatMessage[],
    _tools: ToolDefinition[],
    _opts: ChatOptions,
  ): Promise<AsyncIterable<StreamEvent>> {
    const events = this.events;
    return (async function* stream() {
      for (const event of events) {
        yield event;
      }
    })();
  }
}

class RecordingExecutor {
  public calls: ToolCall[] = [];

  constructor(private readonly output: string) {}

  async executeBatch(_signal: AbortSignal | undefined, calls: ToolCall[]): Promise<ExecuteResult[]> {
    this.calls.push(...calls);
    return calls.map((call) => ({
      toolCallID: call.id,
      content: this.output,
      isError: false,
      durationMs: 1,
    }));
  }
}

class PhaseGuardrail implements Guardrail {
  constructor(
    private readonly phase: GuardrailContext["phase"],
    private readonly result: GuardrailResult,
  ) {}

  name(): string { return this.result.name; }

  check(_signal: AbortSignal | undefined, input: GuardrailContext): GuardrailResult {
    if (input.phase !== this.phase) {
      return { decision: "allow", reason: "", name: this.name() };
    }
    return this.result;
  }
}

class CapturingGuardrail implements Guardrail {
  public contexts: GuardrailContext[] = [];

  constructor(private readonly phase: GuardrailContext["phase"]) {}

  name(): string { return `${this.phase}_capture`; }

  check(_signal: AbortSignal | undefined, input: GuardrailContext): GuardrailResult {
    if (input.phase === this.phase) {
      this.contexts.push(input);
    }
    return { decision: "allow", reason: "", name: this.name() };
  }
}

function contentEvent(content: string): StreamEvent {
  return {
    type: "content_delta",
    content,
    tool_call_id: "",
    tool_call_name: "",
    arguments: "",
    usage: null,
  };
}

function toolCallEvent(id: string, name: string, args: string): StreamEvent {
  return {
    type: "tool_call_delta",
    content: "",
    tool_call_id: id,
    tool_call_name: name,
    arguments: args,
    usage: null,
  };
}

function usageEvent(promptTokens: number, completionTokens: number): StreamEvent {
  return {
    type: "usage",
    content: "",
    tool_call_id: "",
    tool_call_name: "",
    arguments: "",
    usage: {
      prompt_tokens: promptTokens,
      completion_tokens: completionTokens,
      total_tokens: promptTokens + completionTokens,
    },
  };
}

function createLoop(
  provider: AiProvider,
  executor: RecordingExecutor,
  guardrails: GuardrailPipeline,
  logger: GuardrailLogger,
): AgentLoop {
  const context = new AgentContext(SessionId.create(), process.cwd());
  context.addUserMessage("run task");
  return new AgentLoop(
    AgentId.create(),
    provider,
    executor as unknown as ToolExecutor,
    context,
    {
      ...defaultLoopConfig(),
      maxIterations: 1,
      guardrails,
      guardrailLogger: logger,
    },
    [],
  );
}

describe("AgentLoop guardrail integration", () => {
  it("passes the conversation token estimate to pre-model guardrails", async () => {
    const logger = new GuardrailLogger();
    const executor = new RecordingExecutor("unused");
    const guardrail = new CapturingGuardrail("pre_model");
    const loop = createLoop(
      new StaticProvider([contentEvent("done")]),
      executor,
      new GuardrailPipeline([guardrail]),
      logger,
    );

    await loop.run({ model: "test" }, null);

    expect(guardrail.contexts).toHaveLength(1);
    expect(guardrail.contexts[0]!.tokenEstimate).toBeGreaterThan(0);
  });

  it("logs pre-model warnings without stopping the model call", async () => {
    const logger = new GuardrailLogger();
    const executor = new RecordingExecutor("unused");
    const loop = createLoop(
      new StaticProvider([contentEvent("done"), usageEvent(3, 2)]),
      executor,
      new GuardrailPipeline([
        new PhaseGuardrail("pre_model", {
          decision: "warn",
          reason: "token budget nearing limit",
          name: "budget_probe",
        }),
      ]),
      logger,
    );

    const result = await loop.run({ model: "test" }, null);

    expect(result.stopReason).toBe("completed");
    expect(result.tokensUsed.totalTokens).toBe(5);
    expect(logger.warnings()).toMatchObject([
      { phase: "pre_model", decision: "warn", reason: "token budget nearing limit" },
    ]);
  });

  it("stops after post-tool deny and records the guardrail decision", async () => {
    const logger = new GuardrailLogger();
    const executor = new RecordingExecutor("SECRET=leaked");
    const loop = createLoop(
      new StaticProvider([toolCallEvent("call-1", "read_file", "{\"path\":\"secret.txt\"}")]),
      executor,
      new GuardrailPipeline([
        new PhaseGuardrail("post_tool", {
          decision: "deny",
          reason: "tool output contains secret",
          name: "secret_output",
        }),
      ]),
      logger,
    );

    const result = await loop.run({ model: "test" }, null);

    expect(executor.calls.map((call) => call.id)).toEqual(["call-1"]);
    expect(result.stopReason).toBe("guardrail");
    expect(logger.recent(1)).toMatchObject([
      { phase: "post_tool", decision: "deny", reason: "tool output contains secret" },
    ]);
  });

  it("stops before tool execution on pre-tool deny and emits a guardrail decision event", async () => {
    const logger = new GuardrailLogger();
    const executor = new RecordingExecutor("should not run");
    const loop = createLoop(
      new StaticProvider([toolCallEvent("call-1", "bash", "{\"command\":\"rm -rf /\"}")]),
      executor,
      new GuardrailPipeline([
        new PhaseGuardrail("pre_tool", {
          decision: "deny",
          reason: "dangerous shell command",
          name: "dangerous_tool",
        }),
      ]),
      logger,
    );
    const events: { eventType: string }[] = [];

    const result = await loop.run({ model: "test" }, (event) => {
      events.push(event);
    });

    expect(result.stopReason).toBe("guardrail");
    expect(executor.calls).toEqual([]);
    expect(events.map((event) => event.eventType)).not.toContain("tool_call_requested");
    const guardrailEvents = events.filter((event) => event.eventType === "guardrail_decision");
    expect(guardrailEvents).toContainEqual(
      expect.objectContaining({
        phase: "pre_tool",
        decision: "deny",
        reason: "dangerous shell command",
      }),
    );
    expect(guardrailEvents.at(-1)?.toJSON()).toMatchObject({
      type: "guardrail_decision",
      phase: "pre_tool",
      decision: "deny",
      reason: "dangerous shell command",
      guardrail_name: "dangerous_tool",
    });
    expect(logger.recent(1)).toMatchObject([
      { phase: "pre_tool", decision: "deny", reason: "dangerous shell command" },
    ]);
  });
});
