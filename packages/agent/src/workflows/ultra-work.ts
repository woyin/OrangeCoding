/**
 * UltraWork — OmO-style multi-agent parallel execution.
 *
 * Activates all specialized agents in parallel to tackle the task:
 *   - Sisyphus (coding): writes, debugs, refactors code
 *   - Prometheus (planning): creates structured plans
 *   - Atlas (execution): executes plan steps
 *   - Explore (exploration): searches and locates code
 *   - Hephaestus (tool-building): creates and improves tools
 *   - Oracle (analysis): analyzes and provides insights
 *
 * The orchestrator collects results from all agents and synthesizes
 * a final result. Agents that fail do not block others.
 *
 * Modes:
 *   - "parallel": all agents run simultaneously, results synthesized
 *   - "pipeline": agents run in sequence, each building on previous results
 *   - "adaptive": agents selected based on IntentGate analysis
 */

import type { AiProvider } from "@orangecoding/ai";
import { SessionId, AgentId, AgentRole } from "@orangecoding/core";
import { ToolRegistry } from "@orangecoding/tools";
import { AgentContext } from "../context.js";
import { ToolExecutor, filteredRegistry } from "../executor.js";
import { buildToolDefinitions } from "../tool-defs.js";
import { AgentLoop, defaultLoopConfig, type AgentLoopResult } from "../loop.js";
import { IntentGate, type IntentAnalysis } from "../intent-gate.js";

// ---------------------------------------------------------------------------
// UltraWorkConfig
// ---------------------------------------------------------------------------

export type UltraWorkMode = "parallel" | "pipeline" | "adaptive";

export interface UltraWorkConfig {
  /** Execution mode (default: "parallel") */
  mode: UltraWorkMode;
  /** Maximum step budget per agent (default: 30) */
  stepBudgetPerAgent: number;
  /** Timeout per agent in ms (default: 300000 = 5 minutes) */
  timeoutPerAgentMs: number;
  /** Maximum number of parallel agents (default: 4) */
  maxConcurrentAgents: number;
  /** Auto-approve tool calls (default: true) */
  autoApproveTools: boolean;
}

const DEFAULT_ULTRA_CONFIG: UltraWorkConfig = {
  mode: "parallel",
  stepBudgetPerAgent: 30,
  timeoutPerAgentMs: 300_000,
  maxConcurrentAgents: 4,
  autoApproveTools: true,
};

// ---------------------------------------------------------------------------
// AgentSpec — lightweight agent specification for parallel dispatch
// ---------------------------------------------------------------------------

interface AgentSpec {
  name: string;
  role: AgentRole;
  allowedTools: string[];
  systemPrompt: string;
}

/** Pre-defined agent specifications matching OmO's agent roster */
const AGENT_SPECS: Record<string, AgentSpec> = {
  sisyphus: {
    name: "sisyphus",
    role: AgentRole.Coder,
    allowedTools: ["bash", "read_file", "write_file", "edit_file", "find", "grep", "glob"],
    systemPrompt: "You are Sisyphus, a general-purpose coding agent. Write, debug, review, and refactor code. Be thorough and methodical.",
  },
  prometheus: {
    name: "prometheus",
    role: AgentRole.Planner,
    allowedTools: ["read_file", "find", "grep", "glob"],
    systemPrompt: "You are Prometheus, the planner. Decompose the task into a clear, actionable plan. Analyze requirements and create step-by-step strategies.",
  },
  atlas: {
    name: "atlas",
    role: AgentRole.Executor,
    allowedTools: ["bash", "read_file", "write_file", "edit_file"],
    systemPrompt: "You are Atlas, the executor. Execute plan steps precisely and report progress.",
  },
  explore: {
    name: "explore",
    role: AgentRole.Explorer,
    allowedTools: ["read_file", "find", "grep", "glob", "list_directory"],
    systemPrompt: "You are the Explorer. Search the codebase to find relevant files, patterns, and context for the task. Provide a comprehensive summary of findings.",
  },
  hephaestus: {
    name: "hephaestus",
    role: AgentRole.Builder,
    allowedTools: ["bash", "read_file", "write_file", "edit_file", "find", "grep", "glob"],
    systemPrompt: "You are Hephaestus, the tool-builder. Focus on building, improving, and testing tools and utilities.",
  },
  oracle: {
    name: "oracle",
    role: AgentRole.Analyst,
    allowedTools: ["read_file", "find", "grep", "glob"],
    systemPrompt: "You are Oracle, the analyst. Analyze the task deeply, identify risks, edge cases, and provide expert insights.",
  },
};

