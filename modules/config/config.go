package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
)

// OrangeConfig holds the top-level application configuration.
type OrangeConfig struct {
	LogLevel        string                    `json:"log_level"`
	DefaultProvider string                    `json:"default_provider"`
	DefaultModel    string                    `json:"default_model"`
	ControlPort     int                       `json:"control_port"`
	Providers       map[string]ProviderConfig `json:"providers"`
	Hooks           HooksConfig               `json:"hooks"`
	Permissions     PermissionsConfig         `json:"permissions"`
	Harness         HarnessConfig             `json:"harness"`
	Multiplexer     MultiplexerConfig         `json:"multiplexer"`
}

// ProviderConfig holds provider-specific credentials and settings.
type ProviderConfig struct {
	APIKey       string            `json:"api_key"`
	APISecret    string            `json:"api_secret,omitempty"`
	BaseURL      string            `json:"base_url,omitempty"`
	DefaultModel string            `json:"default_model,omitempty"`
	TimeoutSecs  uint64            `json:"timeout_secs,omitempty"`
	Extra        map[string]string `json:"extra,omitempty"`
}

// HooksConfig defines optional hook commands.
type HooksConfig struct {
	PreToolCall  []string `json:"pre_tool_call,omitempty"`
	PostToolCall []string `json:"post_tool_call,omitempty"`
}

// PermissionsConfig defines permission policies per tool category.
type PermissionsConfig struct {
	Bash    string `json:"bash,omitempty"`
	Write   string `json:"write,omitempty"`
	Edit    string `json:"edit,omitempty"`
	Read    string `json:"read,omitempty"`
	Execute string `json:"execute,omitempty"`
}

// HarnessConfig defines first-version harness runtime persistence settings.
type HarnessConfig struct {
	CheckpointStore       string `json:"checkpoint_store"`
	CheckpointDir         string `json:"checkpoint_dir"`
	ReasoningEffort       string `json:"reasoning_effort"`
	ReasoningBudgetTokens uint32 `json:"reasoning_budget_tokens"`
}

// MultiplexerConfig holds terminal multiplexer integration settings.
type MultiplexerConfig struct {
	Enabled          bool   `json:"enabled"`
	PreferredBackend string `json:"preferred_backend"`  // "zellij", "tmux", "auto"
	SocketDir        string `json:"socket_dir"`
	CommandTimeoutMs int    `json:"command_timeout_ms"`
}

// DefaultConfig returns an OrangeConfig with sensible defaults.
func DefaultConfig() OrangeConfig {
	return OrangeConfig{
		LogLevel:        "info",
		DefaultProvider: "openai",
		ControlPort:     3200,
		Providers:       make(map[string]ProviderConfig),
		Harness: HarnessConfig{
			CheckpointStore:       "memory",
			CheckpointDir:         "checkpoints",
			ReasoningEffort:       "high",
			ReasoningBudgetTokens: 4096,
		},
	}
}

// ConfigManager handles loading, saving, and querying configuration files.
type ConfigManager struct{}

// NewConfigManager creates a new ConfigManager.
func NewConfigManager() *ConfigManager {
	return &ConfigManager{}
}

// Load reads a configuration file, strips JSONC comments, and unmarshals it.
func (m *ConfigManager) Load(path string) (*OrangeConfig, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read config file %s: %w", path, err)
	}

	clean, err := ParseJSONC(string(raw))
	if err != nil {
		return nil, fmt.Errorf("parse JSONC in %s: %w", path, err)
	}

	var cfg OrangeConfig
	if err := json.Unmarshal([]byte(clean), &cfg); err != nil {
		return nil, fmt.Errorf("unmarshal config from %s: %w", path, err)
	}

	if cfg.Providers == nil {
		cfg.Providers = make(map[string]ProviderConfig)
	}
	normalizeHarnessConfig(&cfg.Harness)
	normalizeMultiplexerConfig(&cfg.Multiplexer)
	expandConfigEnv(&cfg)

	if err := cfg.Validate(); err != nil {
		return nil, fmt.Errorf("config validation: %w", err)
	}

	return &cfg, nil
}

// Save marshals the configuration and writes it to disk, creating parent directories as needed.
func (m *ConfigManager) Save(path string, cfg *OrangeConfig) error {
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal config: %w", err)
	}

	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("create config directory %s: %w", dir, err)
	}

	if err := os.WriteFile(path, data, 0644); err != nil {
		return fmt.Errorf("write config file %s: %w", path, err)
	}

	return nil
}

// fieldByJSONTag looks up a struct field by its JSON tag name on the OrangeConfig type.
func fieldByJSONTag(v reflect.Value, key string) (reflect.Value, error) {
	t := v.Type()
	for i := 0; i < t.NumField(); i++ {
		field := t.Field(i)
		tag := field.Tag.Get("json")
		// Extract the tag name (before any comma for omitempty etc.)
		tagName := tag
		if idx := strings.Index(tag, ","); idx != -1 {
			tagName = tag[:idx]
		}
		if tagName == key {
			return v.Field(i), nil
		}
	}
	return reflect.Value{}, fmt.Errorf("unknown config field: %s", key)
}

