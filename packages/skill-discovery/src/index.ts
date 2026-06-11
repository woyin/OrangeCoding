/**
 * @orangecoding/skill-discovery — Skill file discovery and SKILL.md parsing.
 *
 * Re-exports all public API from the package.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
export { SkillParseError, newSkillParseError, DEFAULT_DISCOVERY_CONFIG } from "./types.js";
export type { SkillFileMeta, SkillFile, SkillDiscoveryConfig } from "./types.js";

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------
export { parseSkillMd, parseSkillFrontmatter, renderSkillMd } from "./parser.js";

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------
export { SkillDiscoverer } from "./discovery.js";

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------
export { toSkill, toSkillContext, loadDiscoveredSkills } from "./loader.js";
export type { SkillRegistryLike } from "./loader.js";
