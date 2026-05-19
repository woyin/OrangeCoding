package multiplexer

import "os"

// MultiplexerConfig holds settings for terminal multiplexer integration.
type MultiplexerConfig struct {
	Enabled          bool   `json:"enabled"`
	PreferredBackend string `json:"preferred_backend"`  // "zellij", "tmux", "auto"
	SocketDir        string `json:"socket_dir"`         // directory for IPC sockets
	CommandTimeoutMs int    `json:"command_timeout_ms"` // per-command timeout
}

// DefaultMultiplexerConfig returns a config with sensible defaults.
func DefaultMultiplexerConfig() MultiplexerConfig {
	return MultiplexerConfig{
		Enabled:          false,
		PreferredBackend: "auto",
		SocketDir:        defaultSocketDir(),
		CommandTimeoutMs: 30000,
	}
}

// Normalize fills in zero-value fields with defaults.
func (c *MultiplexerConfig) Normalize() {
	if c.PreferredBackend == "" {
		c.PreferredBackend = "auto"
	}
	if c.SocketDir == "" {
		c.SocketDir = defaultSocketDir()
	}
	if c.CommandTimeoutMs == 0 {
		c.CommandTimeoutMs = 30000
	}
}

func defaultSocketDir() string {
	if dir := os.Getenv("XDG_RUNTIME_DIR"); dir != "" {
		return dir + "/orangecoding/panes"
	}
	return "/tmp/orangecoding/panes"
}