func fieldByJSONPath(v reflect.Value, key string) (reflect.Value, error) {
	current := v
	for _, part := range strings.Split(key, ".") {
		if part == "" {
			return reflect.Value{}, fmt.Errorf("unknown config field: %s", key)
		}
		for current.Kind() == reflect.Pointer {
			if current.IsNil() {
				current.Set(reflect.New(current.Type().Elem()))
			}
			current = current.Elem()
		}
		if current.Kind() != reflect.Struct {
			return reflect.Value{}, fmt.Errorf("config field %s is not a struct", part)
		}
		next, err := fieldByJSONTag(current, part)
		if err != nil {
			return reflect.Value{}, err
		}
		current = next
	}
	return current, nil
}

// Get loads the config file and returns the value of the named field.
func (m *ConfigManager) Get(path, key string) (interface{}, error) {
	cfg, err := m.Load(path)
	if err != nil {
		return nil, err
	}

	v := reflect.ValueOf(*cfg)
	field, err := fieldByJSONPath(v, key)
	if err != nil {
		return nil, err
	}

	return field.Interface(), nil
}

// Set loads the config file, updates the named field, and saves it back.
func (m *ConfigManager) Set(path, key string, value interface{}) error {
	cfg, err := m.Load(path)
	if err != nil {
		return err
	}

	v := reflect.ValueOf(cfg).Elem()
	field, err := fieldByJSONPath(v, key)
	if err != nil {
		return err
	}

	val := reflect.ValueOf(value)
	if !val.Type().AssignableTo(field.Type()) {
		// Attempt common conversion: float64 (from JSON) to int
		if field.Kind() == reflect.Int {
			switch n := value.(type) {
			case float64:
				field.SetInt(int64(n))
				return m.Save(path, cfg)
			case int:
				field.SetInt(int64(n))
				return m.Save(path, cfg)
			}
		}
		return fmt.Errorf("cannot assign %T to field %s (%s)", value, key, field.Type())
	}

	field.Set(val)
	return m.Save(path, cfg)
}

func normalizeHarnessConfig(cfg *HarnessConfig) {
	if cfg.CheckpointStore == "" {
		cfg.CheckpointStore = "memory"
	}
	if cfg.CheckpointDir == "" {
		cfg.CheckpointDir = "checkpoints"
	}
	if cfg.ReasoningEffort == "" {
		cfg.ReasoningEffort = "high"
	}
	if cfg.ReasoningBudgetTokens == 0 {
		cfg.ReasoningBudgetTokens = 4096
	}
}

func normalizeMultiplexerConfig(cfg *MultiplexerConfig) {
	if cfg.PreferredBackend == "" {
		cfg.PreferredBackend = "auto"
	}
	if cfg.CommandTimeoutMs == 0 {
		cfg.CommandTimeoutMs = 30000
	}
	if cfg.CommandTimeoutMs < 1000 {
		cfg.CommandTimeoutMs = 1000
	}
}

// Validate checks the config for invalid values and returns an error if found.
func (c *OrangeConfig) Validate() error {
	if c.ControlPort < 0 || c.ControlPort > 65535 {
		return fmt.Errorf("invalid control_port: %d (must be 0-65535)", c.ControlPort)
	}
	validStores := map[string]bool{"": true, "memory": true, "file": true}
	if !validStores[c.Harness.CheckpointStore] {
		return fmt.Errorf("invalid harness.checkpoint_store: %q (must be memory or file)", c.Harness.CheckpointStore)
	}
	validBackends := map[string]bool{"": true, "auto": true, "zellij": true, "tmux": true}
	if !validBackends[c.Multiplexer.PreferredBackend] {
		return fmt.Errorf("invalid multiplexer.preferred_backend: %q (must be auto, zellij, or tmux)", c.Multiplexer.PreferredBackend)
	}
	return nil
}

func expandConfigEnv(cfg *OrangeConfig) {
	cfg.LogLevel = os.ExpandEnv(cfg.LogLevel)
	cfg.DefaultProvider = os.ExpandEnv(cfg.DefaultProvider)
	cfg.DefaultModel = os.ExpandEnv(cfg.DefaultModel)

	for name, provider := range cfg.Providers {
		provider.APIKey = os.ExpandEnv(provider.APIKey)
		provider.APISecret = os.ExpandEnv(provider.APISecret)
		provider.BaseURL = os.ExpandEnv(provider.BaseURL)
		provider.DefaultModel = os.ExpandEnv(provider.DefaultModel)
		for key, value := range provider.Extra {
			provider.Extra[key] = os.ExpandEnv(value)
		}
		cfg.Providers[name] = provider
	}

	cfg.Hooks.PreToolCall = expandStringSliceEnv(cfg.Hooks.PreToolCall)
	cfg.Hooks.PostToolCall = expandStringSliceEnv(cfg.Hooks.PostToolCall)
	cfg.Permissions.Bash = os.ExpandEnv(cfg.Permissions.Bash)
	cfg.Permissions.Write = os.ExpandEnv(cfg.Permissions.Write)
	cfg.Permissions.Edit = os.ExpandEnv(cfg.Permissions.Edit)
	cfg.Permissions.Read = os.ExpandEnv(cfg.Permissions.Read)
	cfg.Permissions.Execute = os.ExpandEnv(cfg.Permissions.Execute)
	cfg.Multiplexer.SocketDir = os.ExpandEnv(cfg.Multiplexer.SocketDir)
}

func expandStringSliceEnv(values []string) []string {
	for i, value := range values {
		values[i] = os.ExpandEnv(value)
	}
	return values
}
