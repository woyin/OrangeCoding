import { describe, it, expect } from "@jest/globals";
import { MarkdownRenderer } from "../markdown.js";

describe("MarkdownRenderer", () => {
  const renderer = new MarkdownRenderer(80);

  it("renders bold text with ANSI bold codes", () => {
    const result = renderer.render("This is **bold** text");
    expect(result).toContain("\x1b[1m");
    expect(result).toContain("bold");
    expect(result).toContain("\x1b[22m");
  });

  it("renders italic text with ANSI italic codes", () => {
    const result = renderer.render("This is *italic* text");
    expect(result).toContain("\x1b[3m");
    expect(result).toContain("italic");
  });

  it("renders inline code with dim style", () => {
    const result = renderer.render("Use `npm install` to install");
    expect(result).toContain("npm install");
    expect(result).toContain("\x1b[");
  });

  it("renders code blocks with indentation and border", () => {
    const result = renderer.render("```javascript\nconsole.log('hi')\n```");
    expect(result).toContain("console.log('hi')");
    expect(result).toContain("│");
  });

  it("renders headers with bold and color", () => {
    const result = renderer.render("# Main Title");
    expect(result).toContain("Main Title");
    expect(result).toContain("\x1b[1m");
  });

  it("renders h2 headers distinctly", () => {
    const result = renderer.render("## Sub Title");
    expect(result).toContain("Sub Title");
    expect(result).toContain("\x1b[");
  });

  it("renders bullet lists with markers", () => {
    const result = renderer.render("- item one\n- item two\n- item three");
    expect(result).toContain("•");
    expect(result).toContain("item one");
    expect(result).toContain("item two");
    expect(result).toContain("item three");
  });

  it("renders numbered lists", () => {
    const result = renderer.render("1. first\n2. second\n3. third");
    expect(result).toContain("first");
    expect(result).toContain("second");
    expect(result).toContain("1.");
  });

  it("renders links with URL in parentheses", () => {
    const result = renderer.render("[Click here](https://example.com)");
    expect(result).toContain("Click here");
    expect(result).toContain("https://example.com");
  });

  it("handles empty content", () => {
    const result = renderer.render("");
    expect(result).toBe("");
  });

  it("passes through plain text unchanged", () => {
    const result = renderer.render("Just plain text");
    expect(result).toContain("Just plain text");
  });

  it("renders blockquotes with left border", () => {
    const result = renderer.render("> This is a quote");
    expect(result).toContain("This is a quote");
    expect(result).toContain("│");
  });

  it("renders horizontal rules", () => {
    const result = renderer.render("---");
    expect(result).toMatch(/[─━]/);
  });
});
