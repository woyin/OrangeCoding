/**
 * Comment Checker — filters AI-sounding comments from code output.
 *
 * Inspired by oh-my-openagent's Comment Checker, this guardrail ensures
 * that code modifications produced by the agent don't contain excessive
 * or obviously AI-generated comments that would make the code look
 * machine-written.
 *
 * The checker runs as a post-tool guardrail on edit_file and write_file
 * tool outputs, scanning the new_string/content for problematic patterns.
 */

// ---------------------------------------------------------------------------
// Patterns that indicate AI-generated comments
// ---------------------------------------------------------------------------

const AI_COMMENT_PATTERNS: Array<{ pattern: RegExp; reason: string }> = [
  // Overly enthusiastic or explanatory
  { pattern: /\/\/\s*(This\s+)?(function|method|class|interface)\s+(is\s+used\s+to|does|handles|provides)/i,
    reason: "overly explanatory comment" },
  { pattern: /\/\/\s*(Note|Important|Key|Main)\s*:/i,
    reason: "AI-style note prefix" },
  { pattern: /\/\/\s*(Here\s+we|Now\s+we|Let's|We\s+need\s+to)\b/i,
    reason: "AI-style narrative comment" },
  { pattern: /\/\/\s*(TODO|FIXME|HACK)\s*:\s*(implement|add|fix|update|change|modify|remove)\b/i,
    reason: "generic AI TODO" },

  // Step-by-step narration
  { pattern: /\/\/\s*Step\s+\d+[\s:]/i,
    reason: "step-by-step narration" },
  { pattern: /\/\/\s*(First|Second|Third|Finally|Next)\s*,?\s*(we|the|this|it)\b/i,
    reason: "sequential narration" },

  // Redundant type/structure comments
  { pattern: /\/\/\s*(Define|Declare|Create|Initialize|Set\s+up)\s+(a|the|our)\b/i,
    reason: "redundant declaration comment" },
  { pattern: /\/\/\s*(Return|Returns)\s+(the|a|an)\s+(result|value|output|response)\b/i,
    reason: "obvious return comment" },

  // Excessive section headers
  { pattern: /\/\/\s*={3,}/i,
    reason: "excessive section divider" },
  { pattern: /\/\/\s*-{3,}\s*(Section|Part|Module)\b/i,
    reason: "AI section header" },

  // "This ensures" pattern
  { pattern: /\/\/\s*This\s+(ensures|guarantees|prevents|avoids|makes\s+sure)\b/i,
    reason: "AI justification comment" },
];

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface CommentCheckerConfig {
  /** Maximum number of AI-sounding comments allowed per file (default: 2) */
  maxAiComments: number;
  /** Whether to strip offending comments or just warn (default: strip) */
  stripComments: boolean;
  /** Enable/disable the checker (default: true) */
  enabled: boolean;
}

const DEFAULT_CONFIG: CommentCheckerConfig = {
  maxAiComments: 2,
  stripComments: true,
  enabled: true,
};

// ---------------------------------------------------------------------------
// CommentChecker
// ---------------------------------------------------------------------------

export interface CommentCheckResult {
  /** Whether any AI comments were found */
  found: boolean;
  /** Number of AI comments found */
  count: number;
  /** The cleaned content (if stripComments is true) */
  cleanedContent: string;
  /** Descriptions of what was found/removed */
  issues: string[];
}

/**
 * Check code content for AI-sounding comments.
 * Returns the cleaned content with problematic comments removed or flagged.
 */
export function checkComments(
  content: string,
  config?: Partial<CommentCheckerConfig>,
): CommentCheckResult {
  const cfg = { ...DEFAULT_CONFIG, ...config };

  if (!cfg.enabled) {
    return { found: false, count: 0, cleanedContent: content, issues: [] };
  }

  const lines = content.split("\n");
  const issues: string[] = [];
  let cleanedLines = [...lines];
  let aiCount = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!.trim();

    // Only check comment lines
    if (!line.startsWith("//") && !line.startsWith("#") && !line.startsWith("*")) {
      continue;
    }

    for (const { pattern, reason } of AI_COMMENT_PATTERNS) {
      if (pattern.test(line)) {
        aiCount++;
        issues.push(`Line ${i + 1}: ${reason} — "${line.slice(0, 60)}"`);

        if (cfg.stripComments) {
          // Remove the comment but keep the line if it has code
          const commentMatch = cleanedLines[i]!.match(/^(\s*)(.*?)\s*(\/\/.*|\/\*.*\*\/)\s*$/);
          if (commentMatch && commentMatch[2]!.trim()) {
            // Line has code + comment — remove only the comment
            cleanedLines[i] = commentMatch[1]! + commentMatch[2]!;
          } else {
            // Pure comment line — remove entirely
            cleanedLines[i] = "";
          }
        }
        break; // Only match first pattern per line
      }
    }
  }

  // Clean up empty lines left by removed comments
  if (cfg.stripComments && issues.length > 0) {
    // Collapse 3+ consecutive empty lines into 2
    const result: string[] = [];
    let emptyCount = 0;
    for (const line of cleanedLines) {
      if (line.trim() === "") {
        emptyCount++;
        if (emptyCount <= 2) result.push(line);
      } else {
        emptyCount = 0;
        result.push(line);
      }
    }
    cleanedLines = result;
  }

  return {
    found: aiCount > 0,
    count: aiCount,
    cleanedContent: cleanedLines.join("\n"),
    issues,
  };
}

/**
 * Quick check: does this content have too many AI comments?
 * Returns true if the content is clean, false if it needs fixing.
 */
export function isContentClean(content: string, maxAllowed?: number): boolean {
  const max = maxAllowed ?? DEFAULT_CONFIG.maxAiComments;
  let count = 0;

  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("//") && !trimmed.startsWith("#")) continue;

    for (const { pattern } of AI_COMMENT_PATTERNS) {
      if (pattern.test(trimmed)) {
        count++;
        if (count > max) return false;
        break;
      }
    }
  }

  return true;
}
