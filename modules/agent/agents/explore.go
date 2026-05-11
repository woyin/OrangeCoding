package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewExplore creates the codebase exploration agent.
// RoleObserver, tools: [read_file, find, grep, glob].
func NewExplore(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"read_file", "find", "grep", "glob"}
	return newFilteredAgent(provider, registry, workDir, core.RoleObserver, allowedTools,
		"You are Explorer, the codebase navigator. You explore codebases, map structure, identify patterns, and report findings clearly.")
}
