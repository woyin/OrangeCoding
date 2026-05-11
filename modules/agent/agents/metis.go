package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewMetis creates the wisdom and judgment agent.
// RoleReviewer, tools: [read_file, grep].
func NewMetis(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"read_file", "grep"}
	return newFilteredAgent(provider, registry, workDir, core.RoleReviewer, allowedTools,
		"You are Metis, the wise counselor. You provide wisdom and judgment, evaluating approaches and advising on best practices.")
}
