package agent

import (
	"context"
	"encoding/json"
	"strings"

	"github.com/woyin/OrangeCoding/modules/core"
)

// GuardrailPhase identifies where a guardrail runs.
type GuardrailPhase string

const (
	GuardrailPhasePreModel    GuardrailPhase = "pre_model"
	GuardrailPhasePreTool     GuardrailPhase = "pre_tool"
	GuardrailPhasePostTool    GuardrailPhase = "post_tool"
	GuardrailPhaseFinalOutput GuardrailPhase = "final_output"
)

// GuardrailDecision is the outcome of a guardrail check.
type GuardrailDecision string

const (
	GuardrailAllow GuardrailDecision = "allow"
	GuardrailDeny  GuardrailDecision = "deny"
	GuardrailWarn  GuardrailDecision = "warn"
)

// GuardrailContext is the input to guardrail checks.
type GuardrailContext struct {
	Phase          GuardrailPhase
	ToolCall       *core.ToolCall
	Output         string
	RecentToolKeys []string
	TokenEstimate  int
	MaxTokens      int
}

// GuardrailResult describes a guardrail decision.
type GuardrailResult struct {
	Decision GuardrailDecision
	Reason   string
	Name     string
}

// Guardrail checks one harness boundary.
type Guardrail interface {
	Name() string
	Check(ctx context.Context, input GuardrailContext) GuardrailResult
}

// GuardrailPipeline runs guardrails in order and stops on deny.
type GuardrailPipeline struct {
	guardrails []Guardrail
}

// NewGuardrailPipeline creates an ordered pipeline.
func NewGuardrailPipeline(guardrails ...Guardrail) *GuardrailPipeline {
	return &GuardrailPipeline{guardrails: guardrails}
}

// Check runs the guardrail pipeline.
func (p *GuardrailPipeline) Check(ctx context.Context, input GuardrailContext) GuardrailResult {
	if err := ctx.Err(); err != nil {
		return GuardrailResult{Decision: GuardrailDeny, Reason: err.Error(), Name: "context"}
	}
	for _, guardrail := range p.guardrails {
		result := guardrail.Check(ctx, input)
		if result.Name == "" {
			result.Name = guardrail.Name()
		}
		if result.Decision == GuardrailDeny || result.Decision == GuardrailWarn {
			return result
		}
	}
	return GuardrailResult{Decision: GuardrailAllow, Name: "pipeline"}
}

type dangerousToolGuardrail struct{}

// NewDangerousToolGuardrail blocks obviously destructive shell commands.
func NewDangerousToolGuardrail() Guardrail {
	return dangerousToolGuardrail{}
}

func (g dangerousToolGuardrail) Name() string { return "dangerous_tool" }

func (g dangerousToolGuardrail) Check(ctx context.Context, input GuardrailContext) GuardrailResult {
	if input.Phase != GuardrailPhasePreTool || input.ToolCall == nil {
		return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
	}
	if input.ToolCall.FunctionName != "bash" {
		return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
	}
	command := extractCommandArgument(input.ToolCall.Arguments)
	lower := strings.ToLower(command)
	blocked := []string{"rm -rf /", "rm -rf /*", "mkfs", "dd if=", ":(){:|:&};:"}
	for _, pattern := range blocked {
		if strings.Contains(lower, pattern) {
			return GuardrailResult{Decision: GuardrailDeny, Reason: "dangerous shell command", Name: g.Name()}
		}
	}
	return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
}

type repeatedToolGuardrail struct {
	limit int
}

// NewRepeatedToolGuardrail blocks repeated identical tool calls at or above limit.
func NewRepeatedToolGuardrail(limit int) Guardrail {
	if limit <= 0 {
		limit = 3
	}
	return repeatedToolGuardrail{limit: limit}
}

func (g repeatedToolGuardrail) Name() string { return "repeated_tool" }

func (g repeatedToolGuardrail) Check(ctx context.Context, input GuardrailContext) GuardrailResult {
	if input.Phase != GuardrailPhasePreTool || input.ToolCall == nil {
		return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
	}
	key := ToolCallKey(*input.ToolCall)
	count := 0
	for _, recent := range input.RecentToolKeys {
		if recent == key {
			count++
		}
	}
	if count >= g.limit {
		return GuardrailResult{Decision: GuardrailDeny, Reason: "repeated identical tool call", Name: g.Name()}
	}
	return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
}

// ToolCallKey returns a stable key for loop detection.
func ToolCallKey(call core.ToolCall) string {
	return call.FunctionName + ":" + string(call.Arguments)
}

func extractCommandArgument(raw json.RawMessage) string {
	var payload struct {
		Command string `json:"command"`
	}
	if err := json.Unmarshal(raw, &payload); err != nil {
		return string(raw)
	}
	return payload.Command
}
