package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

// --- DefaultConfig tests ---

func TestDefaultConfig(t *testing.T) {
	cfg := DefaultConfig()

	if cfg.LogLevel != "info" {
		t.Errorf("DefaultConfig LogLevel = %q, want %q", cfg.LogLevel, "info")
	}
	if cfg.ControlPort != 3200 {
		t.Errorf("DefaultConfig ControlPort = %d, want %d", cfg.ControlPort, 3200)
	}
	if cfg.DefaultProvider != "openai" {
		t.Errorf("DefaultConfig DefaultProvider = %q, want %q", cfg.DefaultProvider, "openai")
	}
	if cfg.DefaultModel != "" {
		t.Errorf("DefaultConfig DefaultModel = %q, want empty string", cfg.DefaultModel)
	}
	if cfg.Providers == nil {
		t.Error("DefaultConfig Providers is nil, want non-nil map")
	}
	if cfg.Harness.CheckpointStore != "memory" {
		t.Errorf("DefaultConfig Harness.CheckpointStore = %q, want memory", cfg.Harness.CheckpointStore)
	}
	if cfg.Harness.CheckpointDir != "checkpoints" {
		t.Errorf("DefaultConfig Harness.CheckpointDir = %q, want checkpoints", cfg.Harness.CheckpointDir)
	}
	if cfg.Harness.ReasoningEffort != "high" {
		t.Errorf("DefaultConfig Harness.ReasoningEffort = %q, want high", cfg.Harness.ReasoningEffort)
	}
	if cfg.Harness.ReasoningBudgetTokens != 4096 {
		t.Errorf("DefaultConfig Harness.ReasoningBudgetTokens = %d, want 4096", cfg.Harness.ReasoningBudgetTokens)
	}
}

// --- ConfigManager Load/Save round-trip ---

func TestConfigManagerLoadSaveRoundTrip(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.jsonc")

	mgr := NewConfigManager()

	cfg := DefaultConfig()
	cfg.Providers["openai"] = ProviderConfig{
		APIKey:  "sk-test-key",
		BaseURL: "https://api.openai.com/v1",
	}

	// Save
	if err := mgr.Save(path, &cfg); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Verify file exists
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("Save did not create file")
	}

	// Load
	loaded, err := mgr.Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if loaded.LogLevel != cfg.LogLevel {
		t.Errorf("LogLevel mismatch: got %q, want %q", loaded.LogLevel, cfg.LogLevel)
	}
	if loaded.ControlPort != cfg.ControlPort {
		t.Errorf("ControlPort mismatch: got %d, want %d", loaded.ControlPort, cfg.ControlPort)
	}
	if loaded.Providers["openai"].APIKey != "sk-test-key" {
		t.Errorf("Provider APIKey mismatch: got %q", loaded.Providers["openai"].APIKey)
	}
	if loaded.Providers["openai"].BaseURL != "https://api.openai.com/v1" {
		t.Errorf("Provider BaseURL mismatch: got %q", loaded.Providers["openai"].BaseURL)
	}
}

func TestConfigManagerLoadJSONC(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.jsonc")

	// Write a file with comments
	content := `{
		// This is a line comment
		"log_level": "debug",
		"control_port": 9999,
		"providers": {}
	}`
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	mgr := NewConfigManager()
	cfg, err := mgr.Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if cfg.LogLevel != "debug" {
		t.Errorf("LogLevel = %q, want %q", cfg.LogLevel, "debug")
	}
	if cfg.ControlPort != 9999 {
		t.Errorf("ControlPort = %d, want %d", cfg.ControlPort, 9999)
	}
}

