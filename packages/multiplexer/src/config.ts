/**
 * MultiplexerConfig holds settings for terminal multiplexer integration.
 */
export interface MultiplexerConfig {
  enabled: boolean;
  /** "zellij", "tmux", "auto" */
  preferredBackend: string;
  /** Directory for IPC sockets */
  socketDir: string;
  /** Per-command timeout in milliseconds */
  commandTimeoutMs: number;
}

/**
 * DefaultMultiplexerConfig returns a config with sensible defaults.
 */
export function defaultMultiplexerConfig(): MultiplexerConfig {
  return {
    enabled: false,
    preferredBackend: "auto",
    socketDir: defaultSocketDir(),
    commandTimeoutMs: 30000,
  };
}

/**
 * Normalize fills in zero-value fields with defaults.
 */
export function normalizeConfig(cfg: MultiplexerConfig): MultiplexerConfig {
  if (!cfg.preferredBackend) {
    cfg.preferredBackend = "auto";
  }
  if (!cfg.socketDir) {
    cfg.socketDir = defaultSocketDir();
  }
  if (!cfg.commandTimeoutMs) {
    cfg.commandTimeoutMs = 30000;
  }
  return cfg;
}

function defaultSocketDir(): string {
  const xdg = process.env.XDG_RUNTIME_DIR;
  if (xdg) {
    return `${xdg}/orangecoding/panes`;
  }
  return "/tmp/orangecoding/panes";
}
