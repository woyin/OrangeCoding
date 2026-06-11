/**
 * OpenAgent Framework — pluggable, discoverable multi-agent runtime.
 *
 * Design principles:
 *   - Agents are plugins: register capabilities, get discovered by role/skill
 *   - Dynamic routing: tasks auto-routed to best-fit agent via CollaborationRouter
 *   - Lifecycle management: pool-based acquire/release with health monitoring
 *   - Observable: event bus for agent lifecycle and task events
 *
 * Usage:
 *   const runtime = new OpenAgentRuntime(provider, toolRegistry);
 *   runtime.register(coderPlugin());
 *   runtime.register(plannerPlugin());
 *   await runtime.start();
 *   const result = await runtime.submit(task);
 */

import {
  AgentId,
  type AgentCapability,
  type AgentRole,
  type SessionId,
  type Task,
  type TaskResult,
  type TaskType,
} from "@orangecoding/core";
import type { AiProvider } from "@orangecoding/ai";
import type { ToolRegistry } from "@orangecoding/tools";
import { BaseAgent } from "./agents/base.js";
import type { ManagedAgent } from "@orangecoding/mesh";
import {
  AgentRegistry,
  AgentPool,
  HealthMonitor,
  DynamicCollaboration,
  MessageBus,
} from "@orangecoding/mesh";
import type { AgentPoolConfig, AgentFactory } from "@orangecoding/mesh";
import type { TaskClassifier } from "@orangecoding/mesh";
import { AgentLoop, defaultLoopConfig } from "./loop.js";
import { AgentContext } from "./context.js";
import { ToolExecutor } from "./executor.js";
import { buildToolDefinitions } from "./tool-defs.js";
import type { AgentLoopConfig } from "./loop.js";

// ---------------------------------------------------------------------------
// AgentPlugin — describes a pluggable agent definition
// ---------------------------------------------------------------------------

export interface AgentPlugin {
  name: string;
  description: string;
  role: AgentRole;
  capabilities: string[];
  allowedTools: string[];
  systemPrompt?: string;
  loopConfig?: Partial<AgentLoopConfig>;
}

// ---------------------------------------------------------------------------
// OpenAgentConfig
// ---------------------------------------------------------------------------

export interface OpenAgentConfig {
  maxPoolAgents?: number;
  poolIdleTimeoutMs?: number;
  healthCheckIntervalMs?: number;
  healthMissedThreshold?: number;
  healthMaxRestarts?: number;
  defaultLoopConfig?: Partial<AgentLoopConfig>;
}

// ---------------------------------------------------------------------------
// AgentEntry — internal tracking for a registered agent
// ---------------------------------------------------------------------------

interface AgentEntry {
  plugin: AgentPlugin;
  instance?: BaseAgent;
}

// ---------------------------------------------------------------------------
// Simple classifier — classifies by task.type field
// ---------------------------------------------------------------------------

class PluginTaskClassifier implements TaskClassifier {
  classify(task: Task): TaskType {
    return task.type;
  }
}

// ---------------------------------------------------------------------------
// OpenAgentRuntime
// ---------------------------------------------------------------------------

export class OpenAgentRuntime {
  private _registry: AgentRegistry;
  private _pool: AgentPool;
  private _healthMonitor: HealthMonitor;
  private _collaboration: DynamicCollaboration;
  private _bus: MessageBus;
  private _entries: Map<string, AgentEntry>;
  private _provider: AiProvider;
  private _toolRegistry: ToolRegistry;
  private _config: OpenAgentConfig;
  private _started = false;

  constructor(
    provider: AiProvider,
    toolRegistry: ToolRegistry,
    config?: OpenAgentConfig,
  ) {
    this._provider = provider;
    this._toolRegistry = toolRegistry;
    this._config = config ?? {};
    this._registry = new AgentRegistry();
    this._bus = new MessageBus();

    const poolConfig: AgentPoolConfig = {
      maxAgents: this._config.maxPoolAgents ?? 0,
      idleTimeoutMs: this._config.poolIdleTimeoutMs ?? 60000,
    };

    const factory: AgentFactory = async (
      _signal: AbortSignal | undefined,
      role: AgentRole,
      caps: string[],
    ) => {
      return this.createManagedAgent(role, caps);
    };

    this._pool = new AgentPool(poolConfig, factory);
    this._healthMonitor = new HealthMonitor({
      checkIntervalMs: this._config.healthCheckIntervalMs ?? 30000,
      missedThreshold: this._config.healthMissedThreshold ?? 3,
      maxRestarts: this._config.healthMaxRestarts ?? 3,
    });
    this._collaboration = new DynamicCollaboration(
      this._pool,
      this._registry,
      new PluginTaskClassifier(),
    );
    this._entries = new Map();
  }

