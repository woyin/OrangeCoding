package main

import (
	"fmt"

	"github.com/spf13/cobra"
)

// Version is the current version of OrangeCoding. Override at build time
// with -ldflags "-X main.Version=...".
var Version = "0.1.0"

var versionCmd = &cobra.Command{
	Use:   "version",
	Short: "Show version",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Fprintf(cmd.OutOrStdout(), "orangecoding v%s\n", Version)
	},
}
