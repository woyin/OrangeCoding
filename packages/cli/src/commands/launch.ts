/**
 * Handles the `launch` command and its sub-modes (single-shot, text, TUI).
 *
 * The launch command is the default when running the CLI with no subcommand.
 * It configures an AI provider, tool registry, agent context, and agent loop,
 * then executes the task through the full agent pipeline.
 */

import * as path from "node:path";
import { runTuiMode } from "./tui-mode.js";
import * as os from "node:os";
import { AgentId, SessionId } from "@orangecoding/core";
import { SessionManager } from "@orangecoding/session";
import { FileCheckpointStore } from "@orangecoding/agent";
import { ConfigManager, defaultConfig } from "@orangecoding/config";
import type { OrangeConfig, ProviderConfig, SkillDefinition } from "@orangecoding/config";
import {
  ProviderFactory,
  normalizeProviderConfig,
} from "@orangecoding/ai";
import type { ProviderConfig as AiProviderConfig } from "@orangecoding/ai";
import { createDefaultRegistry, CLIApprovalHandler } from "@orangecoding/tools";
import {
  AgentLoop,
  AgentContext,
  ToolExecutor,
  buildToolDefinitions,
  defaultLoopConfig,
  SkillRegistry,
  SkillMatcher,
} from "@orangecoding/agent";
import type {
  Skill,
  SkillContext,
  AgentLoopConfig,
} from "@orangecoding/agent";
import { RateLimitHandler } from "@orangecoding/ai";
import { AiError, AiErrorKind } from "@orangecoding/ai";
import type { AgentEvent } from "@orangecoding/core";
import {
  StreamChunkEvent,
  CompletedEvent,
  ToolCallRequestedEvent,
  ToolCallCompletedEvent,
  ErrorEvent,
  GuardrailDecisionEvent,
} from "@orangecoding/core";

/**
 * Handles the `launch` command.
 *
 * @param prompt - Single-shot task prompt (from --prompt / -p flag)
 * @param textMode - Whether to use text REPL mode (from --text flag)
 */
export async function runLaunch(
  prompt?: string,
  textMode = false,
  skillName?: string,
  resumeSessionId?: string,
): Promise<void> {
  // 1. Resolve config path
  const configPath = defaultLaunchConfigPath();

  // 2. Load config
  const mgr = new ConfigManager();
  let cfg: OrangeConfig;
  try {
    cfg = mgr.load(configPath);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    throw new Error(
      `failed to load config from ${configPath}: ${msg}\nRun 'orangecoding init' first`,
    );
  }

  // 3. Determine mode based on flags
  if (resumeSessionId) {
    // Resume mode: load a saved session and continue
    await runResumed(cfg, resumeSessionId, prompt, skillName);
    return;
  }
  if (prompt) {
    // Single-shot mode: run once with the given prompt, print result to stdout
    await runSingleShot(cfg, prompt, skillName);
  } else if (textMode) {
    // Text REPL mode
    await runTextREPL(cfg, skillName);
  } else {
    // TUI mode — full interactive terminal UI
    
    await runTuiMode(cfg, skillName);
  }
}

/**
 * Executes a single prompt through the full agent loop.
 */
