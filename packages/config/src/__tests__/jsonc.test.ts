/**
 * Tests for the JSONC (JSON with comments) preprocessor.
 *
 * Covers: line comments, block comments, string preservation,
 *   escaped characters, and error cases.
 */

import { parseJSONC } from "../jsonc.js";

describe("parseJSONC", () => {
  // -- Basic JSON passthrough --

  it("passes through valid JSON unchanged", () => {
    const input = '{"key": "value", "num": 42}';
    expect(parseJSONC(input)).toBe(input);
  });

  it("handles nested JSON", () => {
    const input = '{"a": {"b": [1, 2, 3]}}';
    expect(parseJSONC(input)).toBe(input);
  });

  // -- Line comments --

  it("strips line comments", () => {
    const input = '{\n  // this is a comment\n  "key": "value"\n}';
    const result = parseJSONC(input);
    expect(result).not.toContain("//");
    expect(result).toContain('"key"');
    // Should be valid JSON after stripping
    expect(() => JSON.parse(result)).not.toThrow();
  });

  it("strips trailing line comments", () => {
    const input = '{"key": "value" // comment\n}';
    const result = parseJSONC(input);
    expect(result).not.toContain("comment");
    expect(() => JSON.parse(result)).not.toThrow();
  });

  // -- Block comments --

  it("strips block comments", () => {
    const input = '{ /* block comment */ "key": "value" }';
    const result = parseJSONC(input);
    expect(result).not.toContain("block comment");
    expect(() => JSON.parse(result)).not.toThrow();
  });

  it("strips multi-line block comments", () => {
    const input = '{\n  /*\n   * multi-line\n   * comment\n   */\n  "key": "value"\n}';
    const result = parseJSONC(input);
    expect(result).not.toContain("multi-line");
    expect(() => JSON.parse(result)).not.toThrow();
  });

  // -- String preservation --

  it("preserves // inside string literals", () => {
    const input = '{"url": "https://example.com"}';
    const result = parseJSONC(input);
    expect(result).toContain("https://example.com");
  });

  it("preserves /* inside string literals", () => {
    const input = '{"note": "use /* comment */ syntax"}';
    const result = parseJSONC(input);
    expect(result).toContain("/* comment */");
  });

  // -- Escape handling --

  it("handles escaped quotes in strings", () => {
    const input = '{"msg": "he said \\"hello\\""}';
    const result = parseJSONC(input);
    expect(result).toContain('\\"hello\\"');
  });

  it("handles escaped backslashes before quotes", () => {
    const input = '{"path": "C:\\\\\\\\Users"}';
    const result = parseJSONC(input);
    expect(result).toContain("C:\\\\\\\\Users");
  });

  // -- Error cases --

  it("throws on unterminated block comment", () => {
    const input = '{ /* unclosed comment "key": "value" }';
    expect(() => parseJSONC(input)).toThrow("unterminated block comment");
  });

  it("throws on unterminated string literal", () => {
    const input = '{"key": "unterminated';
    expect(() => parseJSONC(input)).toThrow("unterminated string");
  });

  // -- Edge cases --

  it("handles empty input", () => {
    expect(parseJSONC("")).toBe("");
  });

  it("handles input with only comments", () => {
    const result = parseJSONC("// just a comment\n");
    expect(result.trim()).toBe("");
  });

  it("preserves line comments at the end of input without newline", () => {
    const input = '{"key": 1} // trailing';
    const result = parseJSONC(input);
    expect(result).not.toContain("trailing");
  });
});
