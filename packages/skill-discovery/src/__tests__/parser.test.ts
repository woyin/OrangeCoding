/**
 * Tests for the SKILL.md parser — frontmatter parsing, full file parsing,
 * and rendering.
 */

import {
  parseSkillFrontmatter,
  parseSkillMd,
  renderSkillMd,
} from "../parser.js";
import { SkillParseError } from "../types.js";

// ---------------------------------------------------------------------------
// parseSkillFrontmatter — YAML-like frontmatter
// ---------------------------------------------------------------------------

describe("parseSkillFrontmatter", () => {
  it("parses name and description", () => {
    const meta = parseSkillFrontmatter("name: my-skill\ndescription: A test skill");
    expect(meta.name).toBe("my-skill");
    expect(meta.description).toBe("A test skill");
  });

  it("parses version", () => {
    const meta = parseSkillFrontmatter("name: s\ndescription: d\nversion: 1.0.0");
    expect(meta.version).toBe("1.0.0");
  });

  it("parses tools as bracket list", () => {
    const meta = parseSkillFrontmatter("name: s\ndescription: d\ntools: [bash, read_file, write_file]");
    expect(meta.tools).toEqual(["bash", "read_file", "write_file"]);
  });

  it("parses tags as bracket list", () => {
    const meta = parseSkillFrontmatter("name: s\ndescription: d\ntags: [coding, automation]");
    expect(meta.tags).toEqual(["coding", "automation"]);
  });

  it("parses tools as comma-separated values", () => {
    const meta = parseSkillFrontmatter("name: s\ndescription: d\ntools: bash, read_file");
    expect(meta.tools).toEqual(["bash", "read_file"]);
  });

  it("parses single tool value", () => {
    const meta = parseSkillFrontmatter("name: s\ndescription: d\ntools: bash");
    expect(meta.tools).toEqual(["bash"]);
  });

  it("parses examples with list items", () => {
    const raw = [
      "name: s",
      "description: d",
      "examples:",
      "  - fix the bug",
      "  - add tests",
    ].join("\n");
    const meta = parseSkillFrontmatter(raw);
    expect(meta.examples).toEqual(["fix the bug", "add tests"]);
  });

  it("skips empty lines", () => {
    const raw = "name: s\n\ndescription: d\n";
    const meta = parseSkillFrontmatter(raw);
    expect(meta.name).toBe("s");
    expect(meta.description).toBe("d");
  });

  it("returns empty meta for empty input", () => {
    const meta = parseSkillFrontmatter("");
    expect(meta.name).toBe("");
    expect(meta.description).toBe("");
  });
});

// ---------------------------------------------------------------------------
// parseSkillMd — full file parsing
// ---------------------------------------------------------------------------

describe("parseSkillMd", () => {
  const validSkill = [
    "---",
    "name: test-skill",
    "description: A test skill for unit testing",
    "version: 1.0.0",
    "tools: [bash, read_file]",
    "tags: [testing]",
    "examples:",
    "  - run tests",
    "  - fix bugs",
    "---",
    "",
    "# System Prompt",
    "",
    "You are a test agent.",
  ].join("\n");

  it("parses a valid skill file", () => {
    const skill = parseSkillMd(validSkill, "test.md");
    expect(skill.path).toBe("test.md");
    expect(skill.meta.name).toBe("test-skill");
    expect(skill.meta.description).toBe("A test skill for unit testing");
    expect(skill.meta.version).toBe("1.0.0");
    expect(skill.meta.tools).toEqual(["bash", "read_file"]);
    expect(skill.meta.tags).toEqual(["testing"]);
    expect(skill.meta.examples).toEqual(["run tests", "fix bugs"]);
    expect(skill.body).toContain("System Prompt");
    expect(skill.body).toContain("You are a test agent.");
  });

  it("throws on missing frontmatter delimiter", () => {
    expect(() => parseSkillMd("no frontmatter here", "test.md")).toThrow(SkillParseError);
  });

  it("throws on unclosed frontmatter", () => {
    expect(() => parseSkillMd("---\nname: test\n", "test.md")).toThrow("unclosed frontmatter");
  });

  it("throws when name is missing", () => {
    const noName = "---\ndescription: test\n---\nbody";
    expect(() => parseSkillMd(noName, "test.md")).toThrow("name");
  });

  it("throws when description is missing", () => {
    const noDesc = "---\nname: test\n---\nbody";
    expect(() => parseSkillMd(noDesc, "test.md")).toThrow("description");
  });

  it("handles minimal valid skill file", () => {
    const minimal = "---\nname: min\ndescription: minimal\n---\n";
    const skill = parseSkillMd(minimal, "min.md");
    expect(skill.meta.name).toBe("min");
    expect(skill.meta.description).toBe("minimal");
    expect(skill.body).toBe("");
  });

  it("preserves multi-line body content", () => {
    const multiLine = "---\nname: ml\ndescription: multi\n---\n\nLine 1\nLine 2\nLine 3";
    const skill = parseSkillMd(multiLine, "ml.md");
    expect(skill.body).toContain("Line 1");
    expect(skill.body).toContain("Line 2");
    expect(skill.body).toContain("Line 3");
  });
});

// ---------------------------------------------------------------------------
// renderSkillMd — round-trip serialization
// ---------------------------------------------------------------------------

describe("renderSkillMd", () => {
  it("renders a complete skill file", () => {
    const rendered = renderSkillMd({
      path: "test.md",
      meta: {
        name: "test-skill",
        description: "A test skill",
        version: "1.0.0",
        tools: ["bash", "read_file"],
        tags: ["testing"],
        examples: ["run tests", "fix bugs"],
      },
      body: "# System Prompt\n\nYou are a test agent.",
    });

    expect(rendered).toContain("---");
    expect(rendered).toContain("name: test-skill");
    expect(rendered).toContain("description: A test skill");
    expect(rendered).toContain("version: 1.0.0");
    expect(rendered).toContain("tools: [bash, read_file]");
    expect(rendered).toContain("tags: [testing]");
    expect(rendered).toContain("examples:");
    expect(rendered).toContain("  - run tests");
    expect(rendered).toContain("# System Prompt");
  });

  it("omits optional fields when not present", () => {
    const rendered = renderSkillMd({
      path: "min.md",
      meta: {
        name: "min",
        description: "minimal",
      },
      body: "",
    });

    expect(rendered).toContain("name: min");
    expect(rendered).not.toContain("version:");
    expect(rendered).not.toContain("tools:");
    expect(rendered).not.toContain("tags:");
    expect(rendered).not.toContain("examples:");
  });

  it("round-trips through parse and render", () => {
    const original = [
      "---",
      "name: round-trip",
      "description: Test round-trip",
      "version: 2.0.0",
      "tools: [bash]",
      "tags: [test]",
      "examples:",
      "  - example prompt",
      "---",
      "",
      "Body content here.",
    ].join("\n");

    const parsed = parseSkillMd(original, "rt.md");
    const rendered = renderSkillMd(parsed);

    // Re-parse the rendered output
    const reparsed = parseSkillMd(rendered, "rt2.md");
    expect(reparsed.meta.name).toBe(parsed.meta.name);
    expect(reparsed.meta.description).toBe(parsed.meta.description);
    expect(reparsed.meta.version).toBe(parsed.meta.version);
    expect(reparsed.meta.tools).toEqual(parsed.meta.tools);
    expect(reparsed.meta.tags).toEqual(parsed.meta.tags);
    expect(reparsed.meta.examples).toEqual(parsed.meta.examples);
  });
});
