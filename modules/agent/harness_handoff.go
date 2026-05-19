package agent

import (
	"context"
	"fmt"
	"sync"

	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
)

// HandoffRequest transfers control from one agent to another.
type HandoffRequest struct {
	FromAgentID   core.AgentId
	ToAgentID     core.AgentId
	Task          string
	Conversation  []core.Message
	ToolCallsMade uint32
	MemoryKeys    []string
	Metadata      map[string]string
}

// HandoffResult is returned when an agent completes a handoff.
type HandoffResult struct {
	FromAgentID   core.AgentId
	ToAgentID     core.AgentId
	Completed     bool
	ToolCallsMade uint32
	Summary       string
	Error         string
}

// HandoffHandler processes agent handoffs.
type HandoffHandler interface {
	CanHandoff(ctx context.Context, req HandoffRequest) (bool, error)
	ExecuteHandoff(ctx context.Context, req HandoffRequest) (HandoffResult, error)
}

// ToolUseBudget tracks per-tool call counts.
type ToolUseBudget struct {
	// MaxUses is the maximum allowed calls per tool. 0 = unlimited.
	MaxUses map[string]uint32
	// counts tracks actual usage.
	counts map[string]uint32
}

// NewToolUseBudget creates a tool use budget.
func NewToolUseBudget() *ToolUseBudget {
	return &ToolUseBudget{
		MaxUses: make(map[string]uint32),
		counts:  make(map[string]uint32),
	}
}

// SetMaxUses sets the maximum allowed calls for a tool.
func (b *ToolUseBudget) SetMaxUses(toolName string, max uint32) {
	b.MaxUses[toolName] = max
}

// RecordCall records that a tool was called. Returns true if the call is allowed.
func (b *ToolUseBudget) RecordCall(toolName string) bool {
	max, hasMax := b.MaxUses[toolName]
	if !hasMax || max == 0 {
		b.counts[toolName]++
		return true
	}
	current := b.counts[toolName]
	if current >= max {
		return false
	}
	b.counts[toolName]++
	return true
}

// Remaining returns how many more calls are allowed for a tool.
func (b *ToolUseBudget) Remaining(toolName string) uint32 {
	max, hasMax := b.MaxUses[toolName]
	if !hasMax || max == 0 {
		return ^uint32(0) // unlimited
	}
	used := b.counts[toolName]
	if used >= max {
		return 0
	}
	return max - used
}

// Used returns how many times a tool has been called.
func (b *ToolUseBudget) Used(toolName string) uint32 {
	return b.counts[toolName]
}

// AgentModelSettings holds per-agent model configuration.
type AgentModelSettings struct {
	Model           string
	Temperature     *float64
	TopP            *float64
	MaxTokens       *uint32
	ReasoningEffort ReasoningEffort
	ReasoningBudget *uint32
}

// ApplyToChatOptions applies agent-specific model settings to ChatOptions.
// Non-zero/non-nil values override the existing options.
func (s AgentModelSettings) ApplyToChatOptions(opts ai.ChatOptions) ai.ChatOptions {
	if s.Model != "" {
		opts.Model = s.Model
	}
	if s.Temperature != nil {
		opts.Temperature = s.Temperature
	}
	if s.TopP != nil {
		opts.TopP = s.TopP
	}
	if s.MaxTokens != nil {
		opts.MaxTokens = s.MaxTokens
	}
	if s.ReasoningEffort != "" {
		opts.ReasoningEffort = string(s.ReasoningEffort)
	}
	if s.ReasoningBudget != nil {
		opts.ReasoningBudgetTokens = s.ReasoningBudget
	}
	return opts
}

// OrchestratorTask represents a decomposed task for an agent.
type OrchestratorTask struct {
	ID          string
	AgentID     core.AgentId
	Description string
	Scope       []string // file paths or directories
	DependsOn   []string // task IDs this depends on
	Priority    int
}

// OrchestratorResult summarizes the outcome of a decomposed task.
type OrchestratorResult struct {
	TaskID  string
	Success bool
	Summary string
	Error   string
}

// Orchestrator decomposes a task and coordinates sub-agents.
type Orchestrator struct {
	mu      sync.RWMutex
	tasks   map[string]OrchestratorTask
	results map[string]OrchestratorResult
}

// NewOrchestrator creates a new orchestrator.
func NewOrchestrator() *Orchestrator {
	return &Orchestrator{
		tasks:   make(map[string]OrchestratorTask),
		results: make(map[string]OrchestratorResult),
	}
}

// AddTask registers a task for orchestration.
func (o *Orchestrator) AddTask(task OrchestratorTask) error {
	if task.ID == "" {
		return fmt.Errorf("orchestrator: task ID is required")
	}
	o.mu.Lock()
	defer o.mu.Unlock()
	o.tasks[task.ID] = task
	return nil
}

// RecordResult records the outcome of a task.
func (o *Orchestrator) RecordResult(result OrchestratorResult) {
	o.mu.Lock()
	defer o.mu.Unlock()
	o.results[result.TaskID] = result
}

// ReadyTasks returns tasks whose dependencies are all completed successfully.
func (o *Orchestrator) ReadyTasks() []OrchestratorTask {
	o.mu.RLock()
	defer o.mu.RUnlock()
	var ready []OrchestratorTask
	for _, task := range o.tasks {
		if _, done := o.results[task.ID]; done {
			continue
		}
		allDepsMet := true
		for _, depID := range task.DependsOn {
			result, exists := o.results[depID]
			if !exists || !result.Success {
				allDepsMet = false
				break
			}
		}
		if allDepsMet {
			ready = append(ready, task)
		}
	}
	return ready
}

// AllCompleted returns true if all tasks have results.
func (o *Orchestrator) AllCompleted() bool {
	o.mu.RLock()
	defer o.mu.RUnlock()
	for _, task := range o.tasks {
		if _, exists := o.results[task.ID]; !exists {
			return false
		}
	}
	return true
}

// Summary returns a summary of all task results.
func (o *Orchestrator) Summary() string {
	o.mu.RLock()
	defer o.mu.RUnlock()
	total := len(o.tasks)
	completed := 0
	failed := 0
	for _, r := range o.results {
		if r.Success {
			completed++
		} else {
			failed++
		}
	}
	return fmt.Sprintf("tasks: %d total, %d completed, %d failed", total, completed, failed)
}
