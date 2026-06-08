/**
 * Permission types for tool access control.
 *
 * Ported from modules/tools/permissions.go.
 */

// ---------------------------------------------------------------------------
// PermissionDecision
// ---------------------------------------------------------------------------

export enum PermissionDecision {
  /** Explicitly allowed. */
  Allow = 0,
  /** Explicitly denied. */
  Deny = 1,
  /** Ask the user. */
  Ask = 2,
  /** Auto-approved. */
  AutoApprove = 3,
  /** Allowed with conditions. */
  Conditional = 4,
}

// ---------------------------------------------------------------------------
// PermissionContext
// ---------------------------------------------------------------------------

/** Provides the information needed to make a permission decision. */
export interface PermissionContext {
  /** Current working directory. */
  workingDir: string;
  /** File being accessed (if applicable). */
  filePath?: string;
  /** Command being run (if applicable). */
  command?: string;
  /** Whether the operation is read-only. */
  isReadOnly: boolean;
}
