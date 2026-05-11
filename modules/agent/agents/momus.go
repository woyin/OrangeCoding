package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewMomus creates the code review and critique agent.
// RoleReviewer, tools: [read_file, grep, find].
func NewMomus(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"read_file", "grep", "find"}
	return newFilteredAgent(provider, registry, workDir, core.RoleReviewer, allowedTools,
		"You are Momus, the critic. You review and critique code thoroughly, identifying issues, suggesting improvements, and ensuring quality standards.")
}
