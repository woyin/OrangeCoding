import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { ConfigManager, defaultConfig } from "@orangecoding/config";

/**
 * Returns the default configuration file path (~/.orangecoding/config.json).
 */
export function defaultConfigPath(): string {
  const home = os.homedir() || ".";
  return path.join(home, ".orangecoding", "config.json");
}

/**
 * Creates the default configuration file at the given path.
 * Throws if the file already exists.
 */
export function runInitAtPath(configPath: string): void {
  if (fs.existsSync(configPath)) {
    throw new Error(`config file already exists at ${configPath}`);
  }

  const mgr = new ConfigManager();
  const cfg = defaultConfig();
  mgr.save(configPath, cfg);
}

/**
 * Handles the `init` command.
 * Creates a default configuration file at ~/.orangecoding/config.json.
 */
export function runInit(): void {
  const configPath = defaultConfigPath();
  runInitAtPath(configPath);
  console.log(`Configuration created at ${configPath}`);
}
