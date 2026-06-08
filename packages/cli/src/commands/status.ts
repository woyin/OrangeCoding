import * as fs from "node:fs";
import { ConfigManager } from "@orangecoding/config";
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
    lines.push("Providers: (config not found or unreadable)");
    return lines.join("\n") + "\n";
  }

  const providers = Object.keys(cfg.providers);

  if (providers.length === 0) {
    lines.push("Providers: (none configured)");
  } else {
    lines.push(`Providers: ${providers.join(", ")}`);
  }

  lines.push(`Default provider: ${cfg.default_provider}`);
  lines.push(`Default model: ${cfg.default_model}`);
  lines.push(`Control port: ${cfg.control_port}`);

  // Verify config file exists on disk
  if (!fs.existsSync(configPath)) {
    lines.push("Warning: config file does not exist on disk");
  }

  return lines.join("\n") + "\n";
}

/**
 * Handles the `status` command.
 * Displays the current OrangeCoding version, configuration path, and configured providers.
 */
export function runStatus(): void {
  const configPath = defaultConfigPath();
  const output = runStatusAtPath(configPath);
  process.stdout.write(output);
}
