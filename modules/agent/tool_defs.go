package agent

import (
	"encoding/json"

	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// BuildToolDefinitions converts registered tools into provider-facing schemas.
func BuildToolDefinitions(registry *tools.ToolRegistry) []ai.ToolDefinition {
	var defs []ai.ToolDefinition
	for _, t := range registry.List() {
		params := toolParameters(t.Parameters())
		defs = append(defs, ai.ToolDefinition{
			Type: "function",
			Function: ai.FunctionDefinition{
				Name:        t.Name(),
				Description: t.Description(),
				Parameters:  params,
			},
		})
	}
	return defs
}

func toolParameters(raw json.RawMessage) ai.ToolParameter {
	var params ai.ToolParameter
	if err := json.Unmarshal(raw, &params); err != nil || params.Type == "" {
		return ai.ToolParameter{Type: "object", Properties: make(map[string]interface{})}
	}
	if params.Properties == nil {
		params.Properties = make(map[string]interface{})
	}
	return params
}