// ---------------------------------------------------------------------------
// AgentResult — result from a single agent in the parallel pool
// ---------------------------------------------------------------------------

export interface AgentResult {
  /** Agent name */
  agent: string;
  /** Whether the agent completed successfully */
  success: boolean;
  /** The agent's output */
  output: string;
  /** Agent loop result (if available) */
  loopResult?: AgentLoopResult;
  /** Error message (if failed) */
  error?: string;
  /** Duration in ms */
  durationMs: number;
}

export interface UltraWorkResult {
  /** Results from each agent */
  agentResults: AgentResult[];
  /** Synthesized final output */
  synthesis: string;
  /** Total duration */
  durationMs: number;
  /** Execution mode used */
  mode: UltraWorkMode;
  /** Number of agents that succeeded */
  succeededCount: number;
  /** Number of agents that failed */
  failedCount: number;
}

// ---------------------------------------------------------------------------
// Agent selection based on IntentAnalysis
// ---------------------------------------------------------------------------

/**
 * Select agents based on intent analysis for adaptive mode.
 */
function selectAgentsForIntent(analysis: IntentAnalysis): string[] {
  const agents: string[] = [];

  // Always include sisyphus for coding tasks
  if (analysis.intent === "coding" || analysis.intent === "general") {
    agents.push("sisyphus");
  }

  // Add planner for planning tasks
  if (analysis.intent === "planning" || analysis.wantsPlanning) {
    agents.push("prometheus");
  }

  // Add explorer for exploration tasks
  if (analysis.intent === "explore") {
    agents.push("explore");
  }

  // Add oracle for review and questions
  if (analysis.intent === "review" || analysis.intent === "question") {
    agents.push("oracle");
  }

  // Add atlas for execution-heavy tasks
  if (analysis.scope === "project" || analysis.wantsParallel) {
    agents.push("atlas");
  }

  // Ensure at least one agent
  if (agents.length === 0) {
    agents.push("sisyphus");
  }

  return agents;
}

// ---------------------------------------------------------------------------
// UltraWork
// ---------------------------------------------------------------------------

export class UltraWork {
  private _provider: AiProvider;
  private _registry: ToolRegistry;
  private _workDir: string;
  private _config: UltraWorkConfig;
  private _intentGate: IntentGate;

  constructor(
    provider: AiProvider,
    registry: ToolRegistry,
    workDir: string,
    config?: Partial<UltraWorkConfig>,
  ) {
    this._provider = provider;
    this._registry = registry;
    this._workDir = workDir;
    this._config = { ...DEFAULT_ULTRA_CONFIG, ...config };
    this._intentGate = new IntentGate();
  }

  /** Run executes the ultrawork workflow with all agents. */
  async run(signal: AbortSignal | undefined, task: string): Promise<UltraWorkResult> {
    const start = Date.now();

    // Select agents based on mode
    let agentNames: string[];
    if (this._config.mode === "adaptive") {
      const analysis = this._intentGate.analyze(task);
      agentNames = selectAgentsForIntent(analysis);
    } else {
      // parallel and pipeline modes use all agents
      agentNames = Object.keys(AGENT_SPECS);
    }

    // Limit concurrent agents
    agentNames = agentNames.slice(0, this._config.maxConcurrentAgents);

    let agentResults: AgentResult[];

    if (this._config.mode === "pipeline") {
      agentResults = await this.runPipeline(signal, task, agentNames);
    } else {
      agentResults = await this.runParallel(signal, task, agentNames);
    }

    // Synthesize results
    const synthesis = synthesizeResults(agentResults, task);

    const succeeded = agentResults.filter((r) => r.success).length;
    const failed = agentResults.filter((r) => !r.success).length;

    return {
      agentResults,
      synthesis,
      durationMs: Date.now() - start,
      mode: this._config.mode,
      succeededCount: succeeded,
      failedCount: failed,
    };
  }

  /** Run a single agent as a standalone ultrawork task (backward-compatible). */
  async runSingle(signal: AbortSignal | undefined, task: string): Promise<AgentLoopResult> {
    const sid = SessionId.create();
    const agentCtx = new AgentContext(sid, this._workDir);
    agentCtx.setSystemPrompt("You are an autonomous agent working within a step budget. Complete the task efficiently.");

    const executor = new ToolExecutor(this._registry);
    const toolDefs = buildToolDefinitions(this._registry);
    const config = defaultLoopConfig();
    config.maxIterations = this._config.stepBudgetPerAgent;
    config.timeoutMs = this._config.timeoutPerAgentMs;
    config.autoApproveTools = this._config.autoApproveTools;
    const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, config, toolDefs);

