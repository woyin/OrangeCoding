/**
 * RepeatedToolGuardrail + recentToolKeys 窗口语义测试。
 *
 * 保护目标：recentToolKeys() 重构为“增量扫描 + 窗口修剪”后，
 * 必须仍然让 RepeatedToolGuardrail 正确对同一键的多次出现计数，
 * 既不重复计数（误触发拒绝），也不漏计（重复循环不被拦）。
 */

import { RepeatedToolGuardrail, toolCallKey } from "../harness-guardrail.js";
import type { GuardrailContext, ToolCall } from "@orangecoding/core";

function ctxFor(
  toolCall: ToolCall,
  recentToolKeys: string[],
): GuardrailContext {
  return {
    phase: "pre_tool",
    toolCall,
    output: "",
    recentToolKeys,
    tokenEstimate: 0,
    maxTokens: 0,
  } as unknown as GuardrailContext;
}

describe("RepeatedToolGuardrail 计数语义", () => {
  const call: ToolCall = {
    id: "c1",
    function_name: "bash",
    arguments: { command: "ls" },
  };
  const key = toolCallKey(call);

  it("同一键出现 < limit 次时放行", () => {
    const g = new RepeatedToolGuardrail(3);
    const r = g.check(undefined, ctxFor(call, [key, key]));
    expect(r.decision).toBe("allow");
  });

  it("同一键出现 >= limit 次时拒绝", () => {
    const g = new RepeatedToolGuardrail(3);
    const r = g.check(undefined, ctxFor(call, [key, key, key]));
    expect(r.decision).toBe("deny");
  });

  it("窗口修剪后仍保留 limit 内的重复计数（窗口 ≥ limit）", () => {
    // 模拟 recentToolKeys 在窗口上限内的情况：同一键重复 4 次，limit=3
    const g = new RepeatedToolGuardrail(3);
    const r = g.check(undefined, ctxFor(call, [key, key, key, key]));
    expect(r.decision).toBe("deny");
  });

  it("不同参数的工具调用不互相累加", () => {
    const g = new RepeatedToolGuardrail(3);
    const other: ToolCall = {
      id: "c2",
      function_name: "bash",
      arguments: { command: "pwd" },
    };
    const otherKey = toolCallKey(other);
    const r = g.check(undefined, ctxFor(call, [key, key, otherKey, otherKey]));
    expect(r.decision).toBe("allow");
  });
});
