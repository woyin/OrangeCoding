/**
 * Handles the `serve` command.
 * Starts the OrangeCoding control server for managing agent sessions.
 *
 * When a provider with valid credentials is configured, each new session
 * receives a fully-wired AgentLoop (AI provider + tools + context).
 * When no provider is available, sessions use null-loop mode (task
 * recording only — useful for testing the plumbing).
 *
 * Agent events (streaming text, tool calls, guardrail decisions) are
 * bridged to the control server's WebSocket broadcast channel so
 * connected clients receive real-time updates.
 *
 * Tool approval requests are routed through WebSocket (ApprovalRequestEvent)
 * and resolved via HTTP POST /sessions/:id/approve.
 */

import { ConfigManager } from "@orangecoding/config";
import type { OrangeConfig } from "@orangecoding/config";
import type { AgentEvent } from "@orangecoding/core";
import {
  StreamChunkEvent,
  CompletedEvent,
  ToolCallRequestedEvent,
  ToolCallCompletedEvent,
  GuardrailDecisionEvent,
  ErrorEvent as CoreErrorEvent,
} from "@orangecoding/core";
import {
  AgentStreamEvent,
  AgentCompletedEvent,
  ToolCallEvent,
  GuardrailEvent,
  ErrorEvent as ProtocolErrorEvent,
} from "@orangecoding/control-protocol";
import type { ServerEvent } from "@orangecoding/control-protocol";
import { AuditEventRecorder, AuditLog } from "@orangecoding/audit";
import { WorkerRuntime } from "@orangecoding/worker";
import { Server, ServerApprovalHandler } from "@orangecoding/control-server";
import type { WorkerRuntime as ControlWorkerRuntime } from "@orangecoding/control-server";
import { SessionBuilder } from "./session-builder.js";
import { defaultConfigPath } from "./init.js";

export interface ServeRuntime {
  workerRuntime: WorkerRuntime;
  server: Server;
  auditLog?: AuditLog;
  agentEventHandler?: (event: AgentEvent) => Promise<void>;
  sessionBuilder?: SessionBuilder;
  approvalHandler?: ServerApprovalHandler;
}

/**
 * Run the serve command.
 */
export async function runServe(addr?: string): Promise<void> {
  const configPath = defaultConfigPath();
  const mgr = new ConfigManager();
  const cfg = mgr.load(configPath);
  const bindAddr = addr || `:${cfg.control_port}`;

  const runtime = await createServeRuntime(cfg, bindAddr);
  await runtime.server.start();

  console.log(`OrangeCoding control server listening on ${bindAddr}`);
  console.log("Press Ctrl+C to stop.");

  await waitForSignal();
  console.log("\nShutting down...");
  runtime.approvalHandler?.cancelAll();
  await runtime.server.stop();
}

/**
 * Create the serve runtime with all components wired together.
 */
export async function createServeRuntime(cfg: OrangeConfig, addr?: string): Promise<ServeRuntime> {
  let auditLog: AuditLog | undefined;
  let auditHandler: ((event: AgentEvent) => Promise<void>) | undefined;

  if (cfg.audit.enabled) {
    auditLog = await AuditLog.create(cfg.audit.dir);
    const recorder = new AuditEventRecorder(auditLog);
    auditHandler = recorder.handle.bind(recorder);
  }

  let sessionBuilder: SessionBuilder | undefined;
  if (hasProviderCredentials(cfg)) {
    sessionBuilder = new SessionBuilder(cfg);
  }

  // Mutable broadcast function — set after the Server is created
  let broadcastFn: ((event: ServerEvent) => void) | null = null;

  // Agent event handler: audit + WebSocket bridge
  const agentEventHandler = async (event: AgentEvent): Promise<void> => {
    if (auditHandler) {
      await auditHandler(event);
    }
    const serverEvent = bridgeAgentToServerEvent(event);
    if (serverEvent && broadcastFn) {
      broadcastFn(serverEvent);
    }
  };

  // Server event handler: forward to broadcast
  const serverEventHandler = (event: ServerEvent): void => {
    if (broadcastFn) {
      broadcastFn(event);
    }
  };

  // Approval handler: uses the mutable broadcast function
  const approvalHandler = new ServerApprovalHandler(
    (event: ServerEvent) => { if (broadcastFn) broadcastFn(event); },
  );

  const workerRuntime = new WorkerRuntime(serverEventHandler, agentEventHandler);

  const server = new Server({
    workers: adaptWorkerRuntime(workerRuntime, sessionBuilder, approvalHandler),
    addr: addr ?? `:${cfg.control_port}`,
    approvalHandler,
  });

  // Wire the broadcast function now that the server exists
  broadcastFn = (event: ServerEvent): void => server.broadcastEvent(event);

  return { workerRuntime, server, auditLog, agentEventHandler, sessionBuilder, approvalHandler };
}

