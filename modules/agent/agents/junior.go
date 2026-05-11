package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewJunior creates the simple task handling agent.
// RoleCoder, tools: [bash, read_file, write_file].
func NewJunior(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"bash", "read_file", "write_file"}
	return newFilteredAgent(provider, registry, workDir, core.RoleCoder, allowedTools,
		"You are Junior, the simple task handler. You handle straightforward tasks quickly and efficiently without overthinking.")
}
