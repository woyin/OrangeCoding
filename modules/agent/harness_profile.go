package agent

import (
	"strings"
	"time"

	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
)

// OutputLanguage controls the preferred user-facing response language.
type OutputLanguage string

const (
	OutputLanguageAuto    OutputLanguage = "auto"
	OutputLanguageChinese OutputLanguage = "zh-CN"
	OutputLanguageEnglish OutputLanguage = "en"
)

// ReasoningEffort controls provider-specific reasoning budget hints.
type ReasoningEffort string

const (
	ReasoningEffortLow    ReasoningEffort = "low"
	ReasoningEffortMedium ReasoningEffort = "medium"
	ReasoningEffortHigh   ReasoningEffort = "high"
)

// ReasoningPolicy describes how the harness should ask the model to reason.
type ReasoningPolicy struct {
	Effort       ReasoningEffort
	BudgetTokens uint32
}

// LongTaskPolicy describes guardrails for multi-iteration agent work.
type LongTaskPolicy struct {
	Enabled             bool
	MaxToolCalls        uint32
	ProgressEveryNCalls uint32
	CompactionMaxTokens int
}

// HarnessProfile groups the agent-harness behavior that is orthogonal to tools.
type HarnessProfile struct {
	Language  OutputLanguage
	LongTask  LongTaskPolicy
	Reasoning ReasoningPolicy
}

// StopReason explains why an AgentLoop stopped.
type StopReason string

const (
	StopReasonCompleted     StopReason = "completed"
	StopReasonMaxIterations StopReason = "max_iterations"
	StopReasonCanceled      StopReason = "canceled"
	StopReasonProviderError StopReason = "provider_error"
	StopReasonToolBudget    StopReason = "tool_budget"
)

// ProgressSnapshot records coarse agent progress without leaking hidden reasoning.
type ProgressSnapshot struct {
	Iteration     uint32
	ToolCallsMade uint32
	TokensUsed    core.TokenUsage
	Reason        string
	CreatedAt     time.Time
}

// DefaultHarnessProfile returns conservative defaults for long-running Chinese work.
func DefaultHarnessProfile() HarnessProfile {
	return HarnessProfile{
		Language: OutputLanguageChinese,
		LongTask: LongTaskPolicy{
			Enabled:             true,
			MaxToolCalls:        120,
			ProgressEveryNCalls: 5,
			CompactionMaxTokens: 24000,
		},
		Reasoning: ReasoningPolicy{
			Effort:       ReasoningEffortHigh,
			BudgetTokens: 4096,
		},
	}
}

func (p HarnessProfile) normalized() HarnessProfile {
	defaults := DefaultHarnessProfile()
	if p.Language == "" {
		p.Language = defaults.Language
	}
	if p.Reasoning.Effort == "" {
		p.Reasoning.Effort = defaults.Reasoning.Effort
	}
	if p.Reasoning.BudgetTokens == 0 {
		p.Reasoning.BudgetTokens = defaults.Reasoning.BudgetTokens
	}
	if p.LongTask.Enabled {
		if p.LongTask.MaxToolCalls == 0 {
			p.LongTask.MaxToolCalls = defaults.LongTask.MaxToolCalls
		}
		if p.LongTask.ProgressEveryNCalls == 0 {
			p.LongTask.ProgressEveryNCalls = defaults.LongTask.ProgressEveryNCalls
		}
		if p.LongTask.CompactionMaxTokens == 0 {
			p.LongTask.CompactionMaxTokens = defaults.LongTask.CompactionMaxTokens
		}
	}
	return p
}

func (p HarnessProfile) systemPromptAddendum() string {
	p = p.normalized()
	var b strings.Builder
	b.WriteString("\n\n[OrangeCoding Harness]\n")
	if p.Language == OutputLanguageChinese {
		b.WriteString("- 默认使用简体中文回答；保留代码、命令、路径、API 名称和错误文本的原文。\n")
		b.WriteString("- 中文表达要直接、结构清晰，先给结论，再给必要证据和下一步。\n")
	}
	if p.LongTask.Enabled {
		b.WriteString("- 长任务要持续推进：维护简短的阶段目标，定期报告可验证进度，遇到阻塞时说明阻塞事实和下一步。\n")
		b.WriteString("- 长任务不要反复重读无关上下文；优先保留当前目标、关键决策、待验证假设和最近工具结果。\n")
	}
	b.WriteString("- 使用充分的内部推理来处理复杂任务，但不要输出隐藏推理链；输出可审计的摘要、证据和决策理由。\n")
	return b.String()
}

// ApplyToChatOptions fills provider-facing reasoning options when the caller did not.
func (p HarnessProfile) ApplyToChatOptions(opts ai.ChatOptions) ai.ChatOptions {
	p = p.normalized()
	if opts.ReasoningEffort == "" {
		opts.ReasoningEffort = string(p.Reasoning.Effort)
	}
	if opts.ReasoningBudgetTokens == nil && p.Reasoning.BudgetTokens > 0 {
		budget := p.Reasoning.BudgetTokens
		opts.ReasoningBudgetTokens = &budget
	}
	return opts
}

// ShouldRecordProgress returns true when a progress snapshot should be emitted.
func (p HarnessProfile) ShouldRecordProgress(toolCalls uint32) bool {
	p = p.normalized()
	if !p.LongTask.Enabled {
		return false
	}
	if toolCalls == 0 {
		return true
	}
	return toolCalls%p.LongTask.ProgressEveryNCalls == 0
}
