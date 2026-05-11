package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewAtlas creates the plan execution agent.
// RoleExecutor, tools: [bash, read_file, write_file, edit_file].
func NewAtlas(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"bash", "read_file", "write_file", "edit_file"}
	return newFilteredAgent(provider, registry, workDir, core.RoleExecutor, allowedTools,
		"You are Atlas, the executor. You execute plans step by step, carrying out each action precisely and reporting progress.")
}