async function runSingleShot(
  cfg: OrangeConfig,
  task: string,
  skillName?: string,
): Promise<void> {
  const providerName = cfg.default_provider || "openai";

  const providerConfig = aiProviderConfigFromCLIConfig(providerName, cfg);
  const factory = new ProviderFactory();
  const aiProvider = factory.createProvider(providerName, providerConfig);

  const registry = createDefaultRegistry();
  const executor = new ToolExecutor(registry);
  executor.setApprovalHandler(new CLIApprovalHandler());
  const toolDefs = buildToolDefinitions(registry);

  // Resolve skill
  const skillRegistry = buildSkillRegistry(cfg);
  const skillCtx = resolveSkill(skillName, task, skillRegistry, registry);

  // Create agent context
  const sid = SessionId.create();
  const ctx = new AgentContext(sid, process.cwd());
  ctx.setSystemPrompt(
    skillCtx?.systemPrompt
      ?? "You are OrangeCoding, a practical coding agent. Help the user complete software tasks. " +
         "Be concise and direct. Use tools when needed to read files, write code, and run commands.",
  );

  // Build loop config
  const loopConfig = buildLoopConfig(cfg);

  // Apply skill context if matched
  if (skillCtx) {
    loopConfig.skill = skillCtx;
  }

  // Create agent loop
  const loop = new AgentLoop(
    createAgentId(),
    aiProvider,
    executor,
    ctx,
    loopConfig,
    toolDefs,
  );

  // Add the user's task
  ctx.addUserMessage(task);

  // Set up event handler for streaming output
  const eventHandler = createConsoleEventHandler();

  console.log(`\x1b[36m⚡ OrangeCoding\x1b[0m | ${providerName} | ${providerConfig.defaultModel || "default"}`);
  console.log(`\x1b[90mSession: ${sid.toString()}\x1b[0m`);
  if (skillCtx) {
    console.log(`\x1b[33mSkill: ${skillCtx.skill.name}\x1b[0m — ${skillCtx.skill.description}`);
  }
  console.log("");

  try {
    const result = await loop.run(
      { model: providerConfig.defaultModel } as never,
      eventHandler,
    );

    // Print the last assistant message (the final answer)
    const lastAssistant = ctx.conversation.lastAssistantMessage();
    if (lastAssistant) {
      console.log(`\n\x1b[32m${lastAssistant.content}\x1b[0m`);
    }

    // Print summary
    console.log(`\n\x1b[90m--- Done: ${result.toolCallsMade} tool calls, ` +
      `${result.tokensUsed.totalTokens} tokens, ` +
      `${(result.durationMs / 1000).toFixed(1)}s, ` +
      `stopped: ${result.stopReason}\x1b[0m`);
  } catch (err) {
    if (err instanceof AiError && err.kind === AiErrorKind.RateLimit) {
      console.error("Rate limit exceeded and retry limit reached. Please try again later.");
      process.exit(1);
    }
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`\x1b[31mError: ${msg}\x1b[0m`);
    process.exit(1);
  }
}

/**
 * Text REPL mode — reads prompts from stdin, runs them through the agent loop,
 * and prints results. Maintains conversation context across turns.
 */
