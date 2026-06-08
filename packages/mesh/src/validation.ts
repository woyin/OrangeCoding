/**
 * @module validation
 * Output validation for tool results.
 */

import type { ToolResult } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// OutputValidator
// ---------------------------------------------------------------------------

/** Checks tool results for anomalies. */
export class OutputValidator {
  private maxSize: number;

  constructor(maxSize: number) {
    this.maxSize = maxSize;
  }

  /**
   * Validate checks a tool result and returns [valid, warnings].
   * @param result - The tool result to validate.
   * @returns A tuple of [isValid, warnings[]].
   */
  validate(result: ToolResult): [boolean, string[]] {
    const warnings: string[] = [];
    let valid = true;

    if (this.maxSize > 0 && result.content.length > this.maxSize) {
      warnings.push(`output size ${result.content.length} exceeds limit ${this.maxSize}`);
      valid = false;
    }

    if (result.isError) {
      warnings.push("tool returned error");
    }

    return [valid, warnings];
  }
}