/**
 * Bridge core AgentEvents to control-protocol ServerEvents for WebSocket broadcast.
 */
function bridgeAgentToServerEvent(event: AgentEvent): ServerEvent | null {
  const sid = event.sessionId.toString();

  if (event instanceof StreamChunkEvent) {
    return new AgentStreamEvent(sid, event.content);
  }
  if (event instanceof CompletedEvent) {
    return new AgentCompletedEvent(sid, event.summary, 0, 0, 0, "completed");
  }
  if (event instanceof ToolCallRequestedEvent) {
    const tc = event.toolCall;
    const args = typeof tc.arguments === "string" ? tc.arguments : JSON.stringify(tc.arguments);
    return new ToolCallEvent(sid, tc.function_name, args, "", false);
  }
  if (event instanceof ToolCallCompletedEvent) {
    return new ToolCallEvent(sid, event.toolName, "", "", !event.success);
  }
  if (event instanceof GuardrailDecisionEvent) {
    return new GuardrailEvent(sid, event.phase, event.decision, event.reason, event.guardrailName);
  }
  if (event instanceof CoreErrorEvent) {
    return new ProtocolErrorEvent(sid, event.errorMessage);
  }
  return null;
}

function adaptWorkerRuntime(
  runtime: WorkerRuntime,
  sessionBuilder?: SessionBuilder,
  approvalHandler?: ServerApprovalHandler,
): ControlWorkerRuntime {
  return {
    startSession(sessionId: string): void {
      let agentLoop = null;
      if (sessionBuilder) {
        try {
          agentLoop = sessionBuilder.build({ sessionId, approvalHandler: approvalHandler ?? undefined });
        } catch (err) {
          console.error(
            `[serve] failed to build agent loop for session ${sessionId}:`,
            err instanceof Error ? err.message : String(err),
          );
        }
      }
      runtime.startSession(sessionId, agentLoop);
    },
    stopSession(sessionId: string): void {
      runtime.stopSession(sessionId);
    },
    listSessions(): string[] {
      return runtime.listSessions();
    },
    getStatus(sessionId: string): [status: string, found: boolean] {
      const [status, found] = runtime.getStatus(sessionId);
      return [status ?? "", found];
    },
    submitTask(sessionId: string, task: string): void {
      runtime.submitTask(sessionId, task);
    },
    shutdown(): void {
      approvalHandler?.cancelAll();
      runtime.shutdown();
    },
  };
}

function hasProviderCredentials(cfg: OrangeConfig): boolean {
  const providerName = cfg.default_provider;
  const provider = cfg.providers[providerName];
  if (provider && provider.api_key) return true;
  if (process.env["OPENAI_API_KEY"] && providerName === "openai") return true;
  if (process.env["ANTHROPIC_API_KEY"] && (providerName === "anthropic" || providerName === "claude")) return true;
  return false;
}

export function waitForSignal(): Promise<void> {
  return new Promise((resolve) => {
    const cleanup = (): void => {
      process.off("SIGINT", onSignal);
      process.off("SIGTERM", onSignal);
    };
    const onSignal = (): void => {
      cleanup();
      resolve();
    };
    process.once("SIGINT", onSignal);
    process.once("SIGTERM", onSignal);
  });
}
