/**
 * Core types for the skill-discovery package.
 */

// ---------------------------------------------------------------------------
// Skill File
// ---------------------------------------------------------------------------

export interface SkillFileMeta {
  /** Short kebab-case skill name */
  name: string;
  /** One-line description of what this skill does */
  description: string;
  /** Semantic version (optional) */
  version?: string;
  /** Tool names this skill requires */
  tools?: string[];
  /** Tags for categorization */
  tags?: string[];
  /** Example prompts that would match this skill */
  examples?: string[];
}

export interface SkillFile {
  /** Absolute path to the source file */
  path: string;
  /** Parsed frontmatter metadata */
  meta: SkillFileMeta;
  /** Markdown body (serves as system prompt) */
  body: string;
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export interface SkillDiscoveryConfig {
  /** Directories to search for SKILL.md files */
  searchDirs: string[];
  /** Glob pattern for matching files (default: "&#42;&#42;/&#42;.md") */
  pattern: string;
}

export const DEFAULT_DISCOVERY_CONFIG: Required<SkillDiscoveryConfig> = {
  searchDirs: [".claude/skills", "skills"],
  pattern: "**/*.md",
};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export class SkillParseError extends Error {
  constructor(
    message: string,
    public readonly filePath: string,
    public readonly line?: number
  ) {
    super(message);
    this.name = "SkillParseError";
  }
}

export function newSkillParseError(filePath: string, message: string, line?: number): SkillParseError {
  return new SkillParseError(message, filePath, line);
}
