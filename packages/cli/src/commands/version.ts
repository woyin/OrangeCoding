import { VERSION } from "../version.js";

/**
 * Handles the `version` command.
 * Prints the current OrangeCoding version to stdout.
 */
export function runVersion(): void {
  console.log(`orangecoding v${VERSION}`);
}
