/**
 * @module cli-analyze
 *
 * CLI command for analyzing agent sessions.
 *
 * Provides post-session analysis including:
 * - Token usage breakdown and cost estimation
 * - Tool usage statistics
 * - Performance metrics
 * - Session timeline visualization
 */
import * as os from "node:os";
import * as path from "node:path";
import { SessionAnalyzer } from "@orangecoding/agent";
import { ConfigManager } from "@orangecoding/config";
import type { OrangeConfig } from "@orangecoding/config";

export async function runAnalyze(): Promise<void> {
  const configPath = defaultConfigPath();
  const mgr = new ConfigManager();
  let cfg: OrangeConfig;
  try {
    cfg = mgr.load(configPath);
  } catch {
    throw new Error("failed to load config. Run 'orangecoding init' first.");
  }

  const checkpointDir = cfg.harness?.checkpoint_dir
    ?? path.join(os.homedir(), ".orangecoding", "checkpoints");

  const analyzer = new SessionAnalyzer(checkpointDir);

  console.log("Analyzing sessions...\n");
  const report = await analyzer.analyze();

  if (report.sessionCount === 0) {
    console.log("No sessions found to analyze.");
    return;
  }

  console.log(analyzer.formatReport(report));
}

function defaultConfigPath(): string {
  return path.join(os.homedir(), ".orangecoding", "config.json");
}
