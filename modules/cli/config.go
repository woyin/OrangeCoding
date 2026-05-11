package main

import (
	"fmt"
	"strconv"

	"github.com/spf13/cobra"

	"github.com/woyin/OrangeCoding/modules/config"
)

var configCmd = &cobra.Command{
	Use:   "config",
	Short: "Manage configuration",
	Long:  "View and modify OrangeCoding configuration values.",
}

var configGetCmd = &cobra.Command{
	Use:   "get [key]",
	Short: "Get a configuration value",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		configPath := defaultConfigPath()
		val, err := runConfigGetAtPath(configPath, args[0])
		if err != nil {
			return err
		}
		fmt.Fprintln(cmd.OutOrStdout(), val)
		return nil
	},
}

var configSetCmd = &cobra.Command{
	Use:   "set [key] [value]",
	Short: "Set a configuration value",
	Args:  cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		configPath := defaultConfigPath()
		return runConfigSetAtPath(configPath, args[0], args[1])
	},
}

func init() {
	configCmd.AddCommand(configGetCmd)
	configCmd.AddCommand(configSetCmd)
}

// runConfigGetAtPath loads the config at path and returns the value for key.
func runConfigGetAtPath(path, key string) (interface{}, error) {
	mgr := config.NewConfigManager()
	return mgr.Get(path, key)
}

// runConfigSetAtPath loads the config at path, sets key to value, and saves.
// It performs type coercion for common cases (string, int, bool).
func runConfigSetAtPath(path, key, value string) error {
	mgr := config.NewConfigManager()

	// Determine the target type from the existing config field
	// and coerce the string value appropriately.
	var coerced interface{} = value

	// Try to get the current value to determine its type
	current, err := mgr.Get(path, key)
	if err != nil {
		return err
	}

	switch current.(type) {
	case int:
		n, err := strconv.Atoi(value)
		if err != nil {
			return fmt.Errorf("cannot convert %q to int: %w", value, err)
		}
		coerced = n
	case float64:
		f, err := strconv.ParseFloat(value, 64)
		if err != nil {
			return fmt.Errorf("cannot convert %q to float: %w", value, err)
		}
		coerced = f
	case bool:
		b, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("cannot convert %q to bool: %w", value, err)
		}
		coerced = b
	}

	return mgr.Set(path, key, coerced)
}
