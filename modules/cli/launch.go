package main

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/woyin/OrangeCoding/modules/config"
)

var prompt string
var textMode bool

var launchCmd = &cobra.Command{
	Use:   "launch",
	Short: "Start AI agent",
	Long:  "Launch the OrangeCoding AI agent. By default starts in TUI mode.",
	RunE:  runLaunch,
}

func init() {
	launchCmd.Flags().StringVarP(&prompt, "prompt", "p", "", "single-shot task prompt")
	launchCmd.Flags().BoolVar(&textMode, "text", false, "text mode (no TUI)")
}

func runLaunch(cmd *cobra.Command, args []string) error {
	// 1. Resolve config path
	configPath := defaultConfigPath()

	// 2. Load config via config.ConfigManager
	mgr := config.NewConfigManager()
	cfg, err := mgr.Load(configPath)
	if err != nil {
		return fmt.Errorf("failed to load config from %s: %w\nRun 'orangecoding init' first", configPath, err)
	}

	// 3. Determine mode based on flags
	switch {
	case prompt != "":
		// Single-shot mode: run once with the given prompt, print result to stdout
		return runSingleShot(cmd, cfg, prompt)
	case textMode:
		// Text REPL mode (stub)
		fmt.Fprintln(cmd.OutOrStdout(), "Text REPL mode is not yet implemented.")
		return nil
	default:
		// TUI mode (stub)
		fmt.Fprintln(cmd.OutOrStdout(), "TUI mode is not yet implemented.")
		fmt.Fprintln(cmd.OutOrStdout(), "Use --text for text mode or -p <prompt> for single-shot mode.")
		return nil
	}
}

// runSingleShot executes a single prompt and prints the result.
// Full implementation will wire up the AI provider and agent loop.
// For now, it validates configuration and reports readiness.
func runSingleShot(cmd *cobra.Command, cfg *config.OrangeConfig, task string) error {
	provider := cfg.DefaultProvider
	if provider == "" {
		provider = "openai"
	}

	model := cfg.DefaultModel
	if model == "" {
		model = "default"
	}

	fmt.Fprintf(cmd.OutOrStdout(), "Provider: %s, Model: %s\n", provider, model)
	fmt.Fprintf(cmd.OutOrStdout(), "Task: %s\n", task)
	fmt.Fprintln(cmd.OutOrStdout(), "Single-shot agent execution is not yet fully wired.")

	// TODO: Wire up AI provider via ai.ProviderFactory
	// TODO: Create tool registry via tools.CreateDefaultRegistry()
	// TODO: Create agent loop and run it
	// TODO: Print the result

	return nil
}
