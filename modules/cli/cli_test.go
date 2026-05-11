package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/spf13/cobra"
)

// --- Helper ---

// executeCommand runs a cobra command with the given arguments and returns
// its output and error. It installs a temporary buffer on the command's
// SetOut / SetErr so we can capture printed text.
func executeCommand(root *cobra.Command, args ...string) (string, error) {
	buf := new(bytes.Buffer)
	root.SetOut(buf)
	root.SetErr(buf)
	root.SetArgs(args)

	err := root.Execute()
	return buf.String(), err
}

// --- Tests ---

// TestVersionCommand verifies that the version subcommand prints the version.
func TestVersionCommand(t *testing.T) {
	output, err := executeCommand(rootCmd, "version")
	if err != nil {
		t.Fatalf("version command failed: %v", err)
	}

	expected := fmt.Sprintf("orangecoding v%s", Version)
	if !strings.Contains(output, expected) {
		t.Errorf("version output = %q, want to contain %q", output, expected)
	}
}

// TestInitCommand verifies that init creates a config file in the given directory.
func TestInitCommand(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, ".orangecoding", "config.json")

	// Override home dir detection by using the init command directly
	buf := new(bytes.Buffer)
	initCmd.SetOut(buf)
	initCmd.SetErr(buf)

	// We test init via a custom function that accepts a path
	err := runInitAtPath(configPath)
	if err != nil {
		t.Fatalf("init command failed: %v", err)
	}

	// Verify the config file exists
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		t.Fatalf("config file was not created at %s", configPath)
	}

	// Verify it is valid JSON with expected defaults
	data, err := os.ReadFile(configPath)
	if err != nil {
		t.Fatalf("failed to read config file: %v", err)
	}

	var cfg map[string]interface{}
	if err := json.Unmarshal(data, &cfg); err != nil {
		t.Fatalf("config is not valid JSON: %v", err)
	}

	if cfg["log_level"] != "info" {
		t.Errorf("log_level = %v, want info", cfg["log_level"])
	}
	if cfg["default_provider"] != "openai" {
		t.Errorf("default_provider = %v, want openai", cfg["default_provider"])
	}
}

// TestRootCommand verifies that the root command has all expected subcommands.
func TestRootCommand(t *testing.T) {
	expectedSubcommands := []string{"launch", "init", "config", "status", "serve", "version"}

	for _, name := range expectedSubcommands {
		found := false
		for _, sub := range rootCmd.Commands() {
			if sub.Name() == name {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("root command missing subcommand %q", name)
		}
	}
}

// TestConfigSetGet verifies the config get/set subcommands work together.
func TestConfigSetGet(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, ".orangecoding", "config.json")

	// First, init to create the config
	if err := runInitAtPath(configPath); err != nil {
		t.Fatalf("init failed: %v", err)
	}

	// Set a value
	buf := new(bytes.Buffer)
	configSetCmd.SetOut(buf)
	configSetCmd.SetErr(buf)
	configSetCmd.SetArgs([]string{"log_level", "debug"})
	// Override the config path used by configSetCmd
	err := runConfigSetAtPath(configPath, "log_level", "debug")
	if err != nil {
		t.Fatalf("config set failed: %v", err)
	}

	// Get the value back
	val, err := runConfigGetAtPath(configPath, "log_level")
	if err != nil {
		t.Fatalf("config get failed: %v", err)
	}

	if val != "debug" {
		t.Errorf("config get log_level = %v, want debug", val)
	}
}

// TestStatusCommand verifies that status runs without error.
func TestStatusCommand(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, ".orangecoding", "config.json")

	// Init a config first
	if err := runInitAtPath(configPath); err != nil {
		t.Fatalf("init failed: %v", err)
	}

	output, err := runStatusAtPath(configPath)
	if err != nil {
		t.Fatalf("status command failed: %v", err)
	}

	if !strings.Contains(output, "OrangeCoding") {
		t.Errorf("status output = %q, want to contain 'OrangeCoding'", output)
	}
	if !strings.Contains(output, Version) {
		t.Errorf("status output = %q, want to contain version %s", output, Version)
	}
}

// TestRootDefaultRunsLaunch verifies that running the root command with no
// subcommand invokes launch (the default runner).
func TestRootDefaultRunsLaunch(t *testing.T) {
	// Root command's RunE is runLaunch, which means calling with no args
	// should invoke runLaunch. We verify the wiring is correct.
	if rootCmd.RunE == nil {
		t.Error("root command RunE is nil, expected runLaunch")
	}
}

// TestLaunchFlags verifies launch command flags are registered.
func TestLaunchFlags(t *testing.T) {
	pFlag := launchCmd.Flags().Lookup("prompt")
	if pFlag == nil {
		t.Error("launch command missing --prompt flag")
	}
	tFlag := launchCmd.Flags().Lookup("text")
	if tFlag == nil {
		t.Error("launch command missing --text flag")
	}
}

// TestRootPersistentFlags verifies root command persistent flags.
func TestRootPersistentFlags(t *testing.T) {
	llFlag := rootCmd.PersistentFlags().Lookup("log-level")
	if llFlag == nil {
		t.Error("root command missing --log-level persistent flag")
	}
	jlFlag := rootCmd.PersistentFlags().Lookup("json-log")
	if jlFlag == nil {
		t.Error("root command missing --json-log persistent flag")
	}
}

// TestConfigSubcommands verifies config command has get and set subcommands.
func TestConfigSubcommands(t *testing.T) {
	expected := []string{"get", "set"}
	for _, name := range expected {
		found := false
		for _, sub := range configCmd.Commands() {
			if sub.Name() == name {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("config command missing subcommand %q", name)
		}
	}
}
