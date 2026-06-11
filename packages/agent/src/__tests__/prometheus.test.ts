/**
 * Tests for Prometheus planning agent with interview mode.
 */

import { needsInterview, type PrometheusConfig } from "../agents/prometheus.js";

const DEFAULT_CONFIG: PrometheusConfig = {
  interviewMode: "auto",
  maxInterviewQuestions: 3,
  enableResearch: true,
};

describe("needsInterview", () => {
  test("returns true when interviewMode is 'always'", () => {
    const config: PrometheusConfig = { ...DEFAULT_CONFIG, interviewMode: "always" };
    expect(needsInterview("fix the typo", config)).toBe(true);
  });

  test("returns false when interviewMode is 'never'", () => {
    const config: PrometheusConfig = { ...DEFAULT_CONFIG, interviewMode: "never" };
    expect(needsInterview("refactor the entire architecture", config)).toBe(false);
  });

  test("returns true for ambiguous tasks in auto mode", () => {
    expect(needsInterview("refactor the code architecture", DEFAULT_CONFIG)).toBe(true);
    expect(needsInterview("improve the system performance", DEFAULT_CONFIG)).toBe(true);
    expect(needsInterview("migrate the database", DEFAULT_CONFIG)).toBe(true);
  });

  test("returns false for clear tasks in auto mode", () => {
    expect(needsInterview("fix the lint error in main.ts", DEFAULT_CONFIG)).toBe(false);
    expect(needsInterview("rename the variable foo to bar", DEFAULT_CONFIG)).toBe(false);
    expect(needsInterview("add a function called processOrder for handling orders", DEFAULT_CONFIG)).toBe(false);
  });

  test("returns true for long tasks in auto mode (> 20 words)", () => {
    const longTask = "I want to build a complete authentication system that supports " +
      "OAuth2, JWT tokens, session management, and role-based access control " +
      "with integration to our existing user database and admin panel";
    expect(needsInterview(longTask, DEFAULT_CONFIG)).toBe(true);
  });

  test("returns false for short clear tasks in auto mode", () => {
    expect(needsInterview("fix the typo", DEFAULT_CONFIG)).toBe(false);
  });
});
