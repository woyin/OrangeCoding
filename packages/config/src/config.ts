import * as fs from "node:fs";
import * as path from "node:path";
import { newConfigError, newIOError } from "@orangecoding/core";
import { parseJSONC } from "./jsonc.js";
import {
  validateConfig,
  type OrangeConfig,
  type ProviderConfig,
  type HarnessConfig,
  type MultiplexerConfig,
} from "./types.js";

// ---------------------------------------------------------------------------
// DefaultConfig
// ---------------------------------------------------------------------------

/**
 * DefaultConfig returns an OrangeConfig with sensible defaults.
 */
export function defaultConfig(): OrangeConfig {
  return {
    log_level: "info",
    default_provider: "openai",
    default_model: "",
    control_port: 3200,
    providers: {},
    hooks: {},
    permissions: {},
    harness: {
      checkpoint_store: "memory",
      checkpoint_dir: "checkpoints",
      reasoning_effort: "high",
      reasoning_budget_tokens: 4096,
    },
    multiplexer: {
      enabled: false,
      preferred_backend: "auto",
      socket_dir: "",
      command_timeout_ms: 30000,
    },
  };
}

// ---------------------------------------------------------------------------
// ConfigManager
// ---------------------------------------------------------------------------

/**
 * ConfigManager handles loading, saving, and querying configuration files.
 */
export class ConfigManager {
  /**
   * Load reads a configuration file, strips JSONC comments, and parses it.
   */
  load(filePath: string): OrangeConfig {
    let raw: string;
    try {
      raw = fs.readFileSync(filePath, "utf-8");
    } catch (err) {
      throw newIOError(
        `read config file ${filePath}: ${err instanceof Error ? err.message : String(err)}`,
      );
    }

    const clean = parseJSONC(raw);

    let parsed: Record<string, unknown>;
    try {
      parsed = JSON.parse(clean) as Record<string, unknown>;
    } catch (err) {
      throw newConfigError(
        `parse config from ${filePath}: ${err instanceof Error ? err.message : String(err)}`,
      );
    }

    const cfg = rawToConfig(parsed);

    normalizeHarnessConfig(cfg.harness);
    normalizeMultiplexerConfig(cfg.multiplexer);
    expandConfigEnv(cfg);

    validateConfig(cfg);

    return cfg;
  }

