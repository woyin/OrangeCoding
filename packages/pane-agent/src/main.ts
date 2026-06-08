#!/usr/bin/env node
/**
 * Command pane-agent runs an agent loop inside a terminal pane,
 * communicating with the parent process over a Unix domain socket.
 *
 * Usage: orangecoding-pane-agent --socket /path/to/socket.sock
 *
 * Ported from modules/pane-agent/main.go.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { parseArgs } from "node:util";

import { AgentId, SessionId, Role } from "@orangecoding/core";
import type { AgentEvent } from "@orangecoding/core";
import {
  type AiProvider,
  type ProviderConfig as AiProviderConfig,
  ProviderFactory,
  normalizeProviderConfig,
} from "@orangecoding/ai";
import type { ChatOptions } from "@orangecoding/ai";
import {
  AgentLoop,
  AgentContext,
  type AgentLoopConfig,
  ToolExecutor,
  ReasoningEffort,
  filteredRegistry,
  defaultLoopConfig,
  buildToolDefinitions,
} from "@orangecoding/agent";
import {
  type OrangeConfig,
  ConfigManager,
} from "@orangecoding/config";
import { createDefaultRegistry } from "@orangecoding/tools";
import {
  SocketTransport,
  connectSocket,
  IPCMessageType,
  type IPCMessage,
  type TaskPayload,
  type ResultPayload,
  type EventPayload,
} from "@orangecoding/multiplexer";

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

function main(): void {
  const { values } = parseArgs({
    options: {
      socket: { type: "string" },
      config: { type: "string" },
    },
    strict: true,
  });

  const socketPath = values.socket ?? "";
  const configPath = values.config ?? "";

  if (socketPath === "") {
    console.error("--socket is required");
    process.exit(1);
  }

  run(socketPath, configPath).catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`pane-agent: ${message}`);
    process.exit(1);
  });
}

// ---------------------------------------------------------------------------
// Main logic
// ---------------------------------------------------------------------------

async function run(socketPath: string, configPath: string): Promise<void> {
  // 1. Connect to the parent's Unix socket.
  const conn = await connectSocket(socketPath, 30_000);
  const transport = new SocketTransport(conn);

  try {
    // 2. Receive the task payload from the parent.
    const msg = await transport.receive();
    if (msg.type !== IPCMessageType.Task) {
      throw new Error(`expected task message, got "${msg.type}"`);
    }

    const task = msg.payload as TaskPayload;

    // 3. Set up the AI provider from config.
    const cfg = loadConfig(configPath);
    const { provider, providerModel } = createProvider(cfg);

    // 4. Create the agent loop.
    let registry = createDefaultRegistry();

    // Filter tools if specified.
    if (task.tools != null && task.tools.length > 0) {
      registry = filteredRegistry(registry, task.tools);
    }

    let loopConfig: AgentLoopConfig = defaultLoopConfig();
    if (cfg != null) {
      if (cfg.harness.reasoning_effort !== "") {
        loopConfig = {
          ...loopConfig,
          reasoning: {
            ...loopConfig.reasoning,
            effort: cfg.harness.reasoning_effort as ReasoningEffort,
          },
        };
      }
      if (cfg.harness.reasoning_budget_tokens > 0) {
        loopConfig = {
          ...loopConfig,
          reasoning: {
            ...loopConfig.reasoning,
            budgetTokens: cfg.harness.reasoning_budget_tokens,
          },
        };
      }
    }

    const agentId = AgentId.create();
    const sessionId = SessionId.create();
    const agentCtx = new AgentContext(sessionId, currentWorkDir());
    agentCtx.setSystemPrompt(
      "You are a coding agent running in a terminal pane. Complete the assigned task efficiently.",
    );
    agentCtx.addUserMessage(task.task);

    const loop = new AgentLoop(
      agentId,
      provider,
      new ToolExecutor(registry),
      agentCtx,
      loopConfig,
      buildToolDefinitions(registry),
    );

    // 5. Run the agent loop, streaming events to the parent.
    const eventQueue: AgentEvent[] = [];
    const eventCallbacks = {
      resolve: null as ((value: void) => void) | null,
    };

    let enqueueEvent: ((ev: AgentEvent) => void) = (ev: AgentEvent): void => {
      eventQueue.push(ev);
      if (eventCallbacks.resolve != null) {
        eventCallbacks.resolve();
        eventCallbacks.resolve = null;
      }
    };

    // Set up signal handling for graceful shutdown.
    const controller = new AbortController();
    process.on("SIGINT", () => controller.abort());
    process.on("SIGTERM", () => controller.abort());

    // Start event forwarding in the background.
    const eventForwarder = forwardEvents(
      transport,
      msg.id,
      eventQueue,
      () =>
        new Promise<void>((resolve) => {
          eventCallbacks.resolve = resolve;
        }),
    );

    try {
      await loop.run({ model: providerModel } as ChatOptions, enqueueEvent);
    } catch (err) {
      sendError(transport, msg.id, err);
      throw err;
    }

    // Signal no more events and wait for forwarder to drain.
    enqueueEvent = () => {}; // no-op: stop enqueuing
    if (eventCallbacks.resolve != null) {
      eventCallbacks.resolve();
      eventCallbacks.resolve = null;
    }
    await eventForwarder;

    // 6. Send the result back.
    const answer = lastAssistantContent(agentCtx);
    const resultPayload: ResultPayload = {
      success: true,
      content: answer,
    };
    await transport.send({
      type: IPCMessageType.Result,
      id: msg.id,
      payload: resultPayload,
    });
  } finally {
    await transport.close();
  }
}

// ---------------------------------------------------------------------------
// Event forwarding
// ---------------------------------------------------------------------------

/**
 * Reads events from the queue and forwards them as IPC messages.
 * Returns a promise that resolves when the queue is done and all events are sent.
 */
