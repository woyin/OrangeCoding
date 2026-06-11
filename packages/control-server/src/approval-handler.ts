/**
 * ServerApprovalHandler bridges tool approval requests through the
 * control server's HTTP and WebSocket channels.
 *
 * Flow:
 * 1. Tool asks for approval (PermissionDecision.Ask)
 * 2. ServerApprovalHandler stores a pending request and broadcasts
 *    an ApprovalRequestEvent via WebSocket
 * 3. Client sends HTTP POST /sessions/:id/approve with { requestId, approved }
 * 4. ServerApprovalHandler resolves the pending promise
 */

import { randomUUID } from "node:crypto";
import {
  ApprovalRequestEvent,
  type ServerEvent,
} from "@orangecoding/control-protocol";
import type {
  ApprovalHandler,
  ApprovalRequest,
  ApprovalResult,
} from "@orangecoding/tools";

// ---------------------------------------------------------------------------
// Pending request state
// ---------------------------------------------------------------------------

interface PendingApproval {
  resolve: (result: ApprovalResult) => void;
  timeout: ReturnType<typeof setTimeout>;
}

// ---------------------------------------------------------------------------
// ServerApprovalHandler
// ---------------------------------------------------------------------------

export class ServerApprovalHandler implements ApprovalHandler {
  private readonly pending = new Map<string, PendingApproval>();
  private readonly timeoutMs: number;

  /**
   * @param broadcast - Function to broadcast a ServerEvent (e.g. server.broadcastEvent)
   * @param timeoutMs - Max time to wait for user response (default: 60s)
   */
  constructor(
    private readonly broadcast: (event: ServerEvent) => void,
    timeoutMs = 60_000,
  ) {
    this.timeoutMs = timeoutMs;
  }

  /**
   * Submit an approval request. Broadcasts via WebSocket and waits
   * for a response.
   */
  async requestApproval(request: ApprovalRequest): Promise<ApprovalResult> {
    const argsStr = typeof request.toolArguments === "string"
      ? request.toolArguments
      : JSON.stringify(request.toolArguments);

    // Broadcast the approval request to all connected clients
    this.broadcast(
      new ApprovalRequestEvent(
        request.requestId,
        request.toolName,
        argsStr,
        request.reason,
      ),
    );

    // Wait for the response
    return new Promise<ApprovalResult>((resolve) => {
      const timeout = setTimeout(() => {
        this.pending.delete(request.requestId);
        resolve({ approved: false, reason: "approval request timed out" });
      }, this.timeoutMs);

      this.pending.set(request.requestId, { resolve, timeout });
    });
  }

  /**
   * Respond to a pending approval request.
   * Called by the HTTP endpoint handler.
   * Returns true if the request was found and resolved.
   */
  respond(requestId: string, approved: boolean, reason?: string): boolean {
    const entry = this.pending.get(requestId);
    if (!entry) {
      return false;
    }

    clearTimeout(entry.timeout);
    this.pending.delete(requestId);
    entry.resolve({ approved, reason: reason ?? (approved ? "approved" : "denied") });
    return true;
  }

  /** Returns true if there are pending approval requests. */
  hasPending(): boolean {
    return this.pending.size > 0;
  }

  /** Cancel all pending requests (e.g. on shutdown). */
  cancelAll(): void {
    for (const [id, entry] of this.pending) {
      clearTimeout(entry.timeout);
      entry.resolve({ approved: false, reason: "server shutting down" });
    }
    this.pending.clear();
  }
}