func TestConfigManagerExpandsProviderEnvironmentVariables(t *testing.T) {
	t.Setenv("ORANGECODING_TEST_OPENAI_KEY", "sk-from-env")
	dir := t.TempDir()
	path := filepath.Join(dir, "config.jsonc")

	content := `{
		"default_provider": "openai",
		"providers": {
			"openai": {
				"api_key": "${ORANGECODING_TEST_OPENAI_KEY}",
				"base_url": "https://api.openai.com/v1"
			}
		}
	}`
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	mgr := NewConfigManager()
	cfg, err := mgr.Load(path)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}
	if cfg.Providers["openai"].APIKey != "sk-from-env" {
		t.Fatalf("APIKey = %q, want sk-from-env", cfg.Providers["openai"].APIKey)
	}
}

// --- ConfigManager Set/Get ---

func TestConfigManagerSetGet(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.jsonc")

	mgr := NewConfigManager()

	cfg := DefaultConfig()
	if err := mgr.Save(path, &cfg); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Set a value
	if err := mgr.Set(path, "log_level", "debug"); err != nil {
		t.Fatalf("Set failed: %v", err)
	}

	// Get the value back
	val, err := mgr.Get(path, "log_level")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}

	strVal, ok := val.(string)
	if !ok {
		t.Fatalf("Get returned %T, want string", val)
	}
	if strVal != "debug" {
		t.Errorf("Get log_level = %q, want %q", strVal, "debug")
	}

	// Set control_port (int via float64 from JSON)
	if err := mgr.Set(path, "control_port", float64(5000)); err != nil {
		t.Fatalf("Set control_port failed: %v", err)
	}

	val, err = mgr.Get(path, "control_port")
	if err != nil {
		t.Fatalf("Get control_port failed: %v", err)
	}
	numVal, ok := val.(int)
	if !ok {
		t.Fatalf("Get returned %T, want int", val)
	}
	if numVal != 5000 {
		t.Errorf("Get control_port = %v, want %v", numVal, 5000)
	}
}

func TestConfigManagerSetGetNestedHarnessField(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.jsonc")

	mgr := NewConfigManager()

	cfg := DefaultConfig()
	if err := mgr.Save(path, &cfg); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	if err := mgr.Set(path, "harness.checkpoint_store", "file"); err != nil {
		t.Fatalf("Set harness.checkpoint_store failed: %v", err)
	}
	if err := mgr.Set(path, "harness.checkpoint_dir", "runtime-checkpoints"); err != nil {
		t.Fatalf("Set harness.checkpoint_dir failed: %v", err)
	}

	store, err := mgr.Get(path, "harness.checkpoint_store")
	if err != nil {
		t.Fatalf("Get harness.checkpoint_store failed: %v", err)
	}
	if store != "file" {
		t.Errorf("harness.checkpoint_store = %v, want file", store)
	}

	checkpointDir, err := mgr.Get(path, "harness.checkpoint_dir")
	if err != nil {
		t.Fatalf("Get harness.checkpoint_dir failed: %v", err)
	}
	if checkpointDir != "runtime-checkpoints" {
		t.Errorf("harness.checkpoint_dir = %v, want runtime-checkpoints", checkpointDir)
	}
}

func TestConfigManagerGetUnknownKey(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.jsonc")

	mgr := NewConfigManager()
	cfg := DefaultConfig()
	if err := mgr.Save(path, &cfg); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	_, err := mgr.Get(path, "nonexistent_field")
	if err == nil {
		t.Error("Get with unknown key should return error")
	}
}

// --- JSONC Parser tests ---

func TestParseJSONCLineComments(t *testing.T) {
	input := `{
		// line comment
		"key": "value"
	}`
	result, err := ParseJSONC(input)
	if err != nil {
		t.Fatalf("ParseJSONC failed: %v", err)
	}

	// Verify the result is valid JSON
	var m map[string]interface{}
	if err := json.Unmarshal([]byte(result), &m); err != nil {
		t.Fatalf("Result is not valid JSON: %v\nResult: %s", err, result)
	}
	if m["key"] != "value" {
		t.Errorf("key = %v, want %q", m["key"], "value")
	}
}

