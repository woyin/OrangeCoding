import { newConfigError, type OrangeError } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// ProviderConfig
// ---------------------------------------------------------------------------

export interface ProviderConfig {
  api_key: string;
  api_secret?: string;
  base_url?: string;
  default_model?: string;
  timeout_secs?: number;
  extra?: Record<string, string>;
}

// ---------------------------------------------------------------------------
// HooksConfig
// ---------------------------------------------------------------------------

export interface HooksConfig {
  pre_tool_call?: string[];
  post_tool_call?: string[];
}

// ---------------------------------------------------------------------------
// PermissionsConfig
// ---------------------------------------------------------------------------

export interface PermissionsConfig {
  bash?: string;
  write?: string;
  edit?: string;
  read?: string;
  execute?: string;
}

// ---------------------------------------------------------------------------
// HarnessConfig
// ---------------------------------------------------------------------------

export interface HarnessConfig {
  checkpoint_store: string;
  checkpoint_dir: string;
  reasoning_effort: string;
  reasoning_budget_tokens: number;
}

// ---------------------------------------------------------------------------
// MultiplexerConfig
// ---------------------------------------------------------------------------

export interface MultiplexerConfig {
  enabled: boolean;
  preferred_backend: string;
  socket_dir: string;
  command_timeout_ms: number;
}

// ---------------------------------------------------------------------------
// AuditConfig
// ---------------------------------------------------------------------------

export interface AuditConfig {
  enabled: boolean;
  dir: string;
}

// ---------------------------------------------------------------------------
// OrangeConfig
// ---------------------------------------------------------------------------

export interface OrangeConfig {
  log_level: string;
  default_provider: string;
  default_model: string;
  control_port: number;
  providers: Record<string, ProviderConfig>;
  hooks: HooksConfig;
  permissions: PermissionsConfig;
  harness: HarnessConfig;
  multiplexer: MultiplexerConfig;
  audit: AuditConfig;
  skills?: SkillsConfig;
}

// ---------------------------------------------------------------------------
// SkillsConfig
// ---------------------------------------------------------------------------

export interface SkillDefinition {
  name: string;
  description?: string;
  tools?: string[];
  prompt?: string;
  tags?: string[];
  examples?: string[];
}

export interface SkillsConfig {
  custom?: SkillDefinition[];
  default?: string;
  auto_detect?: boolean;
}

// ---------------------------------------------------------------------------
// Validate
// ---------------------------------------------------------------------------

export function validateConfig(cfg: OrangeConfig): void {
  if (cfg.control_port < 0 || cfg.control_port > 65535) {
    throw newConfigError(
      `invalid control_port: ${cfg.control_port} (must be 0-65535)`,
    );
  }

  const validStores = new Set(["", "memory", "file"]);
  if (!validStores.has(cfg.harness.checkpoint_store)) {
    throw newConfigError(
      `invalid harness.checkpoint_store: "${cfg.harness.checkpoint_store}" (must be memory or file)`,
    );
  }

  const validBackends = new Set(["", "auto", "zellij", "tmux"]);
  if (!validBackends.has(cfg.multiplexer.preferred_backend)) {
    throw newConfigError(
      `invalid multiplexer.preferred_backend: "${cfg.multiplexer.preferred_backend}" (must be auto, zellij, or tmux)`,
    );
  }
}
