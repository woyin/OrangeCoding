/**
 * Handles the `launch` command and its sub-modes (single-shot, text, TUI).
 *
 * The launch command is the default when running the CLI with no subcommand.
 * It configures an AI provider, tool registry, agent context, and agent loop,
 * then executes the task.
 */

import * as path from "node:path";
import * as os from "node:os";
import { ConfigManager } from "@orangecoding/config";
import type { OrangeConfig, ProviderConfig, SkillDefinition } from "@orangecoding/config";
import {
  ProviderFactory,
  normalizeProviderConfig,
} from "@orangecoding/ai";
import type { ProviderConfig as AiProviderConfig } from "@orangecoding/ai";
import { createDefaultRegistry } from "@orangecoding/tools";
import { SkillRegistry, SkillMatcher } from "@orangecoding/agent";
import type { Skill, SkillContext } from "@orangecoding/agent";
import { RateLimitHandler } from "@orangecoding/ai";
import { AiError, AiErrorKind } from "@orangecoding/ai";

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
  if (prompt) {
    // Single-shot mode: run once with the given prompt, print result to stdout
    await runSingleShot(cfg, prompt, skillName);
  } else if (textMode) {
    // Text REPL mode (stub)
    console.log("Text REPL mode is not yet implemented.");
  } else {
    // TUI mode (stub)
    console.log("TUI mode is not yet implemented.");
    console.log(
      "Use --text for text mode or -p <prompt> for single-shot mode.",
    );
  }
}

/**
 * Executes a single prompt through the configured provider.
 *
 * NOTE: The @orangecoding/agent package is not yet available as a TS package.
 * When it is, this will create a full AgentLoop with tool execution.
 * For now, this demonstrates the wiring up to the provider level.
 */
async function runSingleShot(
  cfg: OrangeConfig,
  task: string,
  skillName?: string,
): Promise<void> {
  let providerName = cfg.default_provider;
  if (!providerName) {
    providerName = "openai";
  }

  const providerConfig = aiProviderConfigFromCLIConfig(providerName, cfg);
  const factory = new ProviderFactory();
  const aiProvider = factory.createProvider(providerName, providerConfig);

  console.log(
    `Provider: ${providerName}, Model: ${providerConfig.defaultModel}`,
  );
  console.log(`Task: ${task}`);

  const registry = createDefaultRegistry();
  const toolNames = registry.list().map((t) => t.name());
  console.log(`Tools available: ${toolNames.length}`);

  // Resolve skill
  const skillRegistry = buildSkillRegistry(cfg);
  const skillCtx = resolveSkill(skillName, task, skillRegistry, registry);
  if (skillCtx) {
    console.log(`Skill: ${skillCtx.skill.name} — ${skillCtx.skill.description}`);
  } else {
    console.log("Skill: auto (no specific skill matched)");
  }

  // NOTE: Full agent loop execution requires @orangecoding/agent package.
  // When available, the wiring will be:
  //
  //   const sessionId = newSessionId();
  //   const agentCtx = new AgentContext(sessionId, process.cwd());
  //   agentCtx.setSystemPrompt("You are OrangeCoding, a practical coding agent...");
  //   agentCtx.addUserMessage(task);
  //
  //   const loop = new AgentLoop(
  //     newAgentId(),
  //     aiProvider,
  //     new ToolExecutor(registry),
  //     agentCtx,
  //     await agentLoopConfigFromCLIConfig(configPath, cfg),
  //     buildToolDefinitions(registry),
  //   );
  //
  //   const result = await loop.run({ model: providerConfig.defaultModel });
  //   const answer = lastAssistantContent(agentCtx);
  //   if (answer) console.log(answer);

  // For now, do a simple provider call to verify wiring
  const { systemMsg, userMsg } = await import("@orangecoding/ai");
  const systemPrompt = skillCtx?.systemPrompt
    ?? "You are OrangeCoding, a practical coding agent. Help the user complete software tasks.";
  const messages = [
    systemMsg(systemPrompt),
    userMsg(task),
  ];

  try {
    const handler = new RateLimitHandler({ promptUser: true });
    const response = await handler.execute(() =>
      aiProvider.chatCompletion(messages, [], {
        model: providerConfig.defaultModel,
      }),
    );
    console.log(response.content);
  } catch (err) {
    if (err instanceof AiError && err.kind === AiErrorKind.RateLimit) {
      console.error("Rate limit exceeded and retry limit reached. Please try again later.");
      process.exit(1);
    }
    const msg = err instanceof Error ? err.message : String(err);
    throw new Error(`provider error: ${msg}`);
  }
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
function defaultLaunchConfigPath(): string {
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