async function runTextREPL(
  cfg: OrangeConfig,
  skillName?: string,
): Promise<void> {
  const providerName = cfg.default_provider || "openai";
  const providerConfig = aiProviderConfigFromCLIConfig(providerName, cfg);
  const factory = new ProviderFactory();
  const aiProvider = factory.createProvider(providerName, providerConfig);

  const registry = createDefaultRegistry();
  const executor = new ToolExecutor(registry);
  executor.setApprovalHandler(new CLIApprovalHandler());
  const toolDefs = buildToolDefinitions(registry);

  // Resolve skill
  const skillRegistry = buildSkillRegistry(cfg);

  // Create persistent agent context
  const sid = SessionId.create();
  const ctx = new AgentContext(sid, process.cwd());
  ctx.setSystemPrompt(
    "You are OrangeCoding, a practical coding agent. Help the user complete software tasks. " +
    "Be concise and direct. Use tools when needed to read files, write code, and run commands.",
  );

  const loopConfig = buildLoopConfig(cfg);

  console.log(`\x1b[36m⚡ OrangeCoding Text REPL\x1b[0m | ${providerName} | ${providerConfig.defaultModel || "default"}`);
  console.log(`\x1b[90mSession: ${sid.toString()}\x1b[0m`);
  console.log(`\x1b[90mType your task and press Enter. Type 'exit' or Ctrl+D to quit.\x1b[0m\n`);

  const readline = await import("node:readline");
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  const askQuestion = (): Promise<string> => {
    return new Promise((resolve) => {
      rl.question("\x1b[33m> \x1b[0m", (answer) => {
        resolve(answer.trim());
      });
    });
  };

  while (true) {
    const input = await askQuestion();
    if (!input || input === "exit" || input === "quit") {
      console.log("\x1b[90mGoodbye!\x1b[0m");
      break;
    }

    // Check for skill match on each input
    const skillCtx = resolveSkill(skillName, input, skillRegistry, registry);
    const currentLoopConfig = { ...loopConfig };
    if (skillCtx) {
      currentLoopConfig.skill = skillCtx;
      console.log(`\x1b[33mSkill: ${skillCtx.skill.name}\x1b[0m`);
    }

    ctx.addUserMessage(input);

    const loop = new AgentLoop(
      createAgentId(),
      aiProvider,
      executor,
      ctx,
      currentLoopConfig,
      [...toolDefs],
    );

    const eventHandler = createConsoleEventHandler();

    try {
      const result = await loop.run(
        { model: providerConfig.defaultModel } as never,
        eventHandler,
      );

      const lastAssistant = ctx.conversation.lastAssistantMessage();
      if (lastAssistant) {
        console.log(`\n\x1b[32m${lastAssistant.content}\x1b[0m`);
      }

      console.log(`\x1b[90m--- ${result.toolCallsMade} tool calls, ` +
        `${result.tokensUsed.totalTokens} tokens, ` +
        `${(result.durationMs / 1000).toFixed(1)}s\x1b[0m\n`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error(`\x1b[31mError: ${msg}\x1b[0m`);
    }
  }

  rl.close();
}

/**
 * Creates a console event handler that streams agent events to stdout.
 */
function createConsoleEventHandler(): (event: AgentEvent) => void {
  let lastWasStream = false;

  return (event: AgentEvent): void => {
    if (event instanceof StreamChunkEvent) {
      process.stdout.write(event.content);
      lastWasStream = true;
    } else if (event instanceof ToolCallRequestedEvent) {
      if (lastWasStream) {
        process.stdout.write("\n");
        lastWasStream = false;
      }
      console.log(`\x1b[34m🔧 ${event.toolCall.function_name}\x1b[0m`);
    } else if (event instanceof ToolCallCompletedEvent) {
      const status = event.success ? "✅" : "❌";
      console.log(`\x1b[90m  ${status} ${event.durationMs}ms\x1b[0m`);
    } else if (event instanceof GuardrailDecisionEvent) {
      if (event.decision === "deny") {
        console.log(`\x1b[31m🛡️  Guardrail: ${event.reason}\x1b[0m`);
      }
    } else if (event instanceof ErrorEvent) {
      console.error(`\x1b[31m❌ ${event.errorMessage}\x1b[0m`);
    } else if (event instanceof CompletedEvent) {
      if (lastWasStream) {
        process.stdout.write("\n");
        lastWasStream = false;
      }
    }
  };
}

/**
 * Build an AgentLoopConfig from OrangeConfig harness settings.
 */
function buildLoopConfig(cfg: OrangeConfig): AgentLoopConfig {
  const base = defaultLoopConfig();

  if (cfg.harness.reasoning_effort) {
    base.reasoning.effort = cfg.harness.reasoning_effort as "low" | "medium" | "high";
  }
  if (cfg.harness.reasoning_budget_tokens > 0) {
    base.reasoning.budgetTokens = cfg.harness.reasoning_budget_tokens;
  }

  return base;
}

/**
 * Creates a new AgentId.
 */
function createAgentId(): AgentId {
  return AgentId.create();
}

/**
 * Converts CLI config to AI provider config, resolving provider aliases.
 */
export function aiProviderConfigFromCLIConfig(
  providerName: string,
  cfg: OrangeConfig,
): AiProviderConfig {
  let providerCfg: ProviderConfig = {
    api_key: "",
  };

  if (cfg?.providers) {
    for (const candidate of providerConfigKeys(providerName)) {
      const found = cfg.providers[candidate];
      if (
        found &&
        (found.api_key || found.base_url || found.default_model)
      ) {
        providerCfg = found;
        break;
      }
    }
  }

  const aiCfg: AiProviderConfig = {
    apiKey: providerCfg.api_key || "",
    apiSecret: providerCfg.api_secret || "",
    baseURL: providerCfg.base_url || "",
    defaultModel: providerCfg.default_model || "",
    timeoutSecs: providerCfg.timeout_secs || 0,
    extra: providerCfg.extra || {},
  };

  if (cfg?.default_model) {
    aiCfg.defaultModel = cfg.default_model;
  }

  return normalizeProviderConfig(providerName, aiCfg);
}

/**
 * Returns the candidate config key names for a given provider,
 * including canonical aliases (e.g. gpt -> openai, opus -> anthropic).
 */
export function providerConfigKeys(providerName: string): string[] {
  const normalized = providerName.toLowerCase().trim();
  const keys: string[] = [providerName, normalized];

  switch (normalized) {
    case "gpt":
      keys.push("openai");
      break;
    case "opus":
    case "claude":
      keys.push("anthropic");
      break;
    case "moonshot":
      keys.push("kimi");
      break;
    case "bigmodel":
    case "zhipu":
      keys.push("glm");
      break;
  }

  return keys;
}

/**
 * Returns the default config path for launch.
 * Uses ~/.orangecoding/config.json.
 */
/**
 * Creates an OrangeConfig from environment variables.
 * This allows running without a config file by setting:
 *   OPENAI_API_KEY, ANTHROPIC_API_KEY, DEEPSEEK_API_KEY, etc.
 */
function configFromEnvironment(): OrangeConfig {
  const cfg = defaultConfig();

  // Auto-detect provider from available API keys
  if (process.env["ANTHROPIC_API_KEY"]) {
    cfg.default_provider = "anthropic";
    cfg.providers["anthropic"] = { api_key: process.env["ANTHROPIC_API_KEY"]! };
  } else if (process.env["OPENAI_API_KEY"]) {
    cfg.default_provider = "openai";
    cfg.providers["openai"] = { api_key: process.env["OPENAI_API_KEY"]! };
  } else if (process.env["DEEPSEEK_API_KEY"]) {
    cfg.default_provider = "deepseek";
    cfg.providers["deepseek"] = { api_key: process.env["DEEPSEEK_API_KEY"]! };
  } else if (process.env["DASHSCOPE_API_KEY"]) {
    cfg.default_provider = "qianwen";
    cfg.providers["qianwen"] = { api_key: process.env["DASHSCOPE_API_KEY"]! };
  }

  // Allow model override via env
  if (process.env["ORANGECODING_MODEL"]) {
    cfg.default_model = process.env["ORANGECODING_MODEL"]!;
  }

  return cfg;
}

export function defaultLaunchConfigPath(): string {
  const home = os.homedir() || ".";
  return path.join(home, ".orangecoding", "config.json");
}

/**
 * Build a SkillRegistry with built-in + config-defined custom skills.
 */
function buildSkillRegistry(cfg: OrangeConfig): SkillRegistry {
  const registry = new SkillRegistry();
  const customSkills = cfg.skills?.custom ?? [];
  for (const def of customSkills) {
    registry.register({
      name: def.name,
      description: def.description ?? "",
      tools: def.tools ?? [],
      prompt: def.prompt ?? "",
      tags: def.tags,
      examples: def.examples,
    });
  }
  return registry;
}



/**
 * Resumes a saved session by loading its conversation history
 * and continuing with a new prompt through the agent loop.
 */
async function runResumed(
  cfg: OrangeConfig,
  resumeSessionId: string,
  newPrompt?: string,
  skillName?: string,
): Promise<void> {
  const { restoreConversation } = await import("./resume-helper.js");
  const sessionDir = getSessionDir();

  // Parse the session ID
  let sid: SessionId;
  try {
    sid = resumeSessionId.startsWith("session-")
      ? SessionId.parse(resumeSessionId)
      : SessionId.parse("session-" + resumeSessionId);
  } catch {
    console.error(`[31mInvalid session ID: ${resumeSessionId}[0m`);
    process.exit(1);
    return;
  }

  // Load saved session
  let restored;
  try {
    restored = await restoreConversation(sessionDir, sid);
  } catch (err) {
    console.error(`[31mFailed to load session: ${err instanceof Error ? err.message : String(err)}[0m`);
    process.exit(1);
    return;
  }

  const providerName = cfg.default_provider || "openai";
  const providerConfig = aiProviderConfigFromCLIConfig(providerName, cfg);
  const factory = new ProviderFactory();
  const aiProvider = factory.createProvider(providerName, providerConfig);

  const registry = createDefaultRegistry();
  const executor = new ToolExecutor(registry);
  executor.setApprovalHandler(new CLIApprovalHandler());
  const toolDefs = buildToolDefinitions(registry);

  // Create agent context and restore conversation
  const ctx = new AgentContext(sid, process.cwd());

  // Restore all saved messages to the conversation
  for (const msg of restored.messages) {
    ctx.conversation.addMessage(msg);
  }

  console.log(`[36m⚡ OrangeCoding[0m | Resuming session: ${sid.toString()}`);
  console.log(`[90mLoaded ${restored.messages.length} messages from previous session[0m`);

  // If no new prompt, enter REPL mode with restored context
  if (!newPrompt) {
    const readline = await import("node:readline");
    const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
    const askQuestion = (): Promise<string> => new Promise((resolve) => {
      rl.question("[33m> [0m", (answer) => resolve(answer.trim()));
    });

    console.log(`[90mType your next task and press Enter. Type 'exit' to quit.[0m
`);

    while (true) {
      const input = await askQuestion();
      if (!input || input === "exit" || input === "quit") {
        console.log("[90mGoodbye![0m");
        break;
      }

      ctx.addUserMessage(input);
      const loopConfig = buildLoopConfig(cfg);
      const loop = new AgentLoop(createAgentId(), aiProvider, executor, ctx, loopConfig, [...toolDefs]);
      const eventHandler = createConsoleEventHandler();

      try {
        const result = await loop.run({ model: providerConfig.defaultModel } as never, eventHandler);
        const lastAssistant = ctx.conversation.lastAssistantMessage();
        if (lastAssistant) {
          console.log(`
[32m${lastAssistant.content}[0m`);
        }
        await saveSession(sid, ctx);
        console.log(`[90m--- ${result.toolCallsMade} tool calls, ${result.tokensUsed.totalTokens} tokens, ${(result.durationMs / 1000).toFixed(1)}s[0m
`);
      } catch (err) {
        console.error(`[31mError: ${err instanceof Error ? err.message : String(err)}[0m`);
      }
    }
    rl.close();
    return;
  }

  // Single-shot resume: run the new prompt with restored context
  ctx.addUserMessage(newPrompt);
  const loopConfig = buildLoopConfig(cfg);
  const loop = new AgentLoop(createAgentId(), aiProvider, executor, ctx, loopConfig, toolDefs);
  const eventHandler = createConsoleEventHandler();

  console.log(`[90mTask: ${newPrompt}[0m
`);

  try {
    const result = await loop.run({ model: providerConfig.defaultModel } as never, eventHandler);
    const lastAssistant = ctx.conversation.lastAssistantMessage();
    if (lastAssistant) {
      console.log(`
[32m${lastAssistant.content}[0m`);
    }
    await saveSession(sid, ctx);
    console.log(`
[90m--- Done: ${result.toolCallsMade} tool calls, ${result.tokensUsed.totalTokens} tokens, ${(result.durationMs / 1000).toFixed(1)}s, stopped: ${result.stopReason}[0m`);
  } catch (err) {
    console.error(`[31mError: ${err instanceof Error ? err.message : String(err)}[0m`);
    process.exit(1);
  }
}

/**
 * Saves the current session (conversation) to disk for later resumption.
 */
async function saveSession(sid: SessionId, ctx: import("@orangecoding/agent").AgentContext): Promise<void> {
  const sessionDir = getSessionDir();
  const manager = new SessionManager(sessionDir);

  // Create a Session from the agent context's conversation
  const messages = ctx.conversation.messages();
  const session = new (await import("@orangecoding/session")).Session(
    sid,
    messages as any[],
    {},
    new (await import("@orangecoding/core")).TokenUsage(0, 0, 0),
    new Date(),
    new Date(),
  );

  try {
    await manager.update(session);
    console.log(`[90m💾 Session saved: ${sid.toString()}[0m`);
  } catch (err) {
    console.error(`[90mWarning: failed to save session: ${err instanceof Error ? err.message : String(err)}[0m`);
  }
}

function getSessionDir(): string {
  const home = os.homedir() || ".";
  return path.join(home, ".orangecoding", "sessions");
}

/**
 * Resolve a skill by name or auto-detect from the task prompt.
 */
function resolveSkill(
  skillName: string | undefined,
  task: string,
  skillRegistry: SkillRegistry,
  toolRegistry: ReturnType<typeof createDefaultRegistry>,
): SkillContext | undefined {
  let skill: Skill | undefined;

  if (skillName) {
    const [s, ok] = skillRegistry.get(skillName);
    if (!ok) {
      throw new Error(`unknown skill: "${skillName}". Run 'orangecoding skills' to see available skills.`);
    }
    skill = s;
  } else {
    // Auto-detect
    const matcher = new SkillMatcher();
    const match = matcher.bestMatch(task, skillRegistry);
    if (match) {
      skill = match.skill;
    }
  }

  if (!skill) return undefined;
  return skillRegistry.resolveContext(skill, toolRegistry);
}