    agentCtx.addUserMessage(task);
    const result = await loop.run({}, null);
    if (result.stopReason !== "completed") {
      throw new Error(`ultra work failed: stop reason ${result.stopReason}`);
    }
    return result;
  }

  // -----------------------------------------------------------------------
  // Private: Execution strategies
  // -----------------------------------------------------------------------

  private async runParallel(
    signal: AbortSignal | undefined,
    task: string,
    agentNames: string[],
  ): Promise<AgentResult[]> {
    const promises = agentNames.map((name) =>
      this.runAgent(signal, task, name),
    );

    // Run all agents concurrently, catching failures
    const results = await Promise.allSettled(promises);

    return results.map((result, i) => {
      if (result.status === "fulfilled") {
        return result.value;
      }
      return {
        agent: agentNames[i]!,
        success: false,
        output: "",
        error: result.reason instanceof Error ? result.reason.message : String(result.reason),
        durationMs: 0,
      };
    });
  }

  private async runPipeline(
    signal: AbortSignal | undefined,
    task: string,
    agentNames: string[],
  ): Promise<AgentResult[]> {
    const results: AgentResult[] = [];
    let accumulatedContext = task;

    for (const name of agentNames) {
      const agentTask = accumulatedContext;
      const result = await this.runAgent(signal, agentTask, name);
      results.push(result);

      // If agent succeeded, accumulate its output for the next agent
      if (result.success && result.output) {
        accumulatedContext = `${task}\n\n[${name} output]:\n${result.output}`;
      }

      // Check if aborted
      if (signal?.aborted) break;
    }

    return results;
  }

  private async runAgent(
    signal: AbortSignal | undefined,
    task: string,
    agentName: string,
  ): Promise<AgentResult> {
    const spec = AGENT_SPECS[agentName];
    if (!spec) {
      return {
        agent: agentName,
        success: false,
        output: "",
        error: `Unknown agent: ${agentName}`,
        durationMs: 0,
      };
    }

    const start = Date.now();
    const sid = SessionId.create();
    const agentCtx = new AgentContext(sid, this._workDir);
    agentCtx.setSystemPrompt(spec.systemPrompt);

    const fRegistry = filteredRegistry(this._registry, spec.allowedTools);
    const executor = new ToolExecutor(fRegistry);
    const toolDefs = buildToolDefinitions(fRegistry);

    const config = defaultLoopConfig();
    config.maxIterations = this._config.stepBudgetPerAgent;
    config.timeoutMs = this._config.timeoutPerAgentMs;
    config.autoApproveTools = this._config.autoApproveTools;

    const loop = new AgentLoop(AgentId.create(), this._provider, executor, agentCtx, config, toolDefs);
    agentCtx.addUserMessage(task);

    try {
      const loopResult = await loop.run({}, null);

      const lastAssistant = agentCtx.conversation.lastAssistantMessage();
      const output = lastAssistant?.content ?? "";

      return {
        agent: agentName,
        success: loopResult.stopReason === "completed" || loopResult.stopReason === "max_iterations",
        output,
        loopResult,
        durationMs: Date.now() - start,
      };
    } catch (err) {
      return {
        agent: agentName,
        success: false,
        output: "",
        error: err instanceof Error ? err.message : String(err),
        durationMs: Date.now() - start,
      };
    }
  }
}

// ---------------------------------------------------------------------------
// Result synthesis
// ---------------------------------------------------------------------------

function synthesizeResults(results: AgentResult[], task: string): string {
  const successful = results.filter((r) => r.success);
  const failed = results.filter((r) => !r.success);

  const parts: string[] = [];

  parts.push(`# UltraWork Synthesis\n`);
  parts.push(`Task: ${task}\n`);
  parts.push(`Agents: ${results.length} total, ${successful.length} succeeded, ${failed.length} failed\n`);

  // Include each agent's contribution
  for (const result of successful) {
    if (result.output.trim()) {
      parts.push(`## ${result.agent}\n`);
      parts.push(result.output);
      parts.push("");
    }
  }

  // Report failures
  if (failed.length > 0) {
    parts.push(`## Failed Agents\n`);
    for (const result of failed) {
      parts.push(`- ${result.agent}: ${result.error ?? "unknown error"}`);
    }
  }

  return parts.join("\n");
}
