/**
 * SKILL.md parser — handles frontmatter + markdown body format.
 *
 * Format:
 * ```
 * ---
 * name: my-skill
 * description: What this skill does
 * version: 1.0.0
 * tools: [bash, read_file, write_file]
 * tags: [coding, automation]
 * examples:
 *   - example prompt 1
 *   - example prompt 2
 * ---
 *
 * # System Prompt
 *
 * You are a specialized agent that...
 * ```
 */

import type { SkillFileMeta, SkillFile } from "./types.js";
import { newSkillParseError } from "./types.js";

// ---------------------------------------------------------------------------
// Frontmatter Parsing
// ---------------------------------------------------------------------------

/**
 * Parse the YAML frontmatter from a SKILL.md file.
 */
export function parseSkillFrontmatter(raw: string): SkillFileMeta {
  const meta: SkillFileMeta = {
    name: "",
    description: "",
  };

  const lines = raw.split("\n");
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.length === 0) continue;

    if (trimmed.startsWith("name:")) {
      meta.name = trimmed.slice(5).trim();
    } else if (trimmed.startsWith("description:")) {
      meta.description = trimmed.slice(12).trim();
    } else if (trimmed.startsWith("version:")) {
      meta.version = trimmed.slice(8).trim();
    } else if (trimmed.startsWith("tools:")) {
      meta.tools = _parseList(trimmed.slice(6).trim());
    } else if (trimmed.startsWith("tags:")) {
      meta.tags = _parseList(trimmed.slice(5).trim());
    } else if (trimmed.startsWith("examples:")) {
      // Examples are handled separately — just mark as present
      meta.examples = meta.examples ?? [];
    } else if (trimmed.startsWith("- ") && meta.examples !== undefined) {
      meta.examples.push(trimmed.slice(2).trim());
    }
  }

  return meta;
}

/**
 * Parse a YAML list value: "[a, b, c]" or "a, b, c" or multi-line "- a\n- b".
 */
function _parseList(value: string): string[] {
  // Bracket notation: [a, b, c]
  if (value.startsWith("[") && value.endsWith("]")) {
    return value
      .slice(1, -1)
      .split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
  }

  // Comma-separated: a, b, c
  if (value.includes(",")) {
    return value
      .split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
  }

  // Single value or empty
  if (value.length > 0) {
    return [value];
  }

  return [];
}

// ---------------------------------------------------------------------------
// Full file Parsing
// ---------------------------------------------------------------------------

const FRONTMATTER_DELIMITER = "---";

/**
 * Parse a complete SKILL.md file into a SkillFile.
 */
export function parseSkillMd(content: string, filePath: string): SkillFile {
  const trimmed = content.trim();

  // Must start with frontmatter delimiter
  if (!trimmed.startsWith(FRONTMATTER_DELIMITER)) {
    throw newSkillParseError(filePath, "file must start with '---' frontmatter delimiter");
  }

  // Find the closing delimiter
  const firstDelimEnd = FRONTMATTER_DELIMITER.length;
  const secondDelimStart = trimmed.indexOf(FRONTMATTER_DELIMITER, firstDelimEnd);

  if (secondDelimStart === -1) {
    throw newSkillParseError(filePath, "unclosed frontmatter: missing closing '---'");
  }

  const frontmatterRaw = trimmed.slice(firstDelimEnd, secondDelimStart).trim();
  const body = trimmed.slice(secondDelimStart + FRONTMATTER_DELIMITER.length).trim();

  const meta = parseSkillFrontmatter(frontmatterRaw);

  if (!meta.name || meta.name.length === 0) {
    throw newSkillParseError(filePath, "skill 'name' is required in frontmatter");
  }

  if (!meta.description || meta.description.length === 0) {
    throw newSkillParseError(filePath, "skill 'description' is required in frontmatter");
  }

  return { path: filePath, meta, body };
}

/**
 * Render a SkillFile back to markdown format.
 */
export function renderSkillMd(skill: SkillFile): string {
  const lines: string[] = ["---"];

  lines.push(`name: ${skill.meta.name}`);
  lines.push(`description: ${skill.meta.description}`);

  if (skill.meta.version) {
    lines.push(`version: ${skill.meta.version}`);
  }

  if (skill.meta.tools && skill.meta.tools.length > 0) {
    lines.push(`tools: [${skill.meta.tools.join(", ")}]`);
  }

  if (skill.meta.tags && skill.meta.tags.length > 0) {
    lines.push(`tags: [${skill.meta.tags.join(", ")}]`);
  }

  if (skill.meta.examples && skill.meta.examples.length > 0) {
    lines.push("examples:");
    for (const ex of skill.meta.examples) {
      lines.push(`  - ${ex}`);
    }
  }

  lines.push("---");
  lines.push("");

  if (skill.body.length > 0) {
    lines.push(skill.body);
  }

  return lines.join("\n");
}
