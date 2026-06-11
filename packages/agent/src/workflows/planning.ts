/**
 * PlanningWorkflow — OmO-style interview-based planning workflow.
 *
 * Three-phase planning:
 *   1. Research phase: explore codebase to gather context
 *   2. Interview phase: ask clarifying questions (if needed)
 *   3. Planning phase: produce structured plan
 *
 * The workflow can be configured to skip phases based on task clarity.
 * Ported from modules/agent/workflows/planning.go.
 */

import type { AiProvider } from "@orangecoding/ai";
import { SessionId, AgentId } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import { ToolExecutor, filteredRegistry } from "../executor.js";
import { buildToolDefinitions } from "../tool-defs.js";
import { AgentLoop, defaultLoopConfig } from "../loop.js";
import {
  needsInterview,
  interviewPrompt,
  planPrompt,
  researchPrompt,
  type InterviewMode,
  type PrometheusConfig,
} from "../agents/prometheus.js";

// ---------------------------------------------------------------------------
// PlanResult
// ---------------------------------------------------------------------------

export interface PlanResult {
  /** Parsed steps from the plan */
  steps: string[];
  /** Raw plan text */
  rawPlan: string;
  /** Research summary (if research phase ran) */
  researchSummary: string;
  /** Interview questions and answers (if interview phase ran) */
  interview: { questions: string[]; answers: string[]; summary: string };
  /** Total planning duration */
  durationMs: number;
  /** Which phases were executed */
  phasesExecuted: ("research" | "interview" | "plan")[];
}

// ---------------------------------------------------------------------------
// PlanningWorkflowConfig
// ---------------------------------------------------------------------------

export interface PlanningWorkflowConfig {
  /** Interview mode: auto, always, never (default: auto) */
  interviewMode: InterviewMode;
  /** Max interview questions (default: 3) */
  maxInterviewQuestions: number;
  /** Enable research phase (default: true) */
  enableResearch: boolean;
  /** Custom answers for interview questions (for programmatic use) */
  interviewAnswers?: string[];
}

// ---------------------------------------------------------------------------
// PlanningWorkflow
// ---------------------------------------------------------------------------

export class PlanningWorkflow {
  private _provider: AiProvider;
  private _registry: ToolRegistry;
  private _workDir: string;
  private _config: PlanningWorkflowConfig;

  constructor(
    provider: AiProvider,
    registry: ToolRegistry,
    workDir: string,
    config?: Partial<PlanningWorkflowConfig>,
  ) {
    this._provider = provider;
    this._registry = registry;
    this._workDir = workDir;
    this._config = {
      interviewMode: config?.interviewMode ?? "auto",
      maxInterviewQuestions: config?.maxInterviewQuestions ?? 3,
      enableResearch: config?.enableResearch ?? true,
    };
  }

  /** Run executes the full planning workflow and returns a structured plan. */
  async run(signal: AbortSignal | undefined, task: string): Promise<PlanResult> {
    const start = Date.now();
    const phasesExecuted: PlanResult["phasesExecuted"] = [];
    let researchSummary = "";
    const interviewResult = { questions: [] as string[], answers: [] as string[], summary: "" };

    // Phase 1: Research
    if (this._config.enableResearch) {
      researchSummary = await this.runResearchPhase(signal, task);
      phasesExecuted.push("research");
    }

    // Phase 2: Interview (if needed)
    const shouldInterview = needsInterview(task, {
      interviewMode: this._config.interviewMode,
      maxInterviewQuestions: this._config.maxInterviewQuestions,
      enableResearch: this._config.enableResearch,
    });

    if (shouldInterview) {
      const interviewData = await this.runInterviewPhase(signal, task, researchSummary);
      interviewResult.questions = interviewData.questions;
      interviewResult.answers = interviewData.answers;
      interviewResult.summary = interviewData.summary;
      phasesExecuted.push("interview");
    }

    // Phase 3: Planning
    const planText = await this.runPlanningPhase(signal, task, researchSummary, interviewResult);
    phasesExecuted.push("plan");

    const steps = parseSteps(planText);
    return {
      steps,
      rawPlan: planText,
      researchSummary,
      interview: interviewResult,
      durationMs: Date.now() - start,
      phasesExecuted,
    };
  }

  /** Run a simple plan without interview or research phases. */
  async runSimple(signal: AbortSignal | undefined, task: string): Promise<PlanResult> {
    const start = Date.now();

    const sid = SessionId.create();
    const agentCtx = new AgentContext(sid, this._workDir);
    agentCtx.setSystemPrompt(
      "You are a planning agent. Decompose the given task into a numbered list of clear, actionable steps. Output only the steps, one per line.",
    );

    const allowedTools = ["read_file", "find", "grep", "glob"];
    const fRegistry = filteredRegistry(this._registry, allowedTools);
    const executor = new ToolExecutor(fRegistry);
    const toolDefs = buildToolDefinitions(fRegistry);

    const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, defaultLoopConfig(), toolDefs);
    agentCtx.addUserMessage(task);

    await loop.run({}, null);

    const lastAssistant = agentCtx.conversation.lastAssistantMessage();
    if (!lastAssistant) {
      throw new Error("planning workflow: no assistant response");
    }

