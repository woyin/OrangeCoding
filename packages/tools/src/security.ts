/**
 * PathValidator and SecurityPolicy for tool execution safety.
 *
 * Ported from modules/tools/security.go.
 */

import { execFileSync } from "node:child_process";
import { basename, resolve, normalize, sep } from "node:path";

// ---------------------------------------------------------------------------
// PathValidator
// ---------------------------------------------------------------------------

/**
 * Ensures that file paths are within allowed directories.
 */
export class PathValidator {
  readonly allowedDirs: string[];

  /**
   * Creates a PathValidator that restricts access to the given directories.
   * Each allowed directory is converted to an absolute path.
   */
  constructor(allowedDirs: string[]) {
    this.allowedDirs = allowedDirs.map((d) => resolve(d));
  }

  /**
   * Checks that the given path is within one of the allowed directories
   * and does not contain path traversal sequences.
   * @throws {Error} if the path is invalid or outside allowed directories.
   */
  validate(path: string): void {
    const abs = resolve(path);

    // Check for traversal sequences in the original path.
    if (path.includes("..")) {
      const cleaned = normalize(path);
      const absCleaned = resolve(cleaned);
      if (absCleaned !== abs) {
        throw new Error(`path traversal detected: "${path}" resolves outside allowed directories`);
      }
    }

    for (const dir of this.allowedDirs) {
      if (abs === dir || abs.startsWith(dir + sep)) {
        return;
      }
    }
    throw new Error(`path "${path}" is outside allowed directories`);
  }
}

// ---------------------------------------------------------------------------
// DefaultBlockedCommands
// ---------------------------------------------------------------------------

/** The default list of dangerous command names. */
export const DefaultBlockedCommands: string[] = [
  "rm", "rmdir", "mkfs", "dd", "format", "fdisk",
  "shutdown", "reboot", "halt", "poweroff",
  "kill", "killall", "pkill",
  "chmod", "chown", "chgrp",
  "iptables", "ip", "nft",
  "useradd", "userdel", "usermod", "groupadd", "groupdel",
  "mount", "umount",
  "curl", "wget", // use fetch tool instead
];

// ---------------------------------------------------------------------------
// SecurityPolicy
// ---------------------------------------------------------------------------

/**
 * Defines which commands are blocked from execution.
 */
export class SecurityPolicy {
  readonly blockedCommands: string[];
  private readonly _blockMap: Set<string>;

  /** Creates a SecurityPolicy that blocks the given command names. */
  constructor(blocked: string[]) {
    this.blockedCommands = blocked;
    this._blockMap = new Set(blocked);
  }

  /** Returns a SecurityPolicy with sensible defaults. */
  static default(): SecurityPolicy {
    return new SecurityPolicy(DefaultBlockedCommands);
  }

  /** Returns true if the command passes all security checks. */
  isAllowed(command: string): boolean {
    const cmd = command.trim();
    if (cmd === "") {
      return false;
    }

    // Block shell injection patterns.
    if (containsShellInjection(cmd)) {
      return false;
    }

    // Extract the effective command name.
    const effective = basename(extractCommand(cmd));

    if (this._blockMap.has(effective)) {
      return false;
    }
    return true;
  }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Extracts the first meaningful command token,
 * handling pipes, chains, subshells, and env var prefixes.
 */
function extractCommand(cmd: string): string {
  // Skip env assignments like FOO=bar cmd ...
  const parts = cmd.split(/\s+/);
  for (const p of parts) {
    if (!p.includes("=")) {
      return p;
    }
  }
  return cmd;
}

/** Detects common shell injection patterns. */
function containsShellInjection(cmd: string): boolean {
  const dangerous: string[] = [
    "$(",   // command substitution
    "`",    // backtick execution
    "${",   // variable expansion (could be injection)
    "|",    // pipe
    "&&",   // command chaining
    "||",   // command chaining
    ";",    // command separator
    "\n",   // newline (command separator)
  ];
  for (const d of dangerous) {
    if (cmd.includes(d)) {
      return true;
    }
  }
  // Check eval/exec as word-boundary matches to avoid false positives
  // with words like "evaluate", "execute", "retrieval".
  const words = cmd.split(/\s+/);
  for (const w of words) {
    const base = basename(w);
    if (base === "eval" || base === "exec") {
      return true;
    }
  }
  return false;
}

/**
 * Wraps `which` for use in tool implementations.
 * Returns the resolved path or throws.
 * Fixed: use execFileSync to prevent command injection.
 */
export function lookPath(name: string): string {
  try {
    const result = execFileSync("which", [name], { encoding: "utf-8" }).trim();
    return result;
  } catch {
    throw new Error(`${name}: command not found`);
  }
}
