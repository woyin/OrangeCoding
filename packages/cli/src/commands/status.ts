import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { ConfigManager } from "@orangecoding/config";
import { createDefaultRegistry } from "@orangecoding/tools";
import { VERSION } from "../version.js";
import { defaultConfigPath } from "./init.js";

/**
 * Generates a status summary string using the config at path.
 */
export function runStatusAtPath(configPath: string): string {
  const lines: string[] = [];

  lines.push(`OrangeCoding v${VERSION}`);
  lines.push(`Config: ${configPath}`);

  // Try to load config and list providers
  const mgr = new ConfigManager();
  let cfg: ReturnType<ConfigManager["load"]>;
  try {
    cfg = mgr.load(configPath);
  } catch {
    lines.push("Config: (not found or unreadable)");
    lines.push("");
    lines.push("Environment API keys:");
    appendEnvKeys(lines);
    appendToolCount(lines);
    appendSessionCount(lines);
    return lines.join("\n") + "\n";
  }

  const providers = Object.keys(cfg.providers);

  if (providers.length === 0) {
    lines.push("Providers: (none configured)");
  } else {
    lines.push(`Providers: ${providers.join(", ")}`);
  }

  lines.push(`Default provider: ${cfg.default_provider}`);
  lines.push(`Default model: ${cfg.default_model || "(auto)"}`);
  lines.push(`Control port: ${cfg.control_port}`);

  // Fallback providers
  const fallbackProviders = providers.filter((p) => p !== cfg.default_provider);
  if (fallbackProviders.length > 0) {
    lines.push(`Fallback chain: ${fallbackProviders.join(" → ")}`);
  }

  // API key status
  lines.push("");
  lines.push("API Keys:");
  for (const name of providers) {
    const hasKey = cfg.providers[name]?.api_key ? "✅" : "❌";
    lines.push(`  ${hasKey} ${name}`);
  }
  appendEnvKeys(lines);

  // Tools
  lines.push("");
  appendToolCount(lines);

  // Sessions
  lines.push("");
  appendSessionCount(lines);

  // Audit
  lines.push("");
  if (cfg.audit.enabled) {
    lines.push(`Audit: enabled (${cfg.audit.dir})`);
  } else {
    lines.push("Audit: disabled");
  }

  // Harness
  lines.push(`Harness: checkpoint=${cfg.harness.checkpoint_store}, reasoning=${cfg.harness.reasoning_effort}`);

  return lines.join("\n") + "\n";
}

function appendEnvKeys(lines: string[]): void {
  const envKeys: Array<[string, string]> = [
    ["OPENAI_API_KEY", "openai"],
    ["ANTHROPIC_API_KEY", "anthropic"],
    ["DEEPSEEK_API_KEY", "deepseek"],
    ["DASHSCOPE_API_KEY", "qianwen"],
  ];

  for (const [envVar, provider] of envKeys) {
    if (process.env[envVar]) {
      lines.push(`  ✅ ${provider} (via ${envVar})`);
    }
  }
}

function appendToolCount(lines: string[]): void {
  try {
    const registry = createDefaultRegistry();
    const tools = registry.list();
    lines.push(`Tools: ${tools.length} registered`);
    const toolNames = tools.map((t) => t.name()).join(", ");
    if (toolNames.length < 200) {
      lines.push(`  ${toolNames}`);
    } else {
      lines.push(`  ${toolNames.slice(0, 200)}...`);
    }
  } catch {
    lines.push("Tools: (failed to load registry)");
  }
}

function appendSessionCount(lines: string[]): void {
  const home = os.homedir() || ".";
  const sessionDir = path.join(home, ".orangecoding", "sessions");
  try {
    const entries = fs.readdirSync(sessionDir);
    const sessionFiles = entries.filter((e) => e.endsWith(".jsonl"));
    lines.push(`Sessions: ${sessionFiles.length} saved`);
  } catch {
    lines.push("Sessions: 0 saved");
  }
}

/**
 * Handles the `status` command.
 * Displays the current OrangeCoding version, configuration, and system status.
 */
export function runStatus(): void {
  const configPath = defaultConfigPath();
  const output = runStatusAtPath(configPath);
  process.stdout.write(output);
}
