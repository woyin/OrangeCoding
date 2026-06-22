/**
 * jsonc.ts — minimal JSONC (JSON with comments) preprocessor.
 *
 * The OrangeCoding config file allows // line comments and block comments
 * for documentation. JSON.parse cannot handle these, so parseJSONC strips
 * comments while respecting string literals, then returns clean JSON.
 */
import { newConfigError } from "@orangecoding/core";

/**
 * ParseJSONC strips // line comments and /* *​/ block comments from a JSONC
 * input string while preserving comments inside quoted strings. It returns
 * clean JSON suitable for JSON.parse.
 */
export function parseJSONC(input: string): string {
  const parts: string[] = [];
  let inString = false;
  let escape = false;
  let i = 0;
  const n = input.length;

  while (i < n) {
    const ch = input[i]!;

    if (inString) {
      parts.push(ch);
      if (escape) {
        escape = false;
      } else if (ch === "\\") {
        escape = true;
      } else if (ch === '"') {
        inString = false;
      }
      i++;
      continue;
    }

    // Outside strings
    if (ch === '"') {
      parts.push(ch);
      inString = true;
      i++;
    } else if (ch === "/" && i + 1 < n && input[i + 1] === "/") {
      // Line comment: skip to end of line
      const end = input.indexOf("\n", i);
      if (end === -1) {
        // Rest of input is a comment
        i = n;
      } else {
        i = end + 1; // skip past \n
      }
    } else if (ch === "/" && i + 1 < n && input[i + 1] === "*") {
      // Block comment: skip to *​/
      const end = input.indexOf("*/", i);
      if (end === -1) {
        throw newConfigError(
          `unterminated block comment starting at position ${i}`,
        );
      }
      i = end + 2; // skip past *​/
    } else {
      parts.push(ch);
      i++;
    }
  }

  if (inString) {
    throw newConfigError("unterminated string literal");
  }

  return parts.join("");
}
