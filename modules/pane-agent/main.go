// Command pane-agent runs an agent loop inside a terminal pane,
// communicating with the parent process over a Unix domain socket.
//
// Usage: pane-agent --socket /path/to/socket.sock
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/woyin/OrangeCoding/modules/agent"
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/config"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/multiplexer"
	"github.com/woyin/OrangeCoding/modules/tools"
)

func main() {
	socketPath := flag.String("socket", "", "Unix socket path for IPC with parent")
	configPath := flag.String("config", "", "Path to config file (optional)")
	flag.Parse()

	if *socketPath == "" {
		log.Fatal("--socket is required")
	}

	if err := run(*socketPath, *configPath); err != nil {
		log.Fatalf("pane-agent: %v", err)
	}
}

func run(socketPath string, configPath string) error {
	// 1. Connect to the parent's Unix socket.
	conn, err := multiplexer.ConnectSocket(socketPath, 30*time.Second)
	if err != nil {
		return fmt.Errorf("connect to socket: %w", err)
	}
	defer conn.Close()

	transport := multiplexer.NewSocketTransport(conn)

	// 2. Receive the task payload from the parent.
	msg, err := transport.Receive()
	if err != nil {
		return fmt.Errorf("receive task: %w", err)
	}
	if msg.Type != multiplexer.IPCTask {
		return fmt.Errorf("expected task message, got %q", msg.Type)
	}

	var task multiplexer.TaskPayload
	if err := json.Unmarshal(msg.Payload, &task); err != nil {
		return fmt.Errorf("unmarshal task: %w", err)
	}

	// 3. Set up the AI provider from config.
	cfg := loadConfig(configPath)
	provider, providerModel, err := createProvider(cfg)
	if err != nil {
		sendError(transport, msg.ID, err)
		return err
	}

	// 4. Create the agent loop.
	registry := tools.CreateDefaultRegistry()

	// Filter tools if specified.
	if len(task.Tools) > 0 {
		registry = agent.FilteredRegistry(registry, task.Tools)
	}

	loopConfig := agent.DefaultLoopConfig()
	if cfg != nil {
		if cfg.Harness.ReasoningEffort != "" {
			loopConfig.Reasoning.Effort = agent.ReasoningEffort(cfg.Harness.ReasoningEffort)
		}
		if cfg.Harness.ReasoningBudgetTokens > 0 {
			loopConfig.Reasoning.BudgetTokens = cfg.Harness.ReasoningBudgetTokens
		}
	}

	agentID := core.NewAgentId()
	sessionID := core.NewSessionId()
	agentCtx := agent.NewAgentContext(sessionID, currentWorkDir())
	agentCtx.SetSystemPrompt("You are a coding agent running in a terminal pane. Complete the assigned task efficiently.")
	agentCtx.AddUserMessage(task.Task)

	loop := agent.NewAgentLoop(
		agentID,
		provider,
		agent.NewToolExecutor(registry),
		agentCtx,
		loopConfig,
		agent.BuildToolDefinitions(registry),
	)

	// 5. Run the agent loop, streaming events to the parent.
	eventCh := make(chan core.AgentEvent, 100)
	go func() {
		for ev := range eventCh {
			evtPayload := multiplexer.EventPayload{
				EventType: fmt.Sprintf("%T", ev),
				Data:      fmt.Sprintf("%+v", ev),
			}
			payloadBytes, _ := json.Marshal(evtPayload)
			transport.Send(multiplexer.IPCMessage{
				Type:    multiplexer.IPCEvent,
				ID:      msg.ID,
				Payload: payloadBytes,
			})
		}
	}()

	// Set up signal handling for graceful shutdown.
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-sigCh
		cancel()
	}()

	_, err = loop.Run(ctx, ai.ChatOptions{Model: providerModel}, eventCh)
	close(eventCh)

	// 6. Send the result back.
	if err != nil {
		sendError(transport, msg.ID, err)
		return err
	}

	answer := lastAssistantContent(agentCtx)
	resultPayload := multiplexer.ResultPayload{
		Success: true,
		Content: answer,
	}
	resultBytes, _ := json.Marshal(resultPayload)
	return transport.Send(multiplexer.IPCMessage{
		Type:    multiplexer.IPCResult,
		ID:      msg.ID,
		Payload: resultBytes,
	})
}

func sendError(transport *multiplexer.SocketTransport, id string, err error) {
	resultPayload := multiplexer.ResultPayload{
		Success: false,
		Error:   err.Error(),
	}
	resultBytes, _ := json.Marshal(resultPayload)
	transport.Send(multiplexer.IPCMessage{
		Type:    multiplexer.IPCResult,
		ID:      id,
		Payload: resultBytes,
	})
}

func loadConfig(path string) *config.OrangeConfig {
	if path == "" {
		path = defaultConfigPath()
	}
	mgr := config.NewConfigManager()
	cfg, err := mgr.Load(path)
	if err != nil {
		return nil
	}
	return cfg
}

func defaultConfigPath() string {
	home, _ := os.UserHomeDir()
	return home + "/.orangecoding/config.json"
}

func createProvider(cfg *config.OrangeConfig) (ai.AiProvider, string, error) {
	if cfg == nil {
		return nil, "", fmt.Errorf("no config available")
	}
	providerName := cfg.DefaultProvider
	if providerName == "" {
		providerName = "openai"
	}
	providerConfig := ai.ProviderConfig{}
	if p, ok := cfg.Providers[providerName]; ok {
		providerConfig = ai.ProviderConfig{
			APIKey:       p.APIKey,
			BaseURL:      p.BaseURL,
			DefaultModel: p.DefaultModel,
			TimeoutSecs:  p.TimeoutSecs,
			Extra:        p.Extra,
		}
	}
	if cfg.DefaultModel != "" {
		providerConfig.DefaultModel = cfg.DefaultModel
	}
	providerConfig = ai.NormalizeProviderConfig(providerName, providerConfig)
	provider, err := (&ai.ProviderFactory{}).CreateProvider(providerName, providerConfig)
	if err != nil {
		return nil, "", err
	}
	return provider, providerConfig.DefaultModel, nil
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
