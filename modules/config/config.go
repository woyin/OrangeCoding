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

// DefaultConfig returns an OrangeConfig with sensible defaults.
func DefaultConfig() OrangeConfig {
	return OrangeConfig{
		LogLevel:        "info",
		DefaultProvider: "openai",
		ControlPort:     3200,
		Providers:       make(map[string]ProviderConfig),
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

// Get loads the config file and returns the value of the named field.
func (m *ConfigManager) Get(path, key string) (interface{}, error) {
	cfg, err := m.Load(path)
	if err != nil {
		return nil, err
	}

	v := reflect.ValueOf(*cfg)
	field, err := fieldByJSONTag(v, key)
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
	field, err := fieldByJSONTag(v, key)
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