  /**
   * Save marshals the configuration and writes it to disk, creating parent
   * directories as needed.
   */
  save(filePath: string, cfg: OrangeConfig): void {
    const data = JSON.stringify(cfg, null, 2);

    const dir = path.dirname(filePath);
    fs.mkdirSync(dir, { recursive: true });

    try {
      fs.writeFileSync(filePath, data, "utf-8");
    } catch (err) {
      throw newIOError(
        `write config file ${filePath}: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  }

  /**
   * Get loads the config file and returns the value of the named field.
   * Supports dot-separated paths for nested fields (e.g. "harness.checkpoint_store").
   */
  get(filePath: string, key: string): unknown {
    const cfg = this.load(filePath);
    return getByPath(cfg, key);
  }

  /**
   * Set loads the config file, updates the named field, and saves it back.
   * Supports dot-separated paths for nested fields.
   */
  set(filePath: string, key: string, value: unknown): void {
    const cfg = this.load(filePath);
    setByPath(cfg, key, value);
    this.save(filePath, cfg);
  }
}

// ---------------------------------------------------------------------------
// Internal: raw-to-config conversion
// ---------------------------------------------------------------------------

function rawToConfig(raw: Record<string, unknown>): OrangeConfig {
  const providers: Record<string, ProviderConfig> = {};
  const rawProviders = raw["providers"];
  if (rawProviders && typeof rawProviders === "object" && !Array.isArray(rawProviders)) {
    for (const [name, val] of Object.entries(
      rawProviders as Record<string, Record<string, unknown>>,
    )) {
      providers[name] = {
        api_key: asString(val["api_key"], ""),
        api_secret: asOptionalString(val["api_secret"]),
        base_url: asOptionalString(val["base_url"]),
        default_model: asOptionalString(val["default_model"]),
        timeout_secs: asOptionalNumber(val["timeout_secs"]),
        extra: asOptionalRecord(val["extra"]),
      };
    }
  }

  const rawHooks = asObject(raw["hooks"]);
  const rawPermissions = asObject(raw["permissions"]);
  const rawHarness = asObject(raw["harness"]);
  const rawMultiplexer = asObject(raw["multiplexer"]);

  return {
    log_level: asString(raw["log_level"], "info"),
    default_provider: asString(raw["default_provider"], "openai"),
    default_model: asString(raw["default_model"], ""),
    control_port: asNumber(raw["control_port"], 0),
    providers,
    hooks: {
      pre_tool_call: asOptionalStringArray(rawHooks["pre_tool_call"]),
      post_tool_call: asOptionalStringArray(rawHooks["post_tool_call"]),
    },
    permissions: {
      bash: asOptionalString(rawPermissions["bash"]),
      write: asOptionalString(rawPermissions["write"]),
      edit: asOptionalString(rawPermissions["edit"]),
      read: asOptionalString(rawPermissions["read"]),
      execute: asOptionalString(rawPermissions["execute"]),
    },
    harness: {
      checkpoint_store: asString(rawHarness["checkpoint_store"], ""),
      checkpoint_dir: asString(rawHarness["checkpoint_dir"], ""),
      reasoning_effort: asString(rawHarness["reasoning_effort"], ""),
      reasoning_budget_tokens: asNumber(rawHarness["reasoning_budget_tokens"], 0),
    },
    multiplexer: {
      enabled: asBoolean(rawMultiplexer["enabled"], false),
      preferred_backend: asString(rawMultiplexer["preferred_backend"], ""),
      socket_dir: asString(rawMultiplexer["socket_dir"], ""),
      command_timeout_ms: asNumber(rawMultiplexer["command_timeout_ms"], 0),
    },
  };
}

// ---------------------------------------------------------------------------
// Internal: JSON path helpers (equivalent to Go's fieldByJSONTag/fieldByJSONPath)
// ---------------------------------------------------------------------------

function getByPath(obj: unknown, key: string): unknown {
  const parts = key.split(".");
  let current: unknown = obj;

  for (const part of parts) {
    if (part === "") {
      throw newConfigError(`unknown config field: ${key}`);
    }
    if (current === null || current === undefined || typeof current !== "object" || Array.isArray(current)) {
      throw newConfigError(`config field ${part} is not an object`);
    }
    current = (current as Record<string, unknown>)[part];
    if (current === undefined) {
      throw newConfigError(`unknown config field: ${key}`);
    }
  }

  return current;
}

function setByPath(obj: unknown, key: string, value: unknown): void {
  const parts = key.split(".");
  let current: unknown = obj;

  for (let i = 0; i < parts.length - 1; i++) {
    const part = parts[i]!;
    if (part === "") {
      throw newConfigError(`unknown config field: ${key}`);
    }
    if (current === null || current === undefined || typeof current !== "object" || Array.isArray(current)) {
      throw newConfigError(`config field ${part} is not an object`);
    }
    const next = (current as Record<string, unknown>)[part];
    if (next === undefined) {
      throw newConfigError(`unknown config field: ${key}`);
    }
    current = next;
  }

  const lastPart = parts[parts.length - 1]!;
  if (current === null || current === undefined || typeof current !== "object" || Array.isArray(current)) {
    throw newConfigError(`cannot set field on non-object`);
  }
  (current as Record<string, unknown>)[lastPart] = value;
}

// ---------------------------------------------------------------------------
// Internal: normalization
// ---------------------------------------------------------------------------

function normalizeHarnessConfig(cfg: HarnessConfig): void {
  if (cfg.checkpoint_store === "") {
    cfg.checkpoint_store = "memory";
  }
  if (cfg.checkpoint_dir === "") {
    cfg.checkpoint_dir = "checkpoints";
  }
  if (cfg.reasoning_effort === "") {
    cfg.reasoning_effort = "high";
  }
  if (cfg.reasoning_budget_tokens === 0) {
    cfg.reasoning_budget_tokens = 4096;
  }
}

function normalizeMultiplexerConfig(cfg: MultiplexerConfig): void {
  if (cfg.preferred_backend === "") {
    cfg.preferred_backend = "auto";
  }
  if (cfg.command_timeout_ms === 0) {
    cfg.command_timeout_ms = 30000;
  }
  if (cfg.command_timeout_ms < 1000) {
    cfg.command_timeout_ms = 1000;
  }
}

// ---------------------------------------------------------------------------
// Internal: environment variable expansion
// ---------------------------------------------------------------------------

function expandConfigEnv(cfg: OrangeConfig): void {
  cfg.log_level = expandEnv(cfg.log_level);
  cfg.default_provider = expandEnv(cfg.default_provider);
  cfg.default_model = expandEnv(cfg.default_model);

  for (const [name, provider] of Object.entries(cfg.providers)) {
    provider.api_key = expandEnv(provider.api_key);
    if (provider.api_secret !== undefined) {
      provider.api_secret = expandEnv(provider.api_secret);
    }
    if (provider.base_url !== undefined) {
      provider.base_url = expandEnv(provider.base_url);
    }
    if (provider.default_model !== undefined) {
      provider.default_model = expandEnv(provider.default_model);
    }
    if (provider.extra) {
      for (const [key, value] of Object.entries(provider.extra)) {
        provider.extra[key] = expandEnv(value);
      }
    }
    cfg.providers[name] = provider;
  }

  if (cfg.hooks.pre_tool_call) {
    cfg.hooks.pre_tool_call = cfg.hooks.pre_tool_call.map(expandEnv);
  }
  if (cfg.hooks.post_tool_call) {
    cfg.hooks.post_tool_call = cfg.hooks.post_tool_call.map(expandEnv);
  }

  if (cfg.permissions.bash !== undefined) {
    cfg.permissions.bash = expandEnv(cfg.permissions.bash);
  }
  if (cfg.permissions.write !== undefined) {
    cfg.permissions.write = expandEnv(cfg.permissions.write);
  }
  if (cfg.permissions.edit !== undefined) {
    cfg.permissions.edit = expandEnv(cfg.permissions.edit);
  }
  if (cfg.permissions.read !== undefined) {
    cfg.permissions.read = expandEnv(cfg.permissions.read);
  }
  if (cfg.permissions.execute !== undefined) {
    cfg.permissions.execute = expandEnv(cfg.permissions.execute);
  }

  cfg.multiplexer.socket_dir = expandEnv(cfg.multiplexer.socket_dir);
}

/**
 * expandEnv replaces $VAR and ${VAR} references in the string with the
 * corresponding environment variable values. Mirrors Go's os.ExpandEnv.
 */
function expandEnv(s: string): string {
  return s.replace(/\$\{([^}]+)\}|\$([A-Za-z_][A-Za-z0-9_]*)/g, (_, braced, bare) => {
    const name = braced ?? bare;
    return process.env[name] ?? "";
  });
}

// ---------------------------------------------------------------------------
// Internal: type-safe JSON value extraction helpers
// ---------------------------------------------------------------------------

function asString(val: unknown, fallback: string): string {
  return typeof val === "string" ? val : fallback;
}

function asNumber(val: unknown, fallback: number): number {
  return typeof val === "number" ? val : fallback;
}

function asBoolean(val: unknown, fallback: boolean): boolean {
  return typeof val === "boolean" ? val : fallback;
}

function asOptionalString(val: unknown): string | undefined {
  return typeof val === "string" ? val : undefined;
}

function asOptionalNumber(val: unknown): number | undefined {
  return typeof val === "number" ? val : undefined;
}

function asOptionalStringArray(val: unknown): string[] | undefined {
  if (Array.isArray(val) && val.every((v) => typeof v === "string")) {
    return val as string[];
  }
  return undefined;
}

function asOptionalRecord(val: unknown): Record<string, string> | undefined {
  if (val && typeof val === "object" && !Array.isArray(val)) {
    const rec = val as Record<string, unknown>;
    if (Object.values(rec).every((v) => typeof v === "string")) {
      return rec as Record<string, string>;
    }
  }
  return undefined;
}

function asObject(val: unknown): Record<string, unknown> {
  if (val && typeof val === "object" && !Array.isArray(val)) {
    return val as Record<string, unknown>;
  }
  return {};
}
