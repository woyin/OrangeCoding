import type { ToolRegistry } from "@orangecoding/tools";

// ---------------------------------------------------------------------------
// Skill
// ---------------------------------------------------------------------------

export interface Skill {
  name: string;
  description: string;
  tools: string[];
  prompt: string;
  /** Optional tags for categorization */
  tags?: string[];
  /** Optional examples of prompts this skill handles */
  examples?: string[];
}

// ---------------------------------------------------------------------------
// SkillContext — the resolved execution context for a skill
// ---------------------------------------------------------------------------

export interface SkillContext {
  /** System prompt to inject into the conversation */
  systemPrompt: string;
  /** Tool names this skill is allowed to use */
  allowedTools: string[];
  /** The originating skill */
  skill: Skill;
}

// ---------------------------------------------------------------------------
// SkillComposition — result of merging multiple skills
// ---------------------------------------------------------------------------

export interface SkillComposition {
  combinedPrompt: string;
  allTools: string[];
  skills: Skill[];
}

// ---------------------------------------------------------------------------
// Built-in skills
// ---------------------------------------------------------------------------

const BUILTIN_SKILLS: Skill[] = [
  {
    name: "code",
    description: "Code implementation and modification",
    tools: ["bash", "read_file", "write_file", "edit_file"],
    prompt: "You are a code implementation agent. Write, modify, and debug code efficiently. Follow best practices and write clean, maintainable code.",
    tags: ["implementation", "coding"],
    examples: ["implement a user login system", "add error handling to this function", "create a REST API endpoint"],
  },
  {
    name: "debug",
    description: "Debugging and error diagnosis",
    tools: ["bash", "read_file", "grep"],
    prompt: "You are a debugging agent. Diagnose and fix errors systematically. Reproduce the issue, identify root cause, and apply minimal targeted fixes.",
    tags: ["debugging", "troubleshooting"],
    examples: ["fix this segmentation fault", "why is this test failing", "debug the login timeout issue"],
  },
  {
    name: "review",
    description: "Code review and quality analysis",
    tools: ["read_file", "grep", "find"],
    prompt: "You are a code review agent. Analyze code quality, identify bugs, security issues, and suggest improvements. Be specific and actionable.",
    tags: ["review", "quality", "security"],
    examples: ["review this pull request", "check for security vulnerabilities", "analyze code quality"],
  },
  {
    name: "plan",
    description: "Task planning and decomposition",
    tools: ["read_file", "find", "grep"],
    prompt: "You are a planning agent. Break down complex tasks into actionable steps. Consider dependencies, risks, and alternatives.",
    tags: ["planning", "design"],
    examples: ["plan the migration to TypeScript", "design the authentication system", "create a roadmap for this feature"],
  },
  {
    name: "explore",
    description: "Codebase exploration and understanding",
    tools: ["read_file", "find", "grep", "glob"],
    prompt: "You are an exploration agent. Navigate and understand codebase structure. Answer questions about how the code works.",
    tags: ["exploration", "understanding"],
    examples: ["how does the agent loop work", "where is authentication handled", "explain the project structure"],
  },
  {
    name: "refactor",
    description: "Code refactoring and cleanup",
    tools: ["bash", "read_file", "write_file", "edit_file", "grep"],
    prompt: "You are a refactoring agent. Improve code structure while preserving behavior. Apply design patterns, reduce duplication, and improve naming.",
    tags: ["refactoring", "cleanup"],
    examples: ["refactor this module to use dependency injection", "clean up duplicate code", "extract this into a reusable function"],
  },
  {
    name: "test",
    description: "Test writing and test-driven development",
    tools: ["bash", "read_file", "write_file", "edit_file", "grep"],
    prompt: "You are a testing agent. Write comprehensive tests including unit, integration, and edge cases. Follow TDD practices when appropriate.",
    tags: ["testing", "quality"],
    examples: ["write tests for this module", "add test coverage for auth", "create integration tests"],
  },
  {
    name: "document",
    description: "Documentation generation",
    tools: ["read_file", "write_file", "grep"],
    prompt: "You are a documentation agent. Generate clear, concise documentation including API docs, READMEs, and inline comments where needed.",
    tags: ["documentation"],
    examples: ["document this API", "write a README for this module", "generate API reference"],
  },
];

// ---------------------------------------------------------------------------
// SkillRegistry
// ---------------------------------------------------------------------------

export class SkillRegistry {
  private _skills: Map<string, Skill>;

  constructor() {
    this._skills = new Map();
    this.registerBuiltins();
  }

  register(s: Skill): void {
    this._skills.set(s.name, s);
  }

  get(name: string): [Skill, true] | [undefined, false] {
    const s = this._skills.get(name);
    if (s !== undefined) return [s, true];
    return [undefined, false];
  }

  has(name: string): boolean {
    return this._skills.has(name);
  }

  list(): Skill[] {
    return Array.from(this._skills.values());
  }

  listByTag(tag: string): Skill[] {
    return this.list().filter((s) => s.tags?.includes(tag));
  }

  /**
   * Resolve a skill into an execution context.
   * If the skill specifies tools, filter the available tools.
   */
  resolveContext(skill: Skill, allTools: ToolRegistry): SkillContext {
    const available = skill.tools.length > 0
      ? skill.tools.filter((name) => allTools.get(name) !== undefined)
      : allTools.list().map((t) => t.name());

    return {
      systemPrompt: skill.prompt,
      allowedTools: available,
      skill,
    };
  }

  /**
   * Compose multiple skills into a combined execution context.
   * Merges prompts and tool lists.
   */
  compose(skillNames: string[]): SkillComposition {
    const skills: Skill[] = [];
    const promptParts: string[] = [];
    const toolSet = new Set<string>();

    for (const name of skillNames) {
      const [skill, ok] = this.get(name);
      if (!ok) continue;
      skills.push(skill);
      promptParts.push(skill.prompt);
      for (const tool of skill.tools) {
        toolSet.add(tool);
      }
    }

    return {
      combinedPrompt: promptParts.join("\n\n"),
      allTools: Array.from(toolSet),
      skills,
    };
  }

  private registerBuiltins(): void {
    for (const s of BUILTIN_SKILLS) {
      this.register(s);
    }
  }
}
