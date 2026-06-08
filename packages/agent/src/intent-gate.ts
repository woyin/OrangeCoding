/**
 * IntentGate classifies user input into an intent category using keyword matching.
 * Ported from modules/agent/intent_gate.go.
 */

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

export class IntentGate {
  /** Classify returns the intent category for the given input string. */
  classify(input: string): IntentCategory {
    const lower = input.toLowerCase();

    if (containsAny(lower, CODING_KEYWORDS)) return "coding";
    if (containsAny(lower, PLANNING_KEYWORDS)) return "planning";
    if (containsAny(lower, REVIEW_KEYWORDS)) return "review";
    if (containsAny(lower, QUESTION_KEYWORDS)) return "question";
    if (containsAny(lower, EXPLORE_KEYWORDS)) return "explore";

    return "general";
  }
}
