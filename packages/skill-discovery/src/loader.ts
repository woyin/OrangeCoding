/**
 * Skill loader — converts SkillFile to Skill and loads into SkillRegistry.
 *
 * Bridges @orangecoding/skill-discovery with @orangecoding/agent SkillRegistry.
 */

import type { SkillFile } from "./types.js";
import type { Skill, SkillContext } from "@orangecoding/agent";

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/**
 * Convert a SkillFile to a Skill object compatible with SkillRegistry.
 */
export function toSkill(skillFile: SkillFile): Skill {
  return {
    name: skillFile.meta.name,
    description: skillFile.meta.description,
    tools: skillFile.meta.tools ?? [],
    prompt: skillFile.body,
    tags: skillFile.meta.tags,
    examples: skillFile.meta.examples,
  };
}

/**
 * Convert a SkillFile to a SkillContext with resolved tools.
 *
 * @param skillFile - the parsed skill file
 * @param allToolNames - all available tool names
 */
export function toSkillContext(skillFile: SkillFile, allToolNames: string[]): SkillContext {
  const skill = toSkill(skillFile);
  const allowedTools = skill.tools.length > 0
    ? allToolNames.filter((t) => skill.tools.includes(t))
    : allToolNames;

  return {
    systemPrompt: skill.prompt,
    allowedTools,
    skill,
  };
}

// ---------------------------------------------------------------------------
// Registry Integration
// ---------------------------------------------------------------------------

/**
 * Interface for a minimal skill registry.
 * Matches the public register() method of SkillRegistry.
 */
export interface SkillRegistryLike {
  register(skill: Skill): void;
}

/**
 * Load all skill files from a directory into a skill registry.
 *
 * @param discoverer - skill discoverer instance
 * @param registry - skill registry to populate
 */
export async function loadDiscoveredSkills(
  discoverer: { discover(): Promise<SkillFile[]> },
  registry: SkillRegistryLike
): Promise<number> {
  const files = await discoverer.discover();
  let loaded = 0;

  for (const file of files) {
    try {
      const skill = toSkill(file);
      registry.register(skill);
      loaded++;
    } catch {
      // Skip invalid skill files
    }
  }

  return loaded;
}
