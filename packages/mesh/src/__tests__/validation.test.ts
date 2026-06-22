/**
 * Tests for the mesh validation module.
 */

import { OutputValidator } from "../validation.js";
import type { ToolResult } from "@orangecoding/core";

function makeResult(overrides: Partial<ToolResult> = {}): ToolResult {
  return {
    toolCallID: "tc-1",
    content: "ok",
    isError: false,
    ...overrides,
  };
}

describe("OutputValidator", () => {
  it("validates a normal result", () => {
    const v = new OutputValidator(1000);
    const [valid, warnings] = v.validate(makeResult());
    expect(valid).toBe(true);
    expect(warnings).toHaveLength(0);
  });

  it("rejects oversized output", () => {
    const v = new OutputValidator(5);
    const [valid, warnings] = v.validate(makeResult({ content: "this is way too long" }));
    expect(valid).toBe(false);
    expect(warnings.length).toBeGreaterThan(0);
    expect(warnings[0]).toContain("exceeds limit");
  });

  it("warns on error results", () => {
    const v = new OutputValidator(1000);
    const [valid, warnings] = v.validate(makeResult({ isError: true, content: "error" }));
    expect(warnings).toContain("tool returned error");
  });

  it("skips size check when maxSize is 0 (unlimited)", () => {
    const v = new OutputValidator(0);
    const longContent = "x".repeat(100000);
    const [valid, warnings] = v.validate(makeResult({ content: longContent }));
    expect(valid).toBe(true);
    expect(warnings).toHaveLength(0);
  });

  it("reports both size and error warnings together", () => {
    const v = new OutputValidator(5);
    const [valid, warnings] = v.validate(makeResult({ content: "too long!", isError: true }));
    expect(valid).toBe(false);
    expect(warnings.length).toBe(2);
  });
});
