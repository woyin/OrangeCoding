/**
 * Tests for UltraWork multi-agent parallel execution.
 */

import { IntentGate } from "../intent-gate.js";

// We test the IntentGate-based agent selection logic indirectly,
// since UltraWork requires an AiProvider to instantiate.

describe("IntentGate integration for UltraWork", () => {
  const gate = new IntentGate();

  test("coding tasks get coding intent", () => {
    const analysis = gate.analyze("implement the new feature in auth.ts");
    expect(["coding", "general"]).toContain(analysis.intent);
    expect(analysis.confidence).toBeGreaterThan(0);
  });

  test("planning tasks want planning", () => {
    const analysis = gate.analyze("plan the architecture for the new system");
    expect(analysis.wantsPlanning).toBe(true);
  });

  test("parallel keywords trigger wantsParallel", () => {
    const analysis = gate.analyze("fix all files in parallel simultaneously");
    expect(analysis.wantsParallel).toBe(true);
  });

  test("questions are detected", () => {
    const analysis = gate.analyze("how does the authentication system work?");
    expect(analysis.isQuestion).toBe(true);
  });

  test("project scope detected for project-wide tasks", () => {
    const analysis = gate.analyze("refactor all modules in the entire project-wide codebase");
    expect(analysis.scope).toBe("project");
  });

  test("analyze returns valid summary", () => {
    const analysis = gate.analyze("implement the new feature");
    expect(analysis.summary).toBeTruthy();
    expect(analysis.summary.length).toBeGreaterThan(0);
  });

  test("classify returns correct categories", () => {
    expect(gate.classify("write a new function")).toBe("coding");
    expect(gate.classify("plan the roadmap")).toBe("planning");
    expect(gate.classify("review the code")).toBe("coding"); // "code" matches coding first
    expect(gate.classify("inspect the changes")).toBe("review");
    expect(gate.classify("what is this?")).toBe("question");
    expect(gate.classify("find the bug")).toBe("explore");
    expect(gate.classify("hello")).toBe("general");
  });
});
