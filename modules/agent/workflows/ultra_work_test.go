package workflows

import (
	"context"
	"testing"

	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/tools"
)

type testProvider struct{}

func (p testProvider) Name() string { return "test" }
func (p testProvider) ChatCompletion(ctx context.Context, messages []ai.ChatMessage, tools []ai.ToolDefinition, opts ai.ChatOptions) (*ai.AiResponse, error) {
	return &ai.AiResponse{Content: "ok"}, nil
}
func (p testProvider) ChatCompletionStream(ctx context.Context, messages []ai.ChatMessage, tools []ai.ToolDefinition, opts ai.ChatOptions) (<-chan ai.StreamEvent, error) {
	ch := make(chan ai.StreamEvent, 1)
	close(ch)
	return ch, nil
}

func TestNewUltraWorkKeepsDefaultLongTaskHarnessPolicy(t *testing.T) {
	workflow := NewUltraWork(testProvider{}, tools.NewToolRegistry(), t.TempDir(), 7)
	config := workflow.loop.Config()

	if config.MaxIterations != 7 {
		t.Fatalf("MaxIterations = %d, want 7", config.MaxIterations)
	}
	if !config.LongTask.Enabled {
		t.Fatal("LongTask.Enabled = false, want default long-task harness policy")
	}
	if config.LongTask.MaxToolCalls == 0 {
		t.Fatal("LongTask.MaxToolCalls = 0, want default tool budget")
	}
}
