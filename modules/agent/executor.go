package agent

import (
	"context"
	"encoding/json"
	"sync"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// ExecuteResult holds the outcome of executing a single tool call.
type ExecuteResult = tools.ExecuteResult

// ToolExecutor dispatches tool calls to the tool registry and collects results.
type ToolExecutor struct {
	registry *tools.ToolRegistry
	timeout  time.Duration
}

// NewToolExecutor creates a new ToolExecutor backed by the given registry.
func NewToolExecutor(registry *tools.ToolRegistry) *ToolExecutor {
	return &ToolExecutor{
		registry: registry,
		timeout:  30 * time.Second,
	}
}

// Execute runs a single tool call. It looks up the tool by call.FunctionName,
// executes it, and returns the result with timing information.
func (e *ToolExecutor) Execute(ctx context.Context, call core.ToolCall) ExecuteResult {
	start := time.Now()

	tool, ok := e.registry.Get(call.FunctionName)
	if !ok {
		return ExecuteResult{
			ToolCallID: call.ID,
			Content:    "tool not found: " + call.FunctionName,
			IsError:    true,
			Duration:   time.Since(start),
		}
	}

	// Apply per-call timeout
	execCtx, cancel := context.WithTimeout(ctx, e.timeout)
	defer cancel()

	out, err := tool.Execute(execCtx, call.Arguments)
	dur := time.Since(start)

	if err != nil {
		return ExecuteResult{
			ToolCallID: call.ID,
			Content:    err.Error(),
			IsError:    true,
			Duration:   dur,
		}
	}

	return ExecuteResult{
		ToolCallID: call.ID,
		Content:    out,
		IsError:    false,
		Duration:   dur,
	}
}

// maxConcurrentTools is the maximum number of tool calls that can execute in parallel.
const maxConcurrentTools = 8

// ExecuteBatch runs tool calls concurrently with a bounded concurrency limit.
// Results maintain the same order as the input calls.
func (e *ToolExecutor) ExecuteBatch(ctx context.Context, calls []core.ToolCall) []ExecuteResult {
	results := make([]ExecuteResult, len(calls))
	var wg sync.WaitGroup
	sem := make(chan struct{}, maxConcurrentTools)

	for i, call := range calls {
		wg.Add(1)
		go func(idx int, c core.ToolCall) {
			defer wg.Done()
			sem <- struct{}{}        // acquire
			defer func() { <-sem }() // release
			results[idx] = e.Execute(ctx, c)
		}(i, call)
	}

	wg.Wait()
	return results
}

// SetTimeout configures the per-call execution timeout.
func (e *ToolExecutor) SetTimeout(d time.Duration) {
	e.timeout = d
}

// Registry exposes the underlying tool registry for callers that need it.
func (e *ToolExecutor) Registry() *tools.ToolRegistry {
	return e.registry
}

// FilteredRegistry creates a new registry containing only the named tools.
// Tools not found in the parent registry are silently skipped.
func FilteredRegistry(parent *tools.ToolRegistry, names []string) *tools.ToolRegistry {
	filtered := tools.NewToolRegistry()
	nameSet := make(map[string]bool, len(names))
	for _, n := range names {
		nameSet[n] = true
	}
	for _, t := range parent.List() {
		if nameSet[t.Name()] {
			filtered.Register(t)
		}
	}
	return filtered
}

// ToolCallFromAI converts an ai.ToolCall to a core.ToolCall for execution.
func ToolCallFromAI(tc struct {
	ID       string
	Function struct {
		Name      string
		Arguments string
	}
}) core.ToolCall {
	return core.ToolCall{
		ID:           tc.ID,
		FunctionName: tc.Function.Name,
		Arguments:    json.RawMessage(tc.Function.Arguments),
	}
}
