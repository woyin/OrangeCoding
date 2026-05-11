package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/woyin/OrangeCoding/modules/config"
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Initialize project config",
	Long:  "Create the default configuration file at ~/.orangecoding/config.json.",
	RunE: func(cmd *cobra.Command, args []string) error {
		configPath := defaultConfigPath()
		return runInitAtPath(configPath)
	},
}

// runInitAtPath creates a default configuration file at the given path.
// This is a separate function so tests can supply a custom path.
func runInitAtPath(configPath string) error {
	// Check if file already exists
	if _, err := os.Stat(configPath); err == nil {
		return fmt.Errorf("config file already exists at %s", configPath)
	}

	mgr := config.NewConfigManager()
	cfg := config.DefaultConfig()
	return mgr.Save(configPath, &cfg)
}
