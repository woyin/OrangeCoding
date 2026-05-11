package main

import (
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"github.com/woyin/OrangeCoding/modules/config"
)

var rootCmd = &cobra.Command{
	Use:   "orangecoding",
	Short: "AI coding agent",
	Long:  "OrangeCoding is an AI-powered coding agent with multi-agent orchestration capabilities.",
	// Default command = launch when run with no subcommand
	RunE: runLaunch,
}

var logLevel string
var jsonLog bool

func init() {
	rootCmd.PersistentFlags().StringVar(&logLevel, "log-level", "info", "log level (debug, info, warn, error)")
	rootCmd.PersistentFlags().BoolVar(&jsonLog, "json-log", false, "enable JSON log format")

	// Register all subcommands
	rootCmd.AddCommand(launchCmd)
	rootCmd.AddCommand(initCmd)
	rootCmd.AddCommand(configCmd)
	rootCmd.AddCommand(statusCmd)
	rootCmd.AddCommand(serveCmd)
	rootCmd.AddCommand(versionCmd)
}

// configPackage is a reference to the config package, used by launch.go.
// This ensures the import is used without additional ceremony.
var _ = config.DefaultConfig

// Execute runs the root command.
func Execute() error {
	return rootCmd.Execute()
}

// defaultConfigPath returns the default configuration file path
// (~/.orangecoding/config.json).
func defaultConfigPath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		home = "."
	}
	return filepath.Join(home, ".orangecoding", "config.json")
}
