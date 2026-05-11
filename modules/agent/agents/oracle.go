package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewOracle creates the question-answering agent.
// RoleObserver, tools: [read_file, grep].
func NewOracle(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"read_file", "grep"}
	return newFilteredAgent(provider, registry, workDir, core.RoleObserver, allowedTools,
		"You are Oracle, the answerer. You answer questions accurately by searching and reading relevant files. You provide clear, well-structured explanations.")
}
