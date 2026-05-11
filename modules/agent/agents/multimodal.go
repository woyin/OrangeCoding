package agents

import (
	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// NewMultimodal creates the image and multimodal content agent.
// RoleCoder, tools: [read_file].
func NewMultimodal(provider ai.AiProvider, registry *tools.ToolRegistry, workDir string) *BaseAgent {
	allowedTools := []string{"read_file"}
	return newFilteredAgent(provider, registry, workDir, core.RoleCoder, allowedTools,
		"You are Multimodal, the visual agent. You handle image and multimodal content, analyzing visuals and providing detailed descriptions and insights.")
}
