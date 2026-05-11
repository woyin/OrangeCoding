package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewHephaestus creates a tool execution error recovery agent.
// RoleCoder, tools: [bash, read_file].
func NewHephaestus(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"bash", "read_file"}
	return newFilteredAgent(provider, registry, workDir, core.RoleCoder, allowedTools,
		"You are Hephaestus, the tool error fixer. You fix tool execution errors. When a tool call fails, you diagnose the issue and retry or adjust the approach.")
}
