/**
 * Example test for @orangecoding/core types.
 */

import { TokenUsage } from "../types.js";

describe("TokenUsage", () => {
  test("create returns correct values", () => {
    const usage = TokenUsage.create(100, 50);
    expect(usage.promptTokens).toBe(100);
    expect(usage.completionTokens).toBe(50);
    expect(usage.totalTokens).toBe(150);
  });

  test("accumulate adds values correctly", () => {
    const usage1 = TokenUsage.create(100, 50);
    const usage2 = TokenUsage.create(200, 100);
    usage1.accumulate(usage2);
    expect(usage1.promptTokens).toBe(300);
    expect(usage1.completionTokens).toBe(150);
    expect(usage1.totalTokens).toBe(450);
  });

  test("isEmpty returns true for zero usage", () => {
    const usage = TokenUsage.create(0, 0);
    expect(usage.isEmpty()).toBe(true);
  });

  test("isEmpty returns false for non-zero usage", () => {
    const usage = TokenUsage.create(100, 0);
    expect(usage.isEmpty()).toBe(false);
  });

  test("toJSON returns correct format", () => {
    const usage = TokenUsage.create(100, 50);
    const json = usage.toJSON();
    expect(json).toEqual({
      prompt_tokens: 100,
      completion_tokens: 50,
      total_tokens: 150,
    });
  });
});
