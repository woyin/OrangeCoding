package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/woyin/OrangeCoding/modules/config"
)

var statusCmd = &cobra.Command{
	Use:   "status",
	Short: "Show system status",
	Long:  "Display the current OrangeCoding version, configuration path, and configured providers.",
	RunE: func(cmd *cobra.Command, args []string) error {
		configPath := defaultConfigPath()
		output, err := runStatusAtPath(configPath)
		if err != nil {
			return err
		}
		fmt.Fprint(cmd.OutOrStdout(), output)
		return nil
	},
}

// runStatusAtPath generates a status summary string using the config at path.
func runStatusAtPath(configPath string) (string, error) {
	var sb strings.Builder

	sb.WriteString(fmt.Sprintf("OrangeCoding v%s\n", Version))
	sb.WriteString(fmt.Sprintf("Config: %s\n", configPath))

	// Try to load config and list providers
	mgr := config.NewConfigManager()
	cfg, err := mgr.Load(configPath)
	if err != nil {
		sb.WriteString("Providers: (config not found or unreadable)\n")
		return sb.String(), nil
	}

	var providers []string
	for name := range cfg.Providers {
		providers = append(providers, name)
	}

	if len(providers) == 0 {
		sb.WriteString("Providers: (none configured)\n")
	} else {
		sb.WriteString(fmt.Sprintf("Providers: %v\n", providers))
	}

	sb.WriteString(fmt.Sprintf("Default provider: %s\n", cfg.DefaultProvider))
	sb.WriteString(fmt.Sprintf("Default model: %s\n", cfg.DefaultModel))
	sb.WriteString(fmt.Sprintf("Control port: %d\n", cfg.ControlPort))

	// Verify config file exists on disk
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		sb.WriteString("Warning: config file does not exist on disk\n")
	}

	return sb.String(), nil
}