  /**
   * Register an agent plugin. Returns a builder for further configuration.
   */
  register(plugin: AgentPlugin): AgentRegistrationBuilder {
    const entry: AgentEntry = { plugin };
    this._entries.set(plugin.name, entry);
    return new AgentRegistrationBuilder(this, plugin.name);
  }

  /**
   * Register an existing BaseAgent instance directly.
   */
  registerInstance(agent: BaseAgent, plugin: AgentPlugin): void {
    const entry: AgentEntry = { plugin, instance: agent };
    this._entries.set(plugin.name, entry);

    this._registry.register({
      id: agent.id(),
      role: agent.role(),
      capabilities: this.toAgentCapabilities(plugin.capabilities),
      status: agent.status(),
    });
  }

  /**
   * Start the runtime — initializes health monitoring and makes agents available.
   */
  async start(): Promise<void> {
    if (this._started) return;

    for (const entry of this._entries.values()) {
      if (!entry.instance) {
        entry.instance = this.createAgent(entry.plugin);
      }

      this._registry.register({
        id: entry.instance.id(),
        role: entry.instance.role(),
        capabilities: this.toAgentCapabilities(entry.plugin.capabilities),
        status: entry.instance.status(),
      });

      this._healthMonitor.start(entry.instance as unknown as ManagedAgent);
    }

    this._started = true;
    this._bus.publish("open-agent:started", { timestamp: Date.now() });
  }

  /**
   * Stop the runtime gracefully.
   */
  async stop(): Promise<void> {
    if (!this._started) return;

    for (const entry of this._entries.values()) {
      if (entry.instance) {
        await entry.instance.stop(undefined, "runtime shutdown");
      }
    }

    this._started = false;
    this._bus.publish("open-agent:stopped", { timestamp: Date.now() });
  }

  /**
   * Submit a task for execution. Routes through DynamicCollaboration.
   */
  async submit(task: Task): Promise<TaskResult> {
    if (!this._started) {
      throw new Error("OpenAgentRuntime not started. Call start() first.");
    }

    const results = await this._collaboration.route(task);

    const result = results.find((r) => r.taskId === task.id);
    if (!result) {
      return {
        taskId: task.id,
        status: "failed",
        output: "",
        error: new Error("no agent produced a result for this task"),
      };
    }
    return result;
  }

  /**
   * Submit a raw task string (auto-wraps in Task object).
   */
  async submitRaw(description: string, type: TaskType = "general"): Promise<TaskResult> {
    const task: Task = {
      id: "task-" + crypto.randomUUID().slice(0, 8),
      type,
      description,
      priority: 0,
      dependencies: [],
    };
    return this.submit(task);
  }

  /**
   * List all registered agent plugins.
   */
  listAgents(): AgentPlugin[] {
    return Array.from(this._entries.values()).map((e) => e.plugin);
  }

  /**
   * Get a specific agent by name.
   */
  getAgent(name: string): BaseAgent | undefined {
    return this._entries.get(name)?.instance;
  }

  get bus(): MessageBus {
    return this._bus;
  }

  get collaboration(): DynamicCollaboration {
    return this._collaboration;
  }

  get pool(): AgentPool {
    return this._pool;
  }

  get registry(): AgentRegistry {
    return this._registry;
  }

  get started(): boolean {
    return this._started;
  }

  /**
   * Get an internal entry by name. Used by AgentRegistrationBuilder.
   * @internal
   */
  getEntry(name: string): AgentEntry | undefined {
    return this._entries.get(name);
  }

  // --- Private ---

