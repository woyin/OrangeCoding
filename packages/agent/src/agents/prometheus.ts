/**
 * Prometheus - planning agent with OmO-style interview mode.
 *
 * Enhanced with interactive interview planning:
 *   - Phase 1: Interview — ask clarifying questions to refine the task
 *   - Phase 2: Research — read files to understand the codebase
 *   - Phase 3: Plan — produce a structured, actionable plan
 *
 * The interview mode is controlled by the `interviewMode` config:
 *   - "auto" (default): interview if the task is ambiguous
 *   - "always": always interview before planning
 *   - "never": skip interview, go straight to planning
 */

import type { AiProvider } from "@orangecoding/ai";
import { AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./base.js";
import { newFilteredAgent } from "./helpers.js";

// ---------------------------------------------------------------------------
// PrometheusConfig
// ---------------------------------------------------------------------------

export type InterviewMode = "auto" | "always" | "never";

export interface PrometheusConfig {
  /** Interview mode: auto (default), always, or never */
  interviewMode: InterviewMode;
  /** Max questions to ask during interview (default: 3) */
  maxInterviewQuestions: number;
  /** Include research phase to read codebase (default: true) */
  enableResearch: boolean;
}

const DEFAULT_PROMETHEUS_CONFIG: PrometheusConfig = {
  interviewMode: "auto",
  maxInterviewQuestions: 3,
  enableResearch: true,
};

// ---------------------------------------------------------------------------
// Interview question detection
// ---------------------------------------------------------------------------

/** Patterns that suggest the task is ambiguous and would benefit from interview */
const AMBIGUITY_PATTERNS = [
  /\b(some|certain|various|multiple)\b/i,
  /\b(refactor|improve|enhance|optimize)\b.*\b(code|system|architecture)\b/i,
  /\b(add|implement|build|create)\b.*\b(feature|support|capability)\b/i,
  /\b(fix|handle|resolve)\b.*\b(issue|problem|bug)\b/i,
  /\b(migrate|upgrade|port)\b/i,
];

/** Patterns that suggest the task is clear enough to plan directly */
const CLARITY_PATTERNS = [
  /\b(add|create|implement)\s+(a\s+)?(function|method|class|component|endpoint|route|test)\s+(called|named|for)\b/i,
  /\bfix\s+(typo|lint|import|type)\s+(error|issue)\b/i,
  /\b(update|change|rename)\s+(the\s+)?(variable|function|file|class|method)\b/i,
  /\b(remove|delete)\s+(unused|dead|deprecated)\s+(code|import|file)\b/i,
];

/**
 * Determine whether a task needs an interview phase.
 * Returns true if the task appears ambiguous.
 */
export function needsInterview(task: string, config: PrometheusConfig): boolean {
  if (config.interviewMode === "always") return true;
  if (config.interviewMode === "never") return false;

  // auto mode: check clarity vs ambiguity
  const isClear = CLARITY_PATTERNS.some((p) => p.test(task));
  if (isClear) return false;

  const isAmbiguous = AMBIGUITY_PATTERNS.some((p) => p.test(task));
  if (isAmbiguous) return true;

  // Default: short tasks (< 20 words) are usually clear, longer ones need interview
  const wordCount = task.split(/\s+/).length;
  return wordCount > 20;
}

// ---------------------------------------------------------------------------
// System prompts
// ---------------------------------------------------------------------------

const RESEARCH_PROMPT = `You are Prometheus, the research agent. Your job is to understand the codebase before planning.

Analyze the task and explore relevant files to gather context. Use read_file, find, grep, and glob tools to understand:
- The current code structure and relevant files
- Existing patterns and conventions
- Dependencies and imports
- Test coverage for the affected area

Produce a concise research summary covering:
1. Relevant files and their purposes
2. Current patterns and conventions
3. Potential risks or constraints
4. Dependencies that may be affected`;

const INTERVIEW_PROMPT = `You are Prometheus, the planning interviewer. Your job is to clarify the task before planning.

Ask focused clarifying questions (max {maxQuestions}) to understand:
- The exact scope and requirements
- Expected behavior vs current behavior
- Constraints (performance, compatibility, style)
- Priority of sub-tasks

Rules:
- Ask one question at a time
- Keep questions specific and actionable
- Don't ask about things you can verify by reading the code
- After {maxQuestions} questions, proceed to planning

Output your questions in this format:
QUESTION: <your question here>

After receiving answers, output:
READY: <brief summary of understood requirements>`;

const PLAN_PROMPT = `You are Prometheus, the planner. You create clear, actionable execution plans.

Based on the research and interview context, produce a structured plan with:
1. Numbered steps (each a concrete action)
2. Files to modify and what changes to make
3. Dependencies between steps
4. Risk assessment for each step
5. Verification steps (how to test each change)

Plan format:
## Plan: <title>

### Phase 1: <phase name>
1. <step description>
   - File: <path>
   - Action: <what to do>
   - Verify: <how to check>

### Phase 2: <phase name>
...

### Risks
- <risk>: <mitigation>

### Summary
<brief summary of the plan>`;

const DIRECT_PLAN_PROMPT = `You are Prometheus, the planner. You decompose tasks into clear, actionable plans.
You analyze requirements and create step-by-step execution strategies.`;

// ---------------------------------------------------------------------------
// Interview result
// ---------------------------------------------------------------------------

export interface InterviewResult {
  /** Questions asked during interview */
  questions: string[];
  /** Answers received */
  answers: string[];
  /** Summary of understood requirements */
  summary: string;
}

// ---------------------------------------------------------------------------
// Prometheus factory
// ---------------------------------------------------------------------------

/**
 * Create a Prometheus planning agent.
 * Uses read-only tools: read_file, find, grep, glob.
 */
export function newPrometheus(
  provider: AiProvider,
  registry: ToolRegistry,
  workDir: string,
): BaseAgent {
  return newFilteredAgent(
    provider,
    registry,
    workDir,
    AgentRole.Planner,
    ["read_file", "find", "grep", "glob"],
    DIRECT_PLAN_PROMPT,
  );
}

/**
 * Get the research phase system prompt.
 */
export function researchPrompt(): string {
  return RESEARCH_PROMPT;
}

/**
 * Get the interview phase system prompt with configured max questions.
 */
export function interviewPrompt(maxQuestions: number): string {
  return INTERVIEW_PROMPT.replace(/\{maxQuestions\}/g, String(maxQuestions));
}

/**
 * Get the planning phase system prompt.
 */
export function planPrompt(): string {
  return PLAN_PROMPT;
}

/**
 * Create a Prometheus agent configured for the research phase.
 */
export function newPrometheusResearch(
  provider: AiProvider,
  registry: ToolRegistry,
  workDir: string,
): BaseAgent {
  return newFilteredAgent(
    provider,
    registry,
    workDir,
    AgentRole.Planner,
    ["read_file", "find", "grep", "glob"],
    RESEARCH_PROMPT,
  );
}

/**
 * Create a Prometheus agent configured for the planning phase
 * (with full context from research and interview).
 */
export function newPrometheusPlanner(
  provider: AiProvider,
  registry: ToolRegistry,
  workDir: string,
): BaseAgent {
  return newFilteredAgent(
    provider,
    registry,
    workDir,
    AgentRole.Planner,
    ["read_file", "find", "grep", "glob"],
    PLAN_PROMPT,
  );
}
