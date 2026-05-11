package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewPrometheus creates the planning agent.
// RolePlanner, tools: [read_file, find, grep, glob].
func NewPrometheus(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"read_file", "find", "grep", "glob"}
	return newFilteredAgent(provider, registry, workDir, core.RolePlanner, allowedTools,
		"You are Prometheus, the planner. You decompose tasks into clear, actionable plans. You analyze requirements and create step-by-step execution strategies.")
}
