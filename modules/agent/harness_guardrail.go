package agent

import (
	"context"
	"encoding/json"
	"strings"
	"sync"
	"time"

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
func NewGuardrailPipeline(guardrails ...Guardrail) *GuardrailPipeline {
	return &GuardrailPipeline{guardrails: guardrails}
}

// GuardrailPipeline runs guardrails in order and stops on deny or warn.
type GuardrailPipeline struct {
	guardrails []Guardrail
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

// --- Phase 13: Additional guardrails and infrastructure ---

// TokenBudgetGuardrail warns when approaching token budget limits.
type TokenBudgetGuardrail struct{}

// NewTokenBudgetGuardrail creates a guardrail that warns on token budget approach.
func NewTokenBudgetGuardrail() Guardrail {
	return TokenBudgetGuardrail{}
}

func (g TokenBudgetGuardrail) Name() string { return "token_budget" }

func (g TokenBudgetGuardrail) Check(ctx context.Context, input GuardrailContext) GuardrailResult {
	if input.Phase != GuardrailPhasePreModel && input.Phase != GuardrailPhaseFinalOutput {
		return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
	}
	if input.MaxTokens > 0 && input.TokenEstimate > input.MaxTokens {
		return GuardrailResult{
			Decision: GuardrailWarn,
			Reason:   "approaching token budget",
			Name:     g.Name(),
		}
	}
	return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
}

// OutputLengthGuardrail warns when output exceeds recommended length.
type OutputLengthGuardrail struct {
	maxLength int
}

// NewOutputLengthGuardrail creates a guardrail that warns on long outputs.
func NewOutputLengthGuardrail() Guardrail {
	return OutputLengthGuardrail{maxLength: 50000}
}

// NewOutputLengthGuardrailWithLimit creates a guardrail with a custom max length.
func NewOutputLengthGuardrailWithLimit(maxLen int) Guardrail {
	return OutputLengthGuardrail{maxLength: maxLen}
}

func (g OutputLengthGuardrail) Name() string { return "output_length" }

func (g OutputLengthGuardrail) Check(ctx context.Context, input GuardrailContext) GuardrailResult {
	if input.Phase != GuardrailPhaseFinalOutput {
		return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
	}
	if len(input.Output) > g.maxLength {
		return GuardrailResult{
			Decision: GuardrailWarn,
			Reason:   "output exceeds recommended length",
			Name:     g.Name(),
		}
	}
	return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
}

// GuardrailLogEntry records a single guardrail decision for audit.
type GuardrailLogEntry struct {
	Name      string
	Decision  GuardrailDecision
	Reason    string
	Phase     GuardrailPhase
	Timestamp time.Time
}

// GuardrailLogger collects guardrail decisions for observability.
type GuardrailLogger struct {
	mu      sync.RWMutex
	entries []GuardrailLogEntry
}

// NewGuardrailLogger creates a new guardrail logger.
func NewGuardrailLogger() *GuardrailLogger {
	return &GuardrailLogger{}
}

// Log appends a guardrail log entry.
func (l *GuardrailLogger) Log(entry GuardrailLogEntry) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.entries = append(l.entries, entry)
}

// Recent returns the last n log entries (or all if n exceeds length).
func (l *GuardrailLogger) Recent(n int) []GuardrailLogEntry {
	l.mu.RLock()
	defer l.mu.RUnlock()
	if n >= len(l.entries) {
		return append([]GuardrailLogEntry(nil), l.entries...)
	}
	return append([]GuardrailLogEntry(nil), l.entries[len(l.entries)-n:]...)
}

// Warnings returns only entries with warn decision.
func (l *GuardrailLogger) Warnings() []GuardrailLogEntry {
	l.mu.RLock()
	defer l.mu.RUnlock()
	var warns []GuardrailLogEntry
	for _, e := range l.entries {
		if e.Decision == GuardrailWarn {
			warns = append(warns, e)
		}
	}
	return warns
}

// Len returns the total number of logged entries.
func (l *GuardrailLogger) Len() int {
	l.mu.RLock()
	defer l.mu.RUnlock()
	return len(l.entries)
}

// LLMGuardrailConfig configures an LLM-based guardrail.
type LLMGuardrailConfig struct {
	Phase    GuardrailPhase
	Prompt   string
	Provider func(ctx context.Context, prompt, content string) (bool, error)
}

// LLMGuardrail uses an external LLM to evaluate content safety.
type LLMGuardrail struct {
	config LLMGuardrailConfig
}

// NewLLMGuardrail creates an LLM-based guardrail.
func NewLLMGuardrail(config LLMGuardrailConfig) Guardrail {
	return &LLMGuardrail{config: config}
}

func (g *LLMGuardrail) Name() string { return "llm_guardrail" }

func (g *LLMGuardrail) Check(ctx context.Context, input GuardrailContext) GuardrailResult {
	if input.Phase != g.config.Phase {
		return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
	}
	if g.config.Provider == nil {
		return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
	}
	content := input.Output
	if content == "" && input.ToolCall != nil {
		content = string(input.ToolCall.Arguments)
	}
	if content == "" {
		return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
	}
	safe, err := g.config.Provider(ctx, g.config.Prompt, content)
	if err != nil {
		return GuardrailResult{
			Decision: GuardrailDeny,
			Reason:   "llm guardrail evaluation failed: " + err.Error(),
			Name:     g.Name(),
		}
	}
	if !safe {
		return GuardrailResult{
			Decision: GuardrailDeny,
			Reason:   "llm guardrail rejected content",
			Name:     g.Name(),
		}
	}
	return GuardrailResult{Decision: GuardrailAllow, Name: g.Name()}
}

// --- Original guardrails below (unchanged) ---

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
