/**
 * Tests for the skill loader module — conversion and registry integration.
 */

import { toSkill, toSkillContext, loadDiscoveredSkills } from "../loader.js";
import type { SkillFile } from "../types.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeSkillFile(overrides: Partial<SkillFile["meta"]> = {}): SkillFile {
  return {
    path: "/skills/test.md",
    meta: {
      name: "test-skill",
      description: "A test skill",
      tools: ["bash", "read_file"],
      tags: ["testing"],
      examples: ["run tests"],
      ...overrides,
    },
    body: "You are a test agent.",
  };
}

// ---------------------------------------------------------------------------
// toSkill
// ---------------------------------------------------------------------------

describe("toSkill", () => {
  it("converts a SkillFile to a Skill object", () => {
    const skill = toSkill(makeSkillFile());
    expect(skill.name).toBe("test-skill");
    expect(skill.description).toBe("A test skill");
    expect(skill.tools).toEqual(["bash", "read_file"]);
    expect(skill.prompt).toBe("You are a test agent.");
    expect(skill.tags).toEqual(["testing"]);
    expect(skill.examples).toEqual(["run tests"]);
  });

  it("handles missing optional fields", () => {
    const skillFile = makeSkillFile({ tools: undefined, tags: undefined, examples: undefined });
    const skill = toSkill(skillFile);
    expect(skill.tools).toEqual([]);
    expect(skill.tags).toBeUndefined();
    expect(skill.examples).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// toSkillContext
// ---------------------------------------------------------------------------

describe("toSkillContext", () => {
  it("creates a SkillContext with filtered tools", () => {
    const allTools = ["bash", "read_file", "write_file", "grep"];
    const skillFile = makeSkillFile({ tools: ["bash", "read_file"] });

    const ctx = toSkillContext(skillFile, allTools);
    expect(ctx.systemPrompt).toBe("You are a test agent.");
    expect(ctx.allowedTools).toEqual(["bash", "read_file"]);
    expect(ctx.skill).toBeDefined();
  });

  it("allows all tools when skill has no tool restriction", () => {
    const allTools = ["bash", "read_file", "write_file"];
    const skillFile = makeSkillFile({ tools: [] });

    const ctx = toSkillContext(skillFile, allTools);
    expect(ctx.allowedTools).toEqual(allTools);
  });

  it("filters tools against available tools", () => {
    const allTools = ["bash", "read_file"];
    const skillFile = makeSkillFile({ tools: ["bash", "grep", "web_search"] });

    const ctx = toSkillContext(skillFile, allTools);
    // Only "bash" is in both lists
    expect(ctx.allowedTools).toEqual(["bash"]);
  });
});

// ---------------------------------------------------------------------------
// loadDiscoveredSkills
// ---------------------------------------------------------------------------

describe("loadDiscoveredSkills", () => {
  it("loads skills into a registry", async () => {
    const discoverer = {
      discover: async () => [
        makeSkillFile({ name: "skill-a" }),
        makeSkillFile({ name: "skill-b" }),
      ],
    };

    const registered: any[] = [];
    const registry = {
      register: (skill: any) => registered.push(skill),
    };

    const count = await loadDiscoveredSkills(discoverer, registry);
    expect(count).toBe(2);
    expect(registered).toHaveLength(2);
    expect(registered[0].name).toBe("skill-a");
    expect(registered[1].name).toBe("skill-b");
  });

  it("returns 0 for empty discovery", async () => {
    const discoverer = { discover: async () => [] };
    const registry = { register: () => {} };

    const count = await loadDiscoveredSkills(discoverer, registry);
    expect(count).toBe(0);
  });

  it("skips invalid skills without crashing", async () => {
    const discoverer = {
      discover: async () => [
        makeSkillFile({ name: "valid" }),
        makeSkillFile({ name: "" }), // Invalid: empty name will cause toSkill issues
      ],
    };

    const registered: any[] = [];
    const registry = {
      register: (skill: any) => {
        if (!skill.name) throw new Error("invalid");
        registered.push(skill);
      },
    };

    const count = await loadDiscoveredSkills(discoverer, registry);
    // One valid, one failed (but caught)
    expect(count).toBe(1);
  });
});
