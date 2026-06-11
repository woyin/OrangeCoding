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
  // New OmO-style skills
  {
    name: "security",
    description: "Security audit and vulnerability scanning",
    tools: ["read_file", "grep", "find", "glob", "bash"],
    prompt: "You are a security audit agent. Analyze code for vulnerabilities, insecure patterns, and compliance issues. Provide specific remediation advice.",
    tags: ["security", "audit"],
    examples: ["audit this codebase for security issues", "check for SQL injection vulnerabilities", "review authentication security"],
  },
  {
    name: "perf",
    description: "Performance optimization and profiling",
    tools: ["bash", "read_file", "grep", "edit_file"],
    prompt: "You are a performance optimization agent. Identify bottlenecks, measure performance, and apply targeted optimizations. Profile before optimizing.",
    tags: ["performance", "optimization"],
    examples: ["optimize this slow function", "profile the API endpoint", "reduce memory usage"],
  },
  {
    name: "deploy",
    description: "Deployment and DevOps",
    tools: ["bash", "read_file", "write_file", "edit_file"],
    prompt: "You are a deployment agent. Handle CI/CD configuration, Dockerfiles, infrastructure as code, and deployment scripts.",
    tags: ["deployment", "devops"],
    examples: ["set up CI/CD pipeline", "create a Dockerfile", "configure deployment scripts"],
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

  /** Register a skill. Overwrites if a skill with the same name exists. */
  register(s: Skill): void {
    this._skills.set(s.name, s);
  }

  /** Unregister a skill by name. */
  unregister(name: string): boolean {
    return this._skills.delete(name);
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

  /** Return all unique tags across all registered skills. */
  allTags(): string[] {
    const tags = new Set<string>();
    for (const skill of this.list()) {
      if (skill.tags) {
        for (const tag of skill.tags) {
          tags.add(tag);
        }
      }
    }
    return Array.from(tags).sort();
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

  /**
   * Load skills from a JSON string (e.g., from a file).
   * The JSON should be an array of Skill objects.
   * Returns the number of skills loaded.
   */
  loadFromJSON(json: string): number {
    try {
      const skills = JSON.parse(json) as Skill[];
      if (!Array.isArray(skills)) return 0;

      let count = 0;
      for (const s of skills) {
        if (s.name && s.description && s.prompt) {
          if (!s.tools) s.tools = [];
          this.register(s);
          count++;
        }
      }
      return count;
    } catch {
      return 0;
    }
  }

  /**
   * Load a single skill from a JSON object.
   * Validates required fields before registering.
   */
  loadFromObject(obj: unknown): boolean {
    if (typeof obj !== "object" || obj === null) return false;
    const s = obj as Record<string, unknown>;
    if (typeof s.name !== "string" || typeof s.description !== "string" || typeof s.prompt !== "string") {
      return false;
    }
    this.register({
      name: s.name,
      description: s.description,
      tools: Array.isArray(s.tools) ? s.tools as string[] : [],
      prompt: s.prompt,
      tags: Array.isArray(s.tags) ? s.tags as string[] : undefined,
      examples: Array.isArray(s.examples) ? s.examples as string[] : undefined,
    });
    return true;
  }

  /**
   * Create a skill that wraps an MCP tool.
   * The skill allows using an MCP tool within the agent's skill system.
   */
  createMcpSkill(toolName: string, description: string): Skill {
    return {
      name: `mcp:${toolName}`,
      description: `MCP tool: ${description}`,
      tools: [toolName],
      prompt: `You are an agent with access to the "${toolName}" MCP tool. Use it to ${description}.`,
      tags: ["mcp", "external"],
    };
  }

  /**
   * Register multiple MCP tools as skills.
   * Each MCP tool becomes a skill with the "mcp:" prefix.
   */
  registerMcpTools(tools: Array<{ name: string; description?: string }>): number {
    let count = 0;
    for (const tool of tools) {
      const skill = this.createMcpSkill(tool.name, tool.description ?? tool.name);
      this.register(skill);
      count++;
    }
    return count;
  }

  private registerBuiltins(): void {
    for (const s of BUILTIN_SKILLS) {
      this.register(s);
    }
  }
}
