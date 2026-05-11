package main

import (
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/spf13/cobra"

	"github.com/woyin/OrangeCoding/modules/config"
	controlprotocol "github.com/woyin/OrangeCoding/modules/control-protocol"
	controlserver "github.com/woyin/OrangeCoding/modules/control-server"
	"github.com/woyin/OrangeCoding/modules/worker"
)

var serveAddr string

var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Start control server",
	Long:  "Start the OrangeCoding control server for managing agent sessions over HTTP/WebSocket.",
	RunE: func(cmd *cobra.Command, args []string) error {
		configPath := defaultConfigPath()

		// Load configuration
		mgr := config.NewConfigManager()
		cfg, err := mgr.Load(configPath)
		if err != nil {
			return fmt.Errorf("failed to load config from %s: %w\nRun 'orangecoding init' first", configPath, err)
		}

		// Determine bind address
		addr := serveAddr
		if addr == "" {
			addr = fmt.Sprintf(":%d", cfg.ControlPort)
		}

		// Create the event channel and worker runtime
		eventCh := make(chan controlprotocol.ServerEvent, 64)
		runtime := worker.NewWorkerRuntime(eventCh)

		// Create and start the control server
		server := controlserver.NewServer(runtime, addr)
		if err := server.Start(); err != nil {
			return fmt.Errorf("failed to start server: %w", err)
		}

		fmt.Fprintf(cmd.OutOrStdout(), "OrangeCoding control server listening on %s\n", addr)
		fmt.Fprintln(cmd.OutOrStdout(), "Press Ctrl+C to stop.")

		// Block until interrupted
		sigCh := make(chan os.Signal, 1)
		signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
		<-sigCh

		fmt.Fprintln(cmd.OutOrStdout(), "\nShutting down...")
		return server.Stop()
	},
}

func init() {
	serveCmd.Flags().StringVar(&serveAddr, "addr", "", "bind address (default: from config control_port)")
}
