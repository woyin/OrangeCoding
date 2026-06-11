/**
 * Tool approval system for interactive user confirmation of dangerous operations.
 *
 * When a tool's checkPermissions() returns PermissionDecision.Ask,
 * the ToolExecutor delegates to an ApprovalHandler to get user confirmation
 * before proceeding with the tool execution.
 */

// ---------------------------------------------------------------------------
// ApprovalRequest
// ---------------------------------------------------------------------------

/** Describes a tool call that requires user approval. */
export interface ApprovalRequest {
  /** Unique identifier for this approval request. */
  requestId: string;
  /** Name of the tool being called. */
  toolName: string;
  /** Arguments passed to the tool. */
  toolArguments: unknown;
  /** Human-readable reason why approval is needed. */
  reason: string;
}

// ---------------------------------------------------------------------------
// ApprovalResult
// ---------------------------------------------------------------------------

/** The user's decision on an approval request. */
export interface ApprovalResult {
  /** Whether the tool call was approved. */
  approved: boolean;
  /** Optional reason for the decision. */
  reason?: string;
}

// ---------------------------------------------------------------------------
// ApprovalHandler interface
// ---------------------------------------------------------------------------

/**
 * ApprovalHandler is the interface for getting user approval for tool calls.
 *
 * Implementations include:
 * - CLIApprovalHandler: prompts the user on stdin
 * - WebSocketApprovalHandler: sends approval request via WebSocket
 * - AutoApproveHandler: always approves (for testing or auto-approve mode)
 * - AutoDenyHandler: always denies (for testing)
 */
export interface ApprovalHandler {
  /**
   * Request approval for a tool call.
   * Returns a promise that resolves with the user's decision.
   * The promise should resolve in a timely manner (implementations
   * may apply their own timeouts).
   */
  requestApproval(request: ApprovalRequest): Promise<ApprovalResult>;
}

// ---------------------------------------------------------------------------
// Built-in handlers
// ---------------------------------------------------------------------------

/** Always approves every request. */
export class AutoApproveHandler implements ApprovalHandler {
  async requestApproval(_request: ApprovalRequest): Promise<ApprovalResult> {
    return { approved: true };
  }
}

/** Always denies every request. */
export class AutoDenyHandler implements ApprovalHandler {
  async requestApproval(_request: ApprovalRequest): Promise<ApprovalResult> {
    return { approved: false, reason: "auto-deny policy" };
  }
}

/**
 * CLIApprovalHandler prompts the user on stdin for approval.
 * Shows the tool name, arguments, and reason, then waits for y/n input.
 */
export class CLIApprovalHandler implements ApprovalHandler {
  async requestApproval(request: ApprovalRequest): Promise<ApprovalResult> {
    const argsStr = typeof request.toolArguments === "string"
      ? request.toolArguments
      : JSON.stringify(request.toolArguments, null, 2);

    console.log(`\n\x1b[33m⚠️  Approval Required\x1b[0m`);
    console.log(`  Tool: \x1b[36m${request.toolName}\x1b[0m`);
    console.log(`  Args: ${argsStr}`);
    if (request.reason) {
      console.log(`  Reason: ${request.reason}`);
    }

    const readline = await import("node:readline");
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    });

    return new Promise<ApprovalResult>((resolve) => {
      rl.question(`\n  Approve? [\x1b[32my\x1b[0m/n] `, (answer) => {
        rl.close();
        const trimmed = answer.trim().toLowerCase();
        if (trimmed === "y" || trimmed === "yes" || trimmed === "") {
          resolve({ approved: true, reason: "user approved" });
        } else {
          resolve({ approved: false, reason: "user denied" });
        }
      });
    });
  }
}
