/**
 * IntentGate — analyzes true user intent before classification or action.
 *
 * Inspired by oh-my-openagent's IntentGate. Runs before the agent loop
 * to extract the underlying intent from the user's prompt:
 *   - What category of task this is (quick/deep/visual/ultrabrain)
 *   - Whether the user wants planning first or direct execution
 *   - Whether parallel agents should be activated
 *   - Whether the task is a question, a request, or a directive
 */

import { ModelCategory } from "@orangecoding/ai";

// ---------------------------------------------------------------------------
// Legacy types (backward-compatible)
// ---------------------------------------------------------------------------

export type IntentCategory = "coding" | "planning" | "review" | "question" | "explore" | "general";

const CODING_KEYWORDS = ["code", "file", "function", "implement", "debug", "fix", "write", "create", "refactor"];
const PLANNING_KEYWORDS = ["plan", "design", "architecture", "roadmap", "strategy"];
const REVIEW_KEYWORDS = ["review", "check", "inspect", "audit", "verify"];
const QUESTION_KEYWORDS = ["what", "how", "why", "when", "where", "who", "which"];
const EXPLORE_KEYWORDS = ["find", "search", "explore", "locate", "list", "show"];

function containsAny(input: string, keywords: string[]): boolean {
  for (const kw of keywords) {
    if (input.includes(kw)) return true;
  }
  return false;
}

// ---------------------------------------------------------------------------
// IntentAnalysis — rich intent extraction (OmO-style)
// ---------------------------------------------------------------------------

export interface IntentAnalysis {
  /** The detected task category for model routing */
  category: ModelCategory;
  /** The legacy intent category */
  intent: IntentCategory;
  /** Whether the user wants planning before execution */
  wantsPlanning: boolean;
  /** Whether to activate parallel agents (ultrawork) */
  wantsParallel: boolean;
  /** Whether this is a question (not a task) */
  isQuestion: boolean;
  /** Detected scope */
  scope: "file" | "module" | "project" | "unknown";
  /** Brief description of the detected intent */
  summary: string;
  /** Confidence level (0.0 - 1.0) */
  confidence: number;
}

// ---------------------------------------------------------------------------
// OmO-style keyword patterns
// ---------------------------------------------------------------------------

const QUICK_PATTERNS = [
  /\bfix\s+(typo|bug|error|issue)\b/i,
  /\brename\b/i,
  /\bupdate\s+(version|dep|readme)\b/i,
  /\badd\s+(comment|import|type|field)\b/i,
  /\bremove\s+(unused|dead|comment)\b/i,
  /\bformat\b/i, /\blint\b/i,
];

const DEEP_PATTERNS = [
  /\brefactor\b/i, /\barchitect\b/i, /\bdesign\b/i,
  /\bimplement\b/i, /\bbuild\b/i, /\bmigrat[ei]\b/i,
  /\boptimiz[ei]\b/i, /\bsecurity\b/i, /\baudit\b/i,
];

const VISUAL_PATTERNS = [
  /\b(ui|ux|frontend|css|style|layout|component|page|screen)\b/i,
  /\b(react|vue|svelte|angular|tailwind)\b/i,
  /\bresponsive\b/i, /\bdark\s*mode\b/i,
];

const ULTRABRAIN_PATTERNS = [
  /\b(concurrency|parallel|race\s*condition|deadlock)\b/i,
  /\b(distributed|consensus|raft|paxos)\b/i,
  /\b(crypto|encrypt|hash|sign)\b/i,
  /\b(complex|hard|difficult|tricky)\b/i,
];

const PLANNING_PATTERNS = [
  /\bplan\b/i, /\bdesign\s+first\b/i, /\bthink\s+about\b/i,
  /\bhow\s+(should|would|could)\s+(we|i)\b/i,
  /\bwhat\s+(is|are)\s+the\s+(best|options|approach)\b/i,
  /\bstrategy\b/i,
];