    const steps = parseSteps(lastAssistant.content);
    return {
      steps,
      rawPlan: lastAssistant.content,
      researchSummary: "",
      interview: { questions: [], answers: [], summary: "" },
      durationMs: Date.now() - start,
      phasesExecuted: ["plan"],
    };
  }

  // -----------------------------------------------------------------------
  // Private: Phase runners
  // -----------------------------------------------------------------------

  private async runResearchPhase(signal: AbortSignal | undefined, task: string): Promise<string> {
    const sid = SessionId.create();
    const agentCtx = new AgentContext(sid, this._workDir);
    agentCtx.setSystemPrompt(researchPrompt());

    const allowedTools = ["read_file", "find", "grep", "glob"];
    const fRegistry = filteredRegistry(this._registry, allowedTools);
    const executor = new ToolExecutor(fRegistry);
    const toolDefs = buildToolDefinitions(fRegistry);

    const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, defaultLoopConfig(), toolDefs);
    agentCtx.addUserMessage(`Research the following task and produce a concise summary:\n\n${task}`);

    await loop.run({}, null);

    const lastAssistant = agentCtx.conversation.lastAssistantMessage();
    return lastAssistant?.content ?? "";
  }

  private async runInterviewPhase(
    signal: AbortSignal | undefined,
    task: string,
    _researchSummary: string,
  ): Promise<{ questions: string[]; answers: string[]; summary: string }> {
    // Generate interview questions in a single pass
    const sid = SessionId.create();
    const agentCtx = new AgentContext(sid, this._workDir);
    agentCtx.setSystemPrompt(interviewPrompt(this._config.maxInterviewQuestions));

    // Interview agent has no tools — it only asks questions
    const emptyRegistry = new ToolRegistry();
    const executor = new ToolExecutor(emptyRegistry);
    const toolDefs = buildToolDefinitions(emptyRegistry);

    const config = defaultLoopConfig();
    config.maxIterations = 1; // Single response for questions

    const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, config, toolDefs);

    const researchContext = _researchSummary
      ? `\n\nResearch context:\n${_researchSummary}`
      : "";

    agentCtx.addUserMessage(
      `Generate clarifying questions for this task (max ${this._config.maxInterviewQuestions}):\n\n${task}${researchContext}`,
    );

    await loop.run({}, null);

    const lastAssistant = agentCtx.conversation.lastAssistantMessage();
    const response = lastAssistant?.content ?? "";

    // Extract questions from the response
    const questions = extractQuestions(response);

    // If pre-provided answers exist, use them; otherwise mark as pending
    const answers = this._config.interviewAnswers
      ? this._config.interviewAnswers.slice(0, questions.length)
      : questions.map(() => "(pending user response)");

    // Extract summary if present
    const summaryMatch = response.match(/READY:\s*(.+?)(?:\n|$)/s);
    const summary = summaryMatch ? summaryMatch[1]!.trim() : "";

    return { questions, answers, summary };
  }

  private async runPlanningPhase(
    signal: AbortSignal | undefined,
    task: string,
    researchSummary: string,
    interview: { questions: string[]; answers: string[]; summary: string },
  ): Promise<string> {
    const sid = SessionId.create();
    const agentCtx = new AgentContext(sid, this._workDir);
    agentCtx.setSystemPrompt(planPrompt());

    const allowedTools = ["read_file", "find", "grep", "glob"];
    const fRegistry = filteredRegistry(this._registry, allowedTools);
    const executor = new ToolExecutor(fRegistry);
    const toolDefs = buildToolDefinitions(fRegistry);

    const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, defaultLoopConfig(), toolDefs);

    // Build the planning prompt with all gathered context
    const parts: string[] = [`## Task\n${task}`];

    if (researchSummary) {
      parts.push(`## Research Summary\n${researchSummary}`);
    }

    if (interview.questions.length > 0) {
      const interviewSection = interview.questions
        .map((q, i) => `Q${i + 1}: ${q}\nA${i + 1}: ${interview.answers[i] ?? "(no answer)"}`)
        .join("\n\n");
      parts.push(`## Interview\n${interviewSection}`);

      if (interview.summary) {
        parts.push(`## Understood Requirements\n${interview.summary}`);
      }
    }

    parts.push("## Instructions\nProduce a detailed execution plan based on the above context.");

    agentCtx.addUserMessage(parts.join("\n\n"));

    await loop.run({}, null);

    const lastAssistant = agentCtx.conversation.lastAssistantMessage();
    return lastAssistant?.content ?? "";
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Extract questions from interview response */
function extractQuestions(text: string): string[] {
  const questions: string[] = [];

  for (const line of text.split("\n")) {
    const trimmed = line.trim();

    // Match "QUESTION: ..." pattern
    const questionMatch = trimmed.match(/^QUESTION:\s*(.+)/i);
    if (questionMatch) {
      questions.push(questionMatch[1]!.trim());
      continue;
    }

    // Match numbered questions like "1. ..." or "1) ..."
    const numberedMatch = trimmed.match(/^\d+[\.\)]\s*(.+\?)$/);
    if (numberedMatch) {
      questions.push(numberedMatch[1]!.trim());
      continue;
    }

    // Match lines ending with "?"
    if (trimmed.endsWith("?") && trimmed.length > 5 && !trimmed.startsWith("#")) {
      questions.push(trimmed);
    }
  }

  return questions;
}

/** parseSteps extracts numbered steps from plan text. */
function parseSteps(text: string): string[] {
  const steps: string[] = [];
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    // Match lines like "1. step" or "- step"
    if (
      trimmed.length > 2 &&
      (trimmed[0] === "-" ||
        (trimmed.charCodeAt(0) >= 0x30 &&
          trimmed.charCodeAt(0) <= 0x39 &&
          trimmed.slice(0, 5).includes(".")))
    ) {
      // Strip leading number/bullet and whitespace
      const idx = trimmed.search(/[A-Za-z]/);
      if (idx > 0) {
        steps.push(trimmed.slice(idx));
      } else {
        steps.push(trimmed);
      }
    }
  }
  return steps;
}
