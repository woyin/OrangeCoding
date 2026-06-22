/**
 * @module cli-tui-mode
 *
 * CLI command for starting the interactive TUI (Terminal UI) mode.
 *
 * Launches the agent with a full-screen terminal interface featuring:
 * - Real-time conversation display
 * - Markdown rendering
 * - Tool execution status indicators
 * - Interactive input handling
 */

import { AgentId, SessionId } from "@orangecoding/core";
import type { AgentEvent } from "@orangecoding/core";
import { SessionManager } from "@orangecoding/session";
import type { OrangeConfig } from "@orangecoding/config";
import {
  ProviderFactory,
  normalizeProviderConfig,
} from "@orangecoding/ai";
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
import type { SkillContext } from "@orangecoding/agent";
import { App, TuiEventBridge, FileWatcher } from "@orangecoding/tui";
import type { ProviderConfig as AiProviderConfig } from "@orangecoding/ai";
import {
  aiProviderConfigFromCLIConfig,
  defaultLaunchConfigPath,
} from "./launch.js";

/**
 * Run the TUI mode — a full-screen terminal UI with interactive agent loop.
 */
export async function runTuiMode(
  cfg: OrangeConfig,
  skillName?: string,
): Promise<void> {
  // Resolve the AI provider from config: name, credentials, default model.
  const providerName = cfg.default_provider || "openai";
  const providerConfig = aiProviderConfigFromCLIConfig(providerName, cfg);
  const factory = new ProviderFactory();
  const aiProvider = factory.createProvider(providerName, providerConfig);

  // Build the tool registry and executor: tools the agent can call this session.
  const registry = createDefaultRegistry();
  const executor = new ToolExecutor(registry);
  executor.setApprovalHandler(new CLIApprovalHandler());
  const toolDefs = buildToolDefinitions(registry);

  // Register custom skills defined in config so the matcher can auto-select them.
  const skillRegistry = new SkillRegistry();
  const customSkills = cfg.skills?.custom ?? [];
  for (const def of customSkills) {
    skillRegistry.register({
      name: def.name,
      description: def.description ?? "",
      tools: def.tools ?? [],
      prompt: def.prompt ?? "",
      tags: def.tags,
      examples: def.examples,
    });
  }

  // Create the agent context with cwd and a baseline system prompt.
  const sid = SessionId.create();
  const ctx = new AgentContext(sid, process.cwd());
  ctx.setSystemPrompt(
    "You are OrangeCoding, a practical coding agent. Help the user complete software tasks. " +
    "Be concise and direct. Use tools when needed to read files, write code, and run commands.",
  );

  const loopConfig = defaultLoopConfig();

  // Create the TUI App — the full-screen raw-mode terminal renderer.
  const app = new App();

  // Create the event bridge — translates AgentEvents into TUI model messages.
  const bridge = new TuiEventBridge(app);

  // Set up the onSubmit callback — runs user input through the agent loop.
  // Guards against re-entrancy with isProcessing, auto-detects a matching skill,
  // builds a fresh AgentLoop per turn, and forwards agent events to the TUI bridge.
  let isProcessing = false;
  app.onSubmit = async (text: string) => {
    if (isProcessing) return;
    isProcessing = true;

    // Add user message to conversation
    ctx.addUserMessage(text);

    // Auto-detect skill
    const matcher = new SkillMatcher();
    const match = matcher.bestMatch(text, skillRegistry);
    const currentLoopConfig = { ...loopConfig };
    if (match) {
      const skillCtx = skillRegistry.resolveContext(match.skill, registry);
      if (skillCtx) {
        currentLoopConfig.skill = skillCtx;
      }
    }

    // Create a new agent loop for this turn
    const loop = new AgentLoop(
      AgentId.create(),
      aiProvider,
      executor,
      ctx,
      currentLoopConfig,
      [...toolDefs],
    );

    const eventHandler = bridge.getHandler();

    try {
      await loop.run(
        { model: providerConfig.defaultModel } as never,
        eventHandler,
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      app.send({ type: "status", status: `❌ ${msg}` });
    }

    isProcessing = false;
  };

  // Start file watcher for the current working directory — surfaces filesystem
  // changes as status-line notifications so the user sees external edits.
  const watcher = new FileWatcher({
    paths: [process.cwd()],
    debounceMs: 500,
    ignorePatterns: ["node_modules", ".git", "dist", ".next", "__pycache__", "*.pyc"],
  });

  watcher.start((event) => {
    // Notify user about file changes
    const basename = event.path.split("/").pop() ?? event.path;
    app.send({
      type: "status",
      status: `📄 ${event.type}: ${basename}`,
    });
  });

  // Run the TUI (blocks until quit)
  try {
    await app.run();
  } finally {
    watcher.stop();
  }
}
