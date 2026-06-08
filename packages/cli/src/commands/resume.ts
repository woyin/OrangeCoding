import * as os from "node:os";
import * as path from "node:path";
import { ResumeManager } from "@orangecoding/agent";
import { ConfigManager } from "@orangecoding/config";
import type { OrangeConfig } from "@orangecoding/config";
import { ProviderFactory, normalizeProviderConfig } from "@orangecoding/ai";
import type { ProviderConfig as AiProviderConfig } from "@orangecoding/ai";
import { createDefaultRegistry } from "@orangecoding/tools";
import { ToolExecutor } from "@orangecoding/agent";
import { aiProviderConfigFromCLIConfig } from "./launch.js";

export async function runResume(runID?: string): Promise<void> {
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

  const resumeMgr = new ResumeManager(checkpointDir);

  // If no runID, list resumable checkpoints
  if (!runID) {
    const checkpoints = await resumeMgr.listResumable();
    if (checkpoints.length === 0) {
      console.log("No resumable sessions found.");
      return;
    }
    console.log("Resumable sessions:\n");
    for (const cp of checkpoints) {
      console.log("  Run: " + cp.runID);
      console.log("    State: " + cp.state + " | Iteration: " + String(cp.iteration) + " | Tool calls: " + String(cp.toolCallsMade));
      console.log("    Task: " + cp.task.slice(0, 80));
      console.log("    Updated: " + cp.updatedAt);
      console.log();
    }
    console.log("Usage: orangecoding resume <run-id>");
    return;
  }

  // Resume specific run
  const canResume = await resumeMgr.canResume(runID);
  if (!canResume) {
    throw new Error("cannot resume run " + runID + ": not found or already terminal");
  }

  const providerName = cfg.default_provider || "openai";
  const providerConfig = aiProviderConfigFromCLIConfig(providerName, cfg);
  const factory = new ProviderFactory();
  const aiProvider = factory.createProvider(providerName, providerConfig);
  const registry = createDefaultRegistry();
  const executor = new ToolExecutor(registry);

  console.log("Resuming run: " + runID);
  const result = await resumeMgr.resume(runID, aiProvider, executor);
  console.log("Resumed: " + String(result.resumed));
  console.log("Tool calls: " + String(result.toolCallsMade));
  console.log("Stop reason: " + result.stopReason);
}

function defaultConfigPath(): string {
  return path.join(os.homedir(), ".orangecoding", "config.json");
}