func TestParseJSONCBlockComments(t *testing.T) {
	input := `{
		/* block comment */
		"key": "value"
	}`
	result, err := ParseJSONC(input)
	if err != nil {
		t.Fatalf("ParseJSONC failed: %v", err)
	}

	var m map[string]interface{}
	if err := json.Unmarshal([]byte(result), &m); err != nil {
		t.Fatalf("Result is not valid JSON: %v\nResult: %s", err, result)
	}
	if m["key"] != "value" {
		t.Errorf("key = %v, want %q", m["key"], "value")
	}
}

func TestParseJSONCMultiLineBlockComment(t *testing.T) {
	input := `{
		/*
		 * multi-line
		 * block comment
		 */
		"key": "value"
	}`
	result, err := ParseJSONC(input)
	if err != nil {
		t.Fatalf("ParseJSONC failed: %v", err)
	}

	var m map[string]interface{}
	if err := json.Unmarshal([]byte(result), &m); err != nil {
		t.Fatalf("Result is not valid JSON: %v\nResult: %s", err, result)
	}
	if m["key"] != "value" {
		t.Errorf("key = %v, want %q", m["key"], "value")
	}
}

func TestParseJSONCCommentsInStrings(t *testing.T) {
	input := `{
		"url": "https://example.com/path"
	}`
	result, err := ParseJSONC(input)
	if err != nil {
		t.Fatalf("ParseJSONC failed: %v", err)
	}

	var m map[string]interface{}
	if err := json.Unmarshal([]byte(result), &m); err != nil {
		t.Fatalf("Result is not valid JSON: %v\nResult: %s", err, result)
	}
	if m["url"] != "https://example.com/path" {
		t.Errorf("url = %v, want %q", m["url"], "https://example.com/path")
	}
}

// --- Encrypt/Decrypt tests ---

func TestEncryptDecryptRoundTrip(t *testing.T) {
	key := make([]byte, 32)
	for i := range key {
		key[i] = byte(i)
	}
	plaintext := []byte("hello, OrangeCoding config secret!")

	ciphertext, err := Encrypt(key, plaintext)
	if err != nil {
		t.Fatalf("Encrypt failed: %v", err)
	}

	decrypted, err := Decrypt(key, ciphertext)
	if err != nil {
		t.Fatalf("Decrypt failed: %v", err)
	}

	if string(decrypted) != string(plaintext) {
		t.Errorf("Decrypt result = %q, want %q", string(decrypted), string(plaintext))
	}
}

func TestEncryptOutputDiffersFromPlaintext(t *testing.T) {
	key := make([]byte, 32)
	plaintext := []byte("same data same data same data")

	ciphertext, err := Encrypt(key, plaintext)
	if err != nil {
		t.Fatalf("Encrypt failed: %v", err)
	}

	if string(ciphertext) == string(plaintext) {
		t.Error("Ciphertext should differ from plaintext")
	}
}

func TestDecryptWithWrongKeyFails(t *testing.T) {
	key := make([]byte, 32)
	for i := range key {
		key[i] = byte(i)
	}
	wrongKey := make([]byte, 32)
	for i := range wrongKey {
		wrongKey[i] = byte(i + 1)
	}

	plaintext := []byte("secret data")

	ciphertext, err := Encrypt(key, plaintext)
	if err != nil {
		t.Fatalf("Encrypt failed: %v", err)
	}

	_, err = Decrypt(wrongKey, ciphertext)
	if err == nil {
		t.Error("Decrypt with wrong key should fail")
	}
}

func TestEncryptInvalidKeySize(t *testing.T) {
	key := []byte("short")
	plaintext := []byte("data")

	_, err := Encrypt(key, plaintext)
	if err == nil {
		t.Error("Encrypt with short key should fail")
	}
}

func TestDecryptInvalidCiphertext(t *testing.T) {
	key := make([]byte, 32)

	_, err := Decrypt(key, []byte("tooshort"))
	if err == nil {
		t.Error("Decrypt with too-short ciphertext should fail")
	}
}
