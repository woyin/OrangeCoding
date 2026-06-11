/**
 * SessionBuilder assembles a complete AgentLoop from OrangeConfig.
 *
 * This is the wiring layer that connects:
 *   - Config → AiProvider (via ProviderFactory)
 *   - Config → ToolRegistry (via createDefaultRegistry)
 *   - ToolRegistry → ToolDefinitions (via buildToolDefinitions)
 *   - ToolRegistry → ToolExecutor
 *   - SessionID → AgentContext
 *   - All of the above → AgentLoop
 */

import { AgentId, SessionId } from "@orangecoding/core";
import type { OrangeConfig } from "@orangecoding/config";
import { ProviderFactory, type ProviderConfig } from "@orangecoding/ai";
import type { AiProvider } from "@orangecoding/ai";
import { FallbackChain } from "@orangecoding/ai";
import { createDefaultRegistry } from "@orangecoding/tools";
import {
  AgentLoop,
  AgentContext,
  ToolExecutor,
  buildToolDefinitions,
  defaultLoopConfig,
} from "@orangecoding/agent";
import type { AgentLoopConfig } from "@orangecoding/agent";
import type { ApprovalHandler } from "@orangecoding/tools";

// ---------------------------------------------------------------------------
// SessionBuilder
// ---------------------------------------------------------------------------

export interface BuildSessionOptions {
  /** The session identifier. */
  sessionId: string;
  /** Working directory for tools (defaults to process.cwd()). */
  workDir?: string;
  /** Optional system prompt override. */
  systemPrompt?: string;
  /** Optional approval handler for tool execution. */
  approvalHandler?: ApprovalHandler;
}

/**
 * SessionBuilder creates AgentLoop instances from OrangeConfig.
 */
export class SessionBuilder {
  private readonly factory = new ProviderFactory();
  private readonly config: OrangeConfig;

  constructor(config: OrangeConfig) {
    this.config = config;
  }

  /**
   * Build assembles a complete AgentLoop for the given session.
   *
   * The returned AgentLoop is ready to be passed to an AgentExecutor
   * for task processing.
   */
  build(opts: BuildSessionOptions): AgentLoop {
    // 1. Create AI provider
    const provider = this.createProvider();

    // 2. Create tool registry and executor
    const registry = createDefaultRegistry();
    const executor = new ToolExecutor(registry);
    if (opts.approvalHandler) {
      executor.setApprovalHandler(opts.approvalHandler);
    }

    // 3. Build tool definitions for the AI provider
    const toolDefs = buildToolDefinitions(registry);

    // 4. Create agent context
    const sid = SessionId.parse(opts.sessionId.startsWith("session-")
      ? opts.sessionId
      : `session-${opts.sessionId}`);
    const workDir = opts.workDir ?? process.cwd();
    const ctx = new AgentContext(sid, workDir);

    if (opts.systemPrompt) {
      ctx.setSystemPrompt(opts.systemPrompt);
    }

    // 5. Build loop config
    const loopConfig = this.buildLoopConfig();

    // 6. Assemble agent loop
    const agentId = AgentId.create();
    return new AgentLoop(agentId, provider, executor, ctx, loopConfig, toolDefs);
  }

  /**
   * CreateProvider instantiates the AI provider from config.
   * When multiple providers have API keys configured, creates a FallbackChain.
   */
  private createProvider(): AiProvider {
    const providerName = this.config.default_provider;

    // Check if multiple providers are configured with API keys
    const configuredProviders = this.getConfiguredProviders();
    if (configuredProviders.length > 1) {
      // Create fallback chain with all configured providers
      // Default provider goes first
      const providers: AiProvider[] = [];
      for (const name of configuredProviders) {
        try {
          providers.push(this.createSingleProvider(name));
        } catch {
          // Skip providers that fail to initialize
        }
      }
      if (providers.length > 1) {
        return new FallbackChain(providers, 30_000); // 30s cooldown
      }
      if (providers.length === 1) {
        return providers[0]!;
      }
    }

    return this.createSingleProvider(providerName);
  }

  private createSingleProvider(providerName: string): AiProvider {
    const providerCfg = this.config.providers[providerName];
    const pcfg: ProviderConfig = {
      apiKey: providerCfg?.api_key ?? process.env["OPENAI_API_KEY"] ?? "",
      apiSecret: providerCfg?.api_secret ?? "",
      baseURL: providerCfg?.base_url ?? "",
      defaultModel: this.config.default_model || providerCfg?.default_model || "",
      timeoutSecs: providerCfg?.timeout_secs ?? 120,
      extra: providerCfg?.extra ?? {},
    };
    return this.factory.createProvider(providerName, pcfg);
  }

  /** Returns names of providers that have API keys configured. */
  private getConfiguredProviders(): string[] {
    const result: string[] = [];
    const defaultName = this.config.default_provider;

    // Check default provider first
    if (this.hasProviderKey(defaultName)) {
      result.push(defaultName);
    }

    // Check other configured providers
    for (const [name, cfg] of Object.entries(this.config.providers)) {
      if (name === defaultName) continue;
      if (cfg.api_key) {
        result.push(name);
      }
    }

    // Check environment variables
    if (process.env["OPENAI_API_KEY"] && !result.includes("openai")) {
      result.push("openai");
    }
    if (process.env["ANTHROPIC_API_KEY"] && !result.includes("anthropic")) {
      result.push("anthropic");
    }

    return result;
  }

  private hasProviderKey(name: string): boolean {
    const cfg = this.config.providers[name];
    if (cfg?.api_key) return true;
    if (name === "openai" && process.env["OPENAI_API_KEY"]) return true;
    if ((name === "anthropic" || name === "claude") && process.env["ANTHROPIC_API_KEY"]) return true;
    return false;
  }

  /**
   * BuildLoopConfig creates an AgentLoopConfig from OrangeConfig harness settings.
   */
  private buildLoopConfig(): AgentLoopConfig {
    const base = defaultLoopConfig();

    // Apply harness overrides
    if (this.config.harness.reasoning_effort) {
      base.reasoning.effort = this.config.harness.reasoning_effort as "low" | "medium" | "high";
    }
    if (this.config.harness.reasoning_budget_tokens > 0) {
      base.reasoning.budgetTokens = this.config.harness.reasoning_budget_tokens;
    }

    return base;
  }
}