const PARALLEL_PATTERNS = [
  /\b(parallel|concurrent|simultaneously|at\s+once)\b/i,
  /\b(all|every|each)\s+(file|module|package)\b/i,
  /\bultrawork\b/i, /\bulw\b/i,
];

// ---------------------------------------------------------------------------
// IntentGate
// ---------------------------------------------------------------------------

export class IntentGate {
  /** Classify returns the legacy intent category for the given input string. */
  classify(input: string): IntentCategory {
    const lower = input.toLowerCase();

    if (containsAny(lower, CODING_KEYWORDS)) return "coding";
    if (containsAny(lower, PLANNING_KEYWORDS)) return "planning";
    if (containsAny(lower, REVIEW_KEYWORDS)) return "review";
    if (containsAny(lower, QUESTION_KEYWORDS)) return "question";
    if (containsAny(lower, EXPLORE_KEYWORDS)) return "explore";

    return "general";
  }

  /**
   * Analyze the user's prompt for full intent extraction.
   * Returns a rich IntentAnalysis with model category, planning intent,
   * parallel mode, scope detection, and confidence.
   */
  analyze(prompt: string): IntentAnalysis {
    const lower = prompt.toLowerCase().trim();
    const intent = this.classify(prompt);

    // Detect model category (OmO-style)
    let category = ModelCategory.General;
    let confidence = 0.3;

    for (const p of ULTRABRAIN_PATTERNS) {
      if (p.test(lower)) { category = ModelCategory.Ultrabrain; confidence = 0.8; break; }
    }
    if (confidence < 0.7) {
      for (const p of DEEP_PATTERNS) {
        if (p.test(lower)) { category = ModelCategory.Deep; confidence = 0.7; break; }
      }
    }
    if (confidence < 0.6) {
      for (const p of VISUAL_PATTERNS) {
        if (p.test(lower)) { category = ModelCategory.Visual; confidence = 0.7; break; }
      }
    }
    if (confidence < 0.5) {
      for (const p of QUICK_PATTERNS) {
        if (p.test(lower)) { category = ModelCategory.Quick; confidence = 0.7; break; }
      }
    }

    // Fallback: map legacy intent to category
    if (confidence < 0.5) {
      switch (intent) {
        case "coding": category = ModelCategory.Coding; confidence = 0.6; break;
        case "planning": category = ModelCategory.Planning; confidence = 0.6; break;
        case "review": category = ModelCategory.Review; confidence = 0.6; break;
        case "explore": category = ModelCategory.Explore; confidence = 0.6; break;
        case "question": category = ModelCategory.Answer; confidence = 0.5; break;
      }
    }

    const wantsPlanning = PLANNING_PATTERNS.some((p) => p.test(lower));
    const isQuestion = /^(what|how|why|when|where|who|which|can|could|would|should|is|are|do|does)\b/i.test(prompt.trim()) ||
      /\?$/.test(prompt.trim());
    const wantsParallel = PARALLEL_PATTERNS.some((p) => p.test(lower));

    let scope: IntentAnalysis["scope"] = "unknown";
    if (/\b(this\s+file|single\s+file|one\s+file)\b/i.test(lower)) scope = "file";
    else if (/\b(this\s+module|this\s+package|this\s+component)\b/i.test(lower)) scope = "module";
    else if (/\b(entire|whole|all|every|project[- ]wide)\b/i.test(lower)) scope = "project";

    const parts: string[] = [`category=${category}`, `intent=${intent}`];
    if (wantsPlanning) parts.push("planning-first");
    if (isQuestion) parts.push("question");
    if (wantsParallel) parts.push("parallel");
    if (scope !== "unknown") parts.push(`scope=${scope}`);

    return {
      category,
      intent,
      wantsPlanning,
      wantsParallel,
      isQuestion,
      scope,
      summary: parts.join(", "),
      confidence,
    };
  }
}

/**
 * Convenience: analyze a prompt and return the suggested model category.
 */
export function suggestCategory(analysis: IntentAnalysis): ModelCategory {
  return analysis.category;
}
