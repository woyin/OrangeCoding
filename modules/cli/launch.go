package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/config"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
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
	loopConfig, err := agentLoopConfigFromCLIConfig(configPath, cfg)
	if err != nil {
		return err
	}

	// 3. Determine mode based on flags
	switch {
	case prompt != "":
		// Single-shot mode: run once with the given prompt, print result to stdout
		return runSingleShot(cmd, cfg, loopConfig, prompt)
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

// runSingleShot executes a single prompt through the configured provider and AgentLoop.
func runSingleShot(cmd *cobra.Command, cfg *config.OrangeConfig, loopConfig agent.AgentLoopConfig, task string) error {
	provider := cfg.DefaultProvider
	if provider == "" {
		provider = "openai"
	}

	providerConfig := aiProviderConfigFromCLIConfig(provider, cfg)
	aiProvider, err := (&ai.ProviderFactory{}).CreateProvider(provider, providerConfig)
	if err != nil {
		return err
	}
	if providerConfig.DefaultModel == "" {
		providerConfig = ai.NormalizeProviderConfig(provider, providerConfig)
	}

	registry := tools.CreateDefaultRegistry()
	agentCtx := agent.NewAgentContext(core.NewSessionId(), currentWorkDir())
	agentCtx.SetSystemPrompt("You are OrangeCoding, a practical coding agent. Help the user complete software tasks by reasoning carefully, using tools when useful, and reporting concise verifiable results.")
	agentCtx.AddUserMessage(task)

	loop := agent.NewAgentLoop(
		core.NewAgentId(),
		aiProvider,
		agent.NewToolExecutor(registry),
		agentCtx,
		loopConfig,
		agent.BuildToolDefinitions(registry),
	)

	fmt.Fprintf(cmd.OutOrStdout(), "Provider: %s, Model: %s\n", provider, providerConfig.DefaultModel)
	fmt.Fprintf(cmd.OutOrStdout(), "Task: %s\n", task)
	if loopConfig.CheckpointStore != nil {
		fmt.Fprintln(cmd.OutOrStdout(), "Harness checkpoints: file")
	} else {
		fmt.Fprintln(cmd.OutOrStdout(), "Harness checkpoints: memory")
	}

	_, err = loop.Run(context.Background(), ai.ChatOptions{Model: providerConfig.DefaultModel}, nil)
	if err != nil {
		return err
	}
	answer := lastAssistantContent(agentCtx)
	if answer != "" {
		fmt.Fprintln(cmd.OutOrStdout(), answer)
	}

	return nil
}

func aiProviderConfigFromCLIConfig(providerName string, cfg *config.OrangeConfig) ai.ProviderConfig {
	var providerCfg config.ProviderConfig
	if cfg != nil && cfg.Providers != nil {
		for _, candidate := range providerConfigKeys(providerName) {
			providerCfg = cfg.Providers[candidate]
			if providerCfg.APIKey != "" || providerCfg.BaseURL != "" || providerCfg.DefaultModel != "" {
				break
			}
		}
	}
	aiCfg := ai.ProviderConfig{
		APIKey:       providerCfg.APIKey,
		APISecret:    providerCfg.APISecret,
		BaseURL:      providerCfg.BaseURL,
		DefaultModel: providerCfg.DefaultModel,
		TimeoutSecs:  providerCfg.TimeoutSecs,
		Extra:        providerCfg.Extra,
	}
	if cfg != nil && cfg.DefaultModel != "" {
		aiCfg.DefaultModel = cfg.DefaultModel
	}
	return ai.NormalizeProviderConfig(providerName, aiCfg)
}

func providerConfigKeys(providerName string) []string {
	normalized := strings.ToLower(strings.TrimSpace(providerName))
	keys := []string{providerName, normalized}
	switch normalized {
	case "gpt":
		keys = append(keys, "openai")
	case "opus", "claude":
		keys = append(keys, "anthropic")
	case "moonshot":
		keys = append(keys, "kimi")
	case "bigmodel", "zhipu":
		keys = append(keys, "glm")
	}
	return keys
}

func agentLoopConfigFromCLIConfig(configPath string, cfg *config.OrangeConfig) (agent.AgentLoopConfig, error) {
	loopConfig := agent.DefaultLoopConfig()
	if cfg == nil {
		return loopConfig, nil
	}
	if cfg.Harness.ReasoningEffort != "" {
		loopConfig.Reasoning.Effort = agent.ReasoningEffort(strings.ToLower(strings.TrimSpace(cfg.Harness.ReasoningEffort)))
	}
	if cfg.Harness.ReasoningBudgetTokens > 0 {
		loopConfig.Reasoning.BudgetTokens = cfg.Harness.ReasoningBudgetTokens
	}

	store := strings.ToLower(strings.TrimSpace(cfg.Harness.CheckpointStore))
	if store == "" || store == "memory" {
		return loopConfig, nil
	}
	if store != "file" {
		return loopConfig, fmt.Errorf("unsupported harness checkpoint_store %q", cfg.Harness.CheckpointStore)
	}

	dir := strings.TrimSpace(cfg.Harness.CheckpointDir)
	if dir == "" {
		dir = "checkpoints"
	}
	if !filepath.IsAbs(dir) {
		dir = filepath.Join(filepath.Dir(configPath), dir)
	}
	loopConfig.CheckpointStore = agent.NewFileCheckpointStore(dir)
	return loopConfig, nil
}

func currentWorkDir() string {
	wd, err := os.Getwd()
	if err != nil {
		return "."
	}
	return wd
}

func lastAssistantContent(ctx *agent.AgentContext) string {
	messages := ctx.Conversation().Messages()
	for i := len(messages) - 1; i >= 0; i-- {
		if messages[i].Role == core.RoleAssistant {
			return messages[i].Content
		}
	}
	return ""
}
