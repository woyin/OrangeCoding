import type { ToolCall } from "@orangecoding/core";
import { jest } from "@jest/globals";
import {
  defaultGuardrailPipeline,
  GuardrailPipeline,
  LLMGuardrail,
} from "../harness-guardrail.js";

function bashCall(command: string): ToolCall {
  return { id: "tool-1", function_name: "bash", arguments: { command } };
}

describe("defaultGuardrailPipeline", () => {
  it("checks pre-model token budget before model calls", async () => {
    const pipeline = defaultGuardrailPipeline({ maxTokens: 100 });

    const result = await pipeline.check(undefined, {
      phase: "pre_model",
      output: "",
      recentToolKeys: [],
      tokenEstimate: 101,
      maxTokens: 0,
    });

    expect(result).toMatchObject({
      decision: "warn",
      name: "token_budget",
    });
  });

  it("checks final output length in the default pipeline", async () => {
    const pipeline = defaultGuardrailPipeline({ maxOutputLength: 5 });

    const result = await pipeline.check(undefined, {
      phase: "final_output",
      output: "too long",
      recentToolKeys: [],
      tokenEstimate: 0,
      maxTokens: 0,
    });

    expect(result).toMatchObject({
      decision: "warn",
      name: "output_length",
    });
  });

  it("keeps dangerous and repeated tool checks enabled by default", async () => {
    const pipeline = defaultGuardrailPipeline();

    const dangerous = await pipeline.check(undefined, {
      phase: "pre_tool",
      toolCall: bashCall("rm -rf /"),
      output: "",
      recentToolKeys: [],
      tokenEstimate: 0,
      maxTokens: 0,
    });

    expect(dangerous).toMatchObject({ decision: "deny", name: "dangerous_tool" });
  });
});

describe("GuardrailPipeline", () => {
  it("awaits asynchronous guardrails and stops on deny", async () => {
    const provider = jest.fn(async () => [false, null] as [boolean, Error | null]);
    const pipeline = new GuardrailPipeline([
      new LLMGuardrail({
        phase: "post_tool",
        prompt: "Reject secrets",
        provider,
      }),
    ]);

    const result = await pipeline.check(undefined, {
      phase: "post_tool",
      output: "API_KEY=secret",
      recentToolKeys: [],
      tokenEstimate: 0,
      maxTokens: 0,
    });

    expect(provider).toHaveBeenCalledWith(undefined, "Reject secrets", "API_KEY=secret");
    expect(result).toMatchObject({
      decision: "deny",
      name: "llm_guardrail",
      reason: "llm guardrail rejected content",
    });
  });
});
