/**
 * GoalEvaluator — checks whether a goal's condition is met.
 *
 * The evaluator uses a separate (typically cheaper) AI model to evaluate
 * the condition independently of the agent that did the work. This is the
 * "maker/checker" separation applied to stop conditions.
 */

import type { AiProvider, ChatMessage } from "@orangecoding/ai";
import type { EvaluationResult, EvaluationContext } from "./types.js";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export interface GoalEvaluatorConfig {
  /** AI provider to use for evaluation */
  provider?: AiProvider;
  /** Model override (e.g. "haiku" for cheap eval) */
  model?: string;
  /** Temperature for evaluation (default: 0.2 — low for consistency) */
  temperature?: number;
}

const DEFAULT_EVALUATION_TEMPERATURE = 0.2;

// ---------------------------------------------------------------------------
// System Prompt
// ---------------------------------------------------------------------------

const EVALUATOR_SYSTEM_PROMPT = `You are a goal evaluation agent. Your job is to determine whether a specified goal condition has been met.

You will receive:
1. The GOAL CONDITION — what "done" looks like
2. Context about what has been tried and what was produced

Respond with a structured evaluation:
- Is the condition satisfied? (yes/no)
- How confident are you? (0-1)
- What still needs to be done if it's not satisfied?
- What suggestions do you have for what to try next?

Be strict and honest. If there's any doubt about whether the condition is met, report the doubt.`;

// ---------------------------------------------------------------------------
// Evaluator Class
// ---------------------------------------------------------------------------

export class GoalEvaluator {
  private readonly _provider: AiProvider | null;
  private readonly _model: string | undefined;
  private readonly _temperature: number;

  constructor(config?: GoalEvaluatorConfig) {
    this._provider = config?.provider ?? null;
    this._model = config?.model;
    this._temperature = config?.temperature ?? DEFAULT_EVALUATION_TEMPERATURE;
  }

  /**
   * Evaluate a goal condition against current context.
   *
   * @param condition - the goal condition (natural language)
   * @param context - current evaluation context
   * @returns structured evaluation result
   */
  async evaluate(condition: string, context: EvaluationContext): Promise<EvaluationResult> {
    if (!this._provider) {
      return this._fallbackEvaluate(condition, context);
    }

    const messages: ChatMessage[] = [
      { role: "system", content: EVALUATOR_SYSTEM_PROMPT },
      {
        role: "user",
        content: this._buildPrompt(condition, context),
      },
    ];

    const response = await this._provider.chatCompletion(messages, [], {
      model: this._model ?? "",
      temperature: this._temperature,
    });

    return this._parseResponse(response.content ?? "");
  }

  /**
   * Fallback evaluation without an AI provider.
   * Uses simple heuristics: looks for completion keywords in recent output.
   */
  private _fallbackEvaluate(condition: string, context: EvaluationContext): EvaluationResult {
    const output = context.recentOutput.toLowerCase();
    const remains = context.iteration > 1 ? condition.toLowerCase().split(/\s+/).filter((w) => w.length > 3) : [];

    // Simple heuristic: if iteration is high and output contains success keywords
    const hasFailureIndicators = /\b(fail|error|timeout|exception|crash)\b/i.test(output);
    const hasSuccessIndicators = /\b(pass|success|complete|done|clean)\b/i.test(output);

    if (hasFailureIndicators && !hasSuccessIndicators) {
      return {
        completed: false,
        confidence: 0.5,
        reason: "Output contains error/failure indicators",
        remainingBlockers: ["Clear errors in recent output"],
        suggestions: ["Review and fix reported errors"],
      };
    }

    if (hasSuccessIndicators && context.iteration >= 3) {
      return {
        completed: true,
        confidence: 0.6,
        reason: "Output contains success indicators and sufficient iterations completed",
        remainingBlockers: [],
        suggestions: [],
      };
    }

    return {
      completed: false,
      confidence: 0.3,
      reason: "Insufficient evidence for completion",
      remainingBlockers: ["Goal condition not yet verifiable"],
      suggestions: ["Continue iterating toward the goal condition"],
    };
  }

  /**
   * Build the evaluation prompt from condition and context.
   */
  private _buildPrompt(condition: string, context: EvaluationContext): string {
    let prompt = `## Goal Condition\n${condition}\n\n`;
    prompt += `## Context\n- Iteration: ${context.iteration}\n`;

    if (context.testResults) {
      prompt += `\n## Test Results\n${context.testResults}\n`;
    }
    if (context.lintOutput) {
      prompt += `\n## Lint Output\n${context.lintOutput}\n`;
    }

    prompt += `\n## Recent Agent Output\n${context.recentOutput.slice(0, 4000)}\n`;

    return prompt;
  }

  /**
   * Parse the AI response into a structured evaluation result.
   */
  private _parseResponse(content: string): EvaluationResult {
    const lower = content.toLowerCase();

    // Determine completion
    const completed = /condition.*(?:is|has been|was)\s+met/i.test(lower) ||
      /(?:yes|true|done),?\s+(?:the|it)\s+(?:condition|goal)/i.test(lower);

    // Extract confidence (look for "0.X" or "0.X%" patterns)
    const confidenceMatch = content.match(/([01]\.\d+)/);
    const confidence = confidenceMatch ? parseFloat(confidenceMatch[1] ?? "0.5") : 0.5;

    // Extract reason (look after "reason:" or "explanation:")
    const reasonMatch = content.match(/(?:reason|explanation|assessment):\s*(.+?)(?:\n|$)/i);
    const reason = reasonMatch
      ? reasonMatch[1]?.trim() ?? ""
      : content.slice(0, 200).trim();

    // Extract blockers (look for bullet points after "blocker" or "remaining")
    const blockMatch = content.match(/(?:remaining\s+blockers?|what.*left|still.*need):?\s*\n([\s\S]+?)(?:\n\n|$)/i);
    const remainingBlockers: string[] = [];

    if (blockMatch) {
      const blockContent = blockMatch[1] ?? "";
      for (const line of blockContent.split("\n")) {
        const trimmed = line.replace(/^[-\*\d+.]\s*/, "").trim();
        if (trimmed.length > 0) {
          remainingBlockers.push(trimmed);
        }
      }
    }

    // Extract suggestions (look for bullet points after "suggestion")
    const suggMatch = content.match(/(?:suggestions?|recommend|next steps?):?\s*\n([\s\S]+?)(?:\n\n|$)/i);
    const suggestions: string[] = [];

    if (suggMatch) {
      const suggContent = suggMatch[1] ?? "";
      for (const line of suggContent.split("\n")) {
        const trimmed = line.replace(/^[-\*\d+.]\s*/, "").trim();
        if (trimmed.length > 0) {
          suggestions.push(trimmed);
        }
      }
    }

    return {
      completed,
      confidence,
      reason: reason || "Parsed evaluation response",
      remainingBlockers,
      suggestions,
    };
  }

  /**
   * Create a string summary of an evaluation result (for logging/reporting).
   */
  static summarize(result: EvaluationResult): string {
    const lines: string[] = [
      `Completed: ${result.completed}`,
      `Confidence: ${result.confidence.toFixed(2)}`,
      `Reason: ${result.reason}`,
    ];

    if (result.remainingBlockers.length > 0) {
      lines.push(`Blockers: ${result.remainingBlockers.join("; ")}`);
    }

    if (result.suggestions.length > 0) {
      lines.push(`Suggestions: ${result.suggestions.join("; ")}`);
    }

    return lines.join("\n");
  }
}
