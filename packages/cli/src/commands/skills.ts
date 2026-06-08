/**
 * Handles the `skills` command — list available skills.
 */

import { SkillRegistry, SkillMatcher } from "@orangecoding/agent";

export function runSkills(): void {
  const registry = new SkillRegistry();
  const matcher = new SkillMatcher();
  const skills = registry.list();

  if (skills.length === 0) {
    console.log("No skills available.");
    return;
  }

  console.log(`Available skills (${skills.length}):\n`);

  for (const skill of skills) {
    const tags = skill.tags?.length ? ` [${skill.tags.join(", ")}]` : "";
    console.log(`  ${skill.name}${tags}`);
    console.log(`    ${skill.description}`);
    console.log(`    Tools: ${skill.tools.join(", ")}`);
    if (skill.examples && skill.examples.length > 0) {
      console.log(`    Examples: ${skill.examples.slice(0, 3).join("; ")}`);
    }
    console.log();
  }

  console.log("Usage:");
  console.log("  orangecoding launch -p <task> --skill <name>");
  console.log("  orangecoding launch -p <task>               # auto-detect skill");
}