  private createAgent(plugin: AgentPlugin): BaseAgent {
    const id = AgentId.create();
    const executor = new ToolExecutor(this._toolRegistry);
    const workDir = process.cwd();
    const ctx = new AgentContext(
      { toString: () => "session-" + crypto.randomUUID().slice(0, 8) } as SessionId,
      workDir,
    );
    const toolDefs = buildToolDefinitions(executor.registry);
    const loopCfg: AgentLoopConfig = {
      ...defaultLoopConfig(),
      ...this._config.defaultLoopConfig,
      ...plugin.loopConfig,
    };

    const loop = new AgentLoop(id, this._provider, executor, ctx, loopCfg, toolDefs);
    return new BaseAgent(plugin.role, loop);
  }

  private createManagedAgent(role: AgentRole, caps: string[]): ManagedAgent {
    const plugin: AgentPlugin = {
      name: "dynamic-" + role,
      description: "Dynamic agent for role " + role,
      role,
      capabilities: caps,
      allowedTools: [],
    };
    return this.createAgent(plugin) as unknown as ManagedAgent;
  }

  private toAgentCapabilities(caps: string[]): AgentCapability[] {
    return caps.map((name) => ({
      name,
      description: name,
      supportedTools: [] as import("@orangecoding/core").ToolName[],
    }));
  }
}

// ---------------------------------------------------------------------------
// AgentRegistrationBuilder — fluent API for configuring an agent registration
// ---------------------------------------------------------------------------

export class AgentRegistrationBuilder {
  private _runtime: OpenAgentRuntime;
  private _name: string;

  constructor(runtime: OpenAgentRuntime, name: string) {
    this._runtime = runtime;
    this._name = name;
  }

  withTools(tools: string[]): AgentRegistrationBuilder {
    const entry = this._runtime.getEntry(this._name);
    if (entry) {
      entry.plugin = { ...entry.plugin, allowedTools: tools };
    }
    return this;
  }

  withSystemPrompt(prompt: string): AgentRegistrationBuilder {
    const entry = this._runtime.getEntry(this._name);
    if (entry) {
      entry.plugin = { ...entry.plugin, systemPrompt: prompt };
    }
    return this;
  }

  withLoopConfig(config: Partial<AgentLoopConfig>): AgentRegistrationBuilder {
    const entry = this._runtime.getEntry(this._name);
    if (entry) {
      entry.plugin = { ...entry.plugin, loopConfig: config };
    }
    return this;
  }

  withCapability(capability: string): AgentRegistrationBuilder {
    const entry = this._runtime.getEntry(this._name);
    if (entry) {
      entry.plugin = {
        ...entry.plugin,
        capabilities: [...entry.plugin.capabilities, capability],
      };
    }
    return this;
  }

  onEvent(handler: (data: unknown) => void): AgentRegistrationBuilder {
    this._runtime.bus.subscribe("agent:" + this._name, handler);
    return this;
  }
}

// ---------------------------------------------------------------------------
// Preset plugins — convenient factory functions for common agent types
// ---------------------------------------------------------------------------

export function coderPlugin(): AgentPlugin {
  return {
    name: "coder",
    description: "General-purpose coding agent",
    role: "coder",
    capabilities: ["code", "debug", "refactor", "test"],
    allowedTools: ["bash", "read_file", "write_file", "edit_file", "find", "grep", "glob"],
  };
}

export function plannerPlugin(): AgentPlugin {
  return {
    name: "planner",
    description: "Task decomposition and planning agent",
    role: "planner",
    capabilities: ["plan", "decompose", "prioritize"],
    allowedTools: ["read_file", "find", "grep", "glob"],
  };
}

export function reviewerPlugin(): AgentPlugin {
  return {
    name: "reviewer",
    description: "Code review and quality assessment agent",
    role: "reviewer",
    capabilities: ["review", "critique", "suggest"],
    allowedTools: ["read_file", "grep", "find"],
  };
}

export function explorerPlugin(): AgentPlugin {
  return {
    name: "explorer",
    description: "Codebase exploration and search agent",
    role: "observer",
    capabilities: ["search", "explore", "navigate"],
    allowedTools: ["read_file", "find", "grep", "glob"],
  };
}

export function executorPlugin(): AgentPlugin {
  return {
    name: "executor",
    description: "Plan execution agent",
    role: "executor",
    capabilities: ["execute", "run", "build", "test"],
    allowedTools: ["bash", "read_file", "write_file", "edit_file"],
  };
}