async function forwardEvents(
  transport: SocketTransport,
  msgId: string,
  queue: AgentEvent[],
  waitForNext: () => Promise<void>,
): Promise<void> {
  for (;;) {
    while (queue.length > 0) {
      const ev = queue.shift()!;
      const evtPayload: EventPayload = {
        eventType: ev.eventType,
        data: JSON.stringify(ev),
      };
      await transport.send({
        type: IPCMessageType.Event,
        id: msgId,
        payload: evtPayload,
      });
    }
    await waitForNext();
    // After waiting, check if more events arrived; if queue is still empty, we're done.
    if (queue.length === 0) {
      break;
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function sendError(transport: SocketTransport, id: string, err: unknown): void {
  const message = err instanceof Error ? err.message : String(err);
  const resultPayload: ResultPayload = {
    success: false,
    content: "",
    error: message,
  };
  // Fire-and-forget; do not await (may be called in catch blocks).
  void transport.send({
    type: IPCMessageType.Result,
    id,
    payload: resultPayload,
  });
}

function loadConfig(configPath: string | undefined): OrangeConfig | null {
  const p = configPath ?? defaultConfigPath();
  const mgr = new ConfigManager();
  try {
    return mgr.load(p);
  } catch {
    return null;
  }
}

function defaultConfigPath(): string {
  const home = os.homedir();
  return path.join(home, ".orangecoding", "config.json");
}

interface ProviderResult {
  provider: AiProvider;
  providerModel: string;
}

function createProvider(cfg: OrangeConfig | null): ProviderResult {
  if (cfg == null) {
    throw new Error("no config available");
  }

  let providerName = cfg.default_provider;
  if (providerName === "") {
    providerName = "openai";
  }

  let aiProviderConfig: AiProviderConfig = {
    apiKey: "",
    apiSecret: "",
    baseURL: "",
    defaultModel: "",
    timeoutSecs: 0,
    extra: {},
  };

  const p = cfg.providers[providerName];
  if (p != null) {
    aiProviderConfig = {
      apiKey: p.api_key,
      apiSecret: p.api_secret ?? "",
      baseURL: p.base_url ?? "",
      defaultModel: p.default_model ?? "",
      timeoutSecs: p.timeout_secs ?? 0,
      extra: p.extra ?? {},
    };
  }

  if (cfg.default_model !== "") {
    aiProviderConfig = { ...aiProviderConfig, defaultModel: cfg.default_model };
  }

  aiProviderConfig = normalizeProviderConfig(providerName, aiProviderConfig);

  const factory = new ProviderFactory();
  const provider = factory.createProvider(providerName, aiProviderConfig);

  return { provider, providerModel: aiProviderConfig.defaultModel };
}

function currentWorkDir(): string {
  try {
    return fs.realpathSync(process.cwd());
  } catch {
    return ".";
  }
}

function lastAssistantContent(ctx: AgentContext): string {
  const messages = ctx.conversation.messages();
  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i]!;
    if (msg.role === Role.Assistant) {
      return msg.content;
    }
  }
  return "";
}

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

main();
