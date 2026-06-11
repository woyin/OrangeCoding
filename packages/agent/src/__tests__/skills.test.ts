/**
 * Tests for SkillRegistry dynamic registration and plugin features.
 */

import { SkillRegistry } from "../skills.js";
import type { Skill } from "../skills.js";

describe("SkillRegistry", () => {
  let registry: SkillRegistry;

  beforeEach(() => {
    registry = new SkillRegistry();
  });

  test("has built-in skills registered", () => {
    const skills = registry.list();
    expect(skills.length).toBeGreaterThanOrEqual(8);
    expect(registry.has("code")).toBe(true);
    expect(registry.has("debug")).toBe(true);
    expect(registry.has("review")).toBe(true);
    expect(registry.has("plan")).toBe(true);
    expect(registry.has("explore")).toBe(true);
    expect(registry.has("refactor")).toBe(true);
    expect(registry.has("test")).toBe(true);
    expect(registry.has("document")).toBe(true);
  });

  test("registers and retrieves a custom skill", () => {
    const skill: Skill = {
      name: "custom",
      description: "Custom skill for testing",
      tools: ["bash"],
      prompt: "You are a custom agent.",
    };
    registry.register(skill);

    const [retrieved, ok] = registry.get("custom");
    expect(ok).toBe(true);
    expect(retrieved!.name).toBe("custom");
    expect(retrieved!.description).toBe("Custom skill for testing");
  });

  test("unregister removes a skill", () => {
    const skill: Skill = {
      name: "temp",
      description: "Temporary",
      tools: [],
      prompt: "Temp prompt",
    };
    registry.register(skill);
    expect(registry.has("temp")).toBe(true);

    const removed = registry.unregister("temp");
    expect(removed).toBe(true);
    expect(registry.has("temp")).toBe(false);
  });

  test("unregister returns false for non-existent skill", () => {
    expect(registry.unregister("nonexistent")).toBe(false);
  });

  test("listByTag returns skills matching a tag", () => {
    const codingSkills = registry.listByTag("coding");
    expect(codingSkills.length).toBeGreaterThan(0);
    expect(codingSkills.every((s) => s.tags?.includes("coding"))).toBe(true);
  });

  test("allTags returns unique sorted tags", () => {
    const tags = registry.allTags();
    expect(tags.length).toBeGreaterThan(0);
    // Check sorted
    for (let i = 1; i < tags.length; i++) {
      expect(tags[i]! >= tags[i - 1]!).toBe(true);
    }
    // Check unique
    expect(new Set(tags).size).toBe(tags.length);
  });

  test("compose merges multiple skills", () => {
    const composition = registry.compose(["code", "debug"]);
    expect(composition.skills.length).toBe(2);
    expect(composition.allTools.length).toBeGreaterThan(0);
    expect(composition.combinedPrompt).toContain("code implementation");
    expect(composition.combinedPrompt).toContain("debugging");
  });

  test("compose skips unknown skills", () => {
    const composition = registry.compose(["code", "nonexistent"]);
    expect(composition.skills.length).toBe(1);
    expect(composition.skills[0]!.name).toBe("code");
  });

  test("loadFromJSON loads skills from JSON string", () => {
    const json = JSON.stringify([
      {
        name: "json-skill-1",
        description: "Loaded from JSON",
        tools: ["bash", "read_file"],
        prompt: "You are loaded from JSON.",
      },
      {
        name: "json-skill-2",
        description: "Also from JSON",
        tools: [],
        prompt: "Another JSON skill.",
        tags: ["json", "test"],
      },
    ]);

    const count = registry.loadFromJSON(json);
    expect(count).toBe(2);
    expect(registry.has("json-skill-1")).toBe(true);
    expect(registry.has("json-skill-2")).toBe(true);
  });

  test("loadFromJSON handles invalid JSON gracefully", () => {
    const count = registry.loadFromJSON("not json");
    expect(count).toBe(0);
  });

  test("loadFromJSON handles non-array JSON gracefully", () => {
    const count = registry.loadFromJSON('{"name": "not array"}');
    expect(count).toBe(0);
  });

  test("loadFromObject validates required fields", () => {
    expect(registry.loadFromObject(null)).toBe(false);
    expect(registry.loadFromObject({})).toBe(false);
    expect(registry.loadFromObject({ name: "x" })).toBe(false);
    expect(registry.loadFromObject({
      name: "valid",
      description: "Valid skill",
      prompt: "Valid prompt",
    })).toBe(true);
    expect(registry.has("valid")).toBe(true);
  });

  test("registerMcpTools creates MCP skills", () => {
    const tools = [
      { name: "mcp_tool_1", description: "First MCP tool" },
      { name: "mcp_tool_2", description: "Second MCP tool" },
    ];

    const count = registry.registerMcpTools(tools);
    expect(count).toBe(2);
    expect(registry.has("mcp:mcp_tool_1")).toBe(true);
    expect(registry.has("mcp:mcp_tool_2")).toBe(true);

    const [skill, ok] = registry.get("mcp:mcp_tool_1");
    expect(ok).toBe(true);
    expect(skill!.tags).toContain("mcp");
    expect(skill!.tags).toContain("external");
  });

  test("createMcpSkill creates a properly formatted skill", () => {
    const skill = registry.createMcpSkill("my_tool", "does something useful");
    expect(skill.name).toBe("mcp:my_tool");
    expect(skill.tools).toContain("my_tool");
    expect(skill.prompt).toContain("my_tool");
    expect(skill.tags).toContain("mcp");
  });

  test("has OmO-style skills", () => {
    expect(registry.has("security")).toBe(true);
    expect(registry.has("perf")).toBe(true);
    expect(registry.has("deploy")).toBe(true);
  });
});
