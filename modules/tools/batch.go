package tools

import (
	"context"
	"sync"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// ExecuteResult holds the outcome of executing a single tool call in a batch.
type ExecuteResult struct {
	ToolCallID string
	Content    string
	IsError    bool
	Duration   time.Duration
}

// ExecuteBatch runs all tool calls concurrently using goroutines and a sync.WaitGroup.
// Each call is dispatched in its own goroutine. Results are returned in an arbitrary order.
func ExecuteBatch(ctx context.Context, registry *ToolRegistry, calls []core.ToolCall) []ExecuteResult {
	results := make([]ExecuteResult, len(calls))
	var wg sync.WaitGroup

	for i, call := range calls {
		wg.Add(1)
		go func(idx int, c core.ToolCall) {
			defer wg.Done()
			start := time.Now()

			tool, ok := registry.Get(c.FunctionName)
			if !ok {
				results[idx] = ExecuteResult{
					ToolCallID: c.ID,
					Content:    "tool not found: " + c.FunctionName,
					IsError:    true,
					Duration:   time.Since(start),
				}
				return
			}

			out, err := tool.Execute(ctx, c.Arguments)
			dur := time.Since(start)
			if err != nil {
				results[idx] = ExecuteResult{
					ToolCallID: c.ID,
					Content:    err.Error(),
					IsError:    true,
					Duration:   dur,
				}
				return
			}

			results[idx] = ExecuteResult{
				ToolCallID: c.ID,
				Content:    out,
				IsError:    false,
				Duration:   dur,
			}
		}(i, call)
	}

	wg.Wait()
	return results
}
