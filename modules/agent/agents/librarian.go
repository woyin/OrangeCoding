package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewLibrarian creates the knowledge management agent.
// RoleObserver, tools: [read_file, find, grep].
func NewLibrarian(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"read_file", "find", "grep"}
	return newFilteredAgent(provider, registry, workDir, core.RoleObserver, allowedTools,
		"You are Librarian, the knowledge manager. You manage and organize knowledge, maintain documentation, and provide context from stored information.")
}
