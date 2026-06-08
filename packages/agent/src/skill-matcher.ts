import type { Skill, SkillRegistry } from "./skills.js";

// ---------------------------------------------------------------------------
// SkillMatch — a scored match result
// ---------------------------------------------------------------------------

export interface SkillMatch {
  skill: Skill;
  score: number;
  matchedKeywords: string[];
}

// ---------------------------------------------------------------------------
// SkillMatcher
// ---------------------------------------------------------------------------

const INTENT_KEYWORDS: Record<string, string[]> = {
  code: ["implement", "write", "create", "build", "add", "code", "function", "class", "method", "module", "feature"],
  debug: ["debug", "fix", "error", "bug", "crash", "traceback", "stack trace", "panic", "fail", "broken", "issue"],
  review: ["review", "check", "audit", "quality", "improve", "optimize", "analyze", "security", "vulnerability"],
  plan: ["plan", "design", "architecture", "roadmap", "strategy", "decompose", "break down", "step", "approach"],
  explore: ["explore", "understand", "explain", "how", "what", "where", "find", "search", "navigate", "structure"],
  refactor: ["refactor", "restructure", "reorganize", "rename", "cleanup", "clean up", "simplify", "extract", "deduplicate"],
};

export class SkillMatcher {
  private _keywordMap: Map<string, string[]>;

  constructor() {
    this._keywordMap = new Map(Object.entries(INTENT_KEYWORDS));
  }

  /**
   * Match a user prompt against registered skills.
   * Returns matches sorted by score (highest first).
   */
  match(prompt: string, registry: SkillRegistry): SkillMatch[] {
    const lower = prompt.toLowerCase();
    const results: SkillMatch[] = [];

    for (const skill of registry.list()) {
      const keywords = this._keywordMap.get(skill.name) ?? this.extractKeywords(skill);
      const matched: string[] = [];
      let score = 0;

      for (const kw of keywords) {
        if (lower.includes(kw)) {
          matched.push(kw);
          score += kw.split(" ").length; // multi-word keywords score higher
        }
      }

      if (score > 0) {
        results.push({ skill, score, matchedKeywords: matched });
      }
    }

    results.sort((a, b) => b.score - a.score);
    return results;
  }

  /**
   * Find the best matching skill for a prompt.
   * Returns undefined if no skill matches.
   */
  bestMatch(prompt: string, registry: SkillRegistry): SkillMatch | undefined {
    const matches = this.match(prompt, registry);
    return matches.length > 0 ? matches[0] : undefined;
  }

  /**
   * Extract keywords from a skill's name and description for matching.
   */
  private extractKeywords(skill: Skill): string[] {
    const words = `${skill.name} ${skill.description}`.toLowerCase().split(/\s+/);
    return words.filter((w) => w.length >= 3);
  }
}
