package agent

import (
	"context"
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

func TestHarnessEngine_InMemoryStateMachineRecordsTraceAndCheckpoint(t *testing.T) {
	store := NewMemoryCheckpointStore()
	sessionID := core.NewSessionId()
	runID := "run-memory-1"
	engine := NewHarnessEngine(HarnessEngineConfig{
		RunID:           runID,
		SessionID:       sessionID,
		CheckpointStore: store,
	})

	cp, err := engine.Start(context.Background(), "实现长任务 harness")
	if err != nil {
		t.Fatalf("Start failed: %v", err)
	}
	if cp.State != HarnessStateBuildContext {
		t.Fatalf("state after Start = %q, want %q", cp.State, HarnessStateBuildContext)
	}

	transitions := []HarnessState{
		HarnessStateModelCall,
		HarnessStateGuardrailCheck,
		HarnessStateToolDispatch,
		HarnessStateObserve,
		HarnessStateMemoryUpdate,
		HarnessStateCheckpoint,
		HarnessStateDecideNext,
		HarnessStateCompleted,
	}
	for _, next := range transitions {
		cp, err = engine.Transition(context.Background(), next, "test transition")
		if err != nil {
			t.Fatalf("Transition(%s) failed: %v", next, err)
		}
	}

	loaded, err := store.Load(context.Background(), runID)
	if err != nil {
		t.Fatalf("Load checkpoint failed: %v", err)
	}
	if loaded.State != HarnessStateCompleted {
		t.Fatalf("loaded state = %q, want %q", loaded.State, HarnessStateCompleted)
	}
	if len(loaded.Trace) != len(transitions)+1 {
		t.Fatalf("trace length = %d, want %d", len(loaded.Trace), len(transitions)+1)
	}
	if loaded.Trace[0].From != HarnessStateInit || loaded.Trace[0].To != HarnessStateBuildContext {
		t.Fatalf("first trace = %+v, want init -> build_context", loaded.Trace[0])
	}
	if loaded.Task != "实现长任务 harness" {
		t.Fatalf("task = %q", loaded.Task)
	}
}

func TestHarnessContextBuilder_BuildsStableMemoryAndRecentBlocksWithinBudget(t *testing.T) {
	mem := NewMemoryStore(t.TempDir())
	if err := mem.Write("project-go-harness", "FACT: 当前项目主分支是 Go 版本。"); err != nil {
		t.Fatalf("memory write failed: %v", err)
	}

	manager := NewHarnessMemoryManager(mem)
	builder := NewHarnessContextBuilder(HarnessContextConfig{
		MaxTokens:       80,
		RecentMessages:  2,
		MemoryMaxBlocks: 4,
	})
	conv := core.NewConversation()
	conv.AddMessage(core.NewSystemMessage("base system"))
	conv.AddMessage(core.NewUserMessage("old message should be trimmed because it is not recent"))
	conv.AddMessage(core.NewAssistantMessage("old assistant should be trimmed because it is not recent"))
	conv.AddMessage(core.NewUserMessage("请继续强化 Go harness"))
	conv.AddMessage(core.NewAssistantMessage("好的，继续。"))

	blocks, err := builder.Build(context.Background(), HarnessContextInput{
		SystemPrompt:  "base system",
		Task:          "Go harness 长任务",
		Conversation:  conv,
		MemoryManager: manager,
	})
	if err != nil {
		t.Fatalf("Build failed: %v", err)
	}

	if len(blocks) == 0 {
		t.Fatal("expected context blocks")
	}
	if blocks[0].Kind != ContextBlockSystem || !blocks[0].Stable {
		t.Fatalf("first block = %+v, want stable system block", blocks[0])
	}
	if totalBlockTokens(blocks) > 80 {
		t.Fatalf("context token estimate = %d, want <= 80", totalBlockTokens(blocks))
	}
	if !containsBlockKind(blocks, ContextBlockMemory) {
		t.Fatalf("expected memory block in %+v", blocks)
	}
	if containsBlockText(blocks, "old message should be trimmed") {
		t.Fatalf("old non-recent message leaked into context: %+v", blocks)
	}
}

func TestHarnessMemoryManager_RecallAndLearnFacts(t *testing.T) {
	mem := NewMemoryStore(t.TempDir())
	manager := NewHarnessMemoryManager(mem)

	if err := manager.LearnObservation(context.Background(), "FACT: 用户偏好用简体中文回答。\n普通日志不用写入。"); err != nil {
		t.Fatalf("LearnObservation failed: %v", err)
	}

	blocks, err := manager.Recall(context.Background(), "中文")
	if err != nil {
		t.Fatalf("Recall failed: %v", err)
	}
	if len(blocks) != 1 {
		t.Fatalf("recall blocks = %d, want 1", len(blocks))
	}
	if blocks[0].Kind != ContextBlockMemory {
		t.Fatalf("block kind = %q, want memory", blocks[0].Kind)
	}
	if !strings.Contains(blocks[0].Content, "简体中文") {
		t.Fatalf("memory content = %q", blocks[0].Content)
	}
}

func TestHarnessGuardrailPipeline_BlocksDangerousAndRepeatedToolCalls(t *testing.T) {
	pipeline := NewGuardrailPipeline(
		NewDangerousToolGuardrail(),
		NewRepeatedToolGuardrail(2),
	)

	dangerousCall := core.ToolCall{
		ID:           "tc-danger",
		FunctionName: "bash",
		Arguments:    json.RawMessage(`{"command":"rm -rf /tmp/project"}`),
	}
	result := pipeline.Check(context.Background(), GuardrailContext{
		Phase:    GuardrailPhasePreTool,
		ToolCall: &dangerousCall,
	})
	if result.Decision != GuardrailDeny {
		t.Fatalf("dangerous command decision = %q, want deny", result.Decision)
	}

	repeatedCall := core.ToolCall{ID: "tc-repeat", FunctionName: "grep", Arguments: json.RawMessage(`{"pattern":"TODO"}`)}
	result = pipeline.Check(context.Background(), GuardrailContext{
		Phase:          GuardrailPhasePreTool,
		ToolCall:       &repeatedCall,
		RecentToolKeys: []string{ToolCallKey(repeatedCall), ToolCallKey(repeatedCall)},
	})
	if result.Decision != GuardrailDeny {
		t.Fatalf("repeated tool decision = %q, want deny", result.Decision)
	}
}

func TestFileCheckpointStore_RoundTrip(t *testing.T) {
	store := NewFileCheckpointStore(filepath.Join(t.TempDir(), "checkpoints"))
	cp := HarnessCheckpoint{
		RunID:     "run-file-1",
		SessionID: core.NewSessionId(),
		Task:      "持久化 harness checkpoint",
		State:     HarnessStateCheckpoint,
		Trace: []HarnessTraceEvent{
			{From: HarnessStateInit, To: HarnessStateBuildContext, Reason: "start"},
		},
		ContextBlocks: []ContextBlock{
			{Kind: ContextBlockTask, Content: "持久化 harness checkpoint", Stable: true},
		},
		MemoryKeys: []string{"memory-1"},
	}

	if err := store.Save(context.Background(), cp); err != nil {
		t.Fatalf("Save failed: %v", err)
	}
	loaded, err := store.Load(context.Background(), "run-file-1")
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}
	if loaded.RunID != cp.RunID || loaded.State != cp.State || loaded.Task != cp.Task {
		t.Fatalf("loaded checkpoint mismatch: %+v", loaded)
	}
	if len(loaded.Trace) != 1 || loaded.Trace[0].To != HarnessStateBuildContext {
		t.Fatalf("loaded trace mismatch: %+v", loaded.Trace)
	}
	if len(loaded.ContextBlocks) != 1 || loaded.ContextBlocks[0].Kind != ContextBlockTask {
		t.Fatalf("loaded context blocks mismatch: %+v", loaded.ContextBlocks)
	}
}

func TestAgentLoop_UsesHarnessGuardrailCheckpointAndMemory(t *testing.T) {
	provider := &mockStreamProvider{
		responses: [][]ai.StreamEvent{
			{
				{Type: "content_delta", Content: "FACT: 用户需要严格的 harness engineering。"},
				{Type: "tool_call_delta", ToolCallID: "tc-danger", ToolCallName: "bash", Arguments: `{"command":"rm -rf /tmp/project"}`},
				{Type: "usage", Usage: &ai.TokenUsage{PromptTokens: 10, CompletionTokens: 20, TotalTokens: 30}},
			},
		},
	}
	registry := tools.NewToolRegistry()
	registry.Register(&mockTool{
		name: "bash",
		execute: func(ctx context.Context, input json.RawMessage) (string, error) {
			t.Fatal("dangerous tool should be blocked before execution")
			return "", nil
		},
	})
	executor := NewToolExecutor(registry)
	sid := core.NewSessionId()
	agentCtx := NewAgentContext(sid, "/tmp/work")
	agentCtx.AddUserMessage("继续强化 harness")

	checkpoints := NewMemoryCheckpointStore()
	memory := NewHarnessMemoryManager(NewMemoryStore(t.TempDir()))
	config := DefaultLoopConfig()
	config.CheckpointStore = checkpoints
	config.MemoryManager = memory
	config.Guardrails = NewGuardrailPipeline(NewDangerousToolGuardrail())

	loop := NewAgentLoop(core.NewAgentId(), provider, executor, agentCtx, config, []ai.ToolDefinition{
		{
			Type: "function",
			Function: ai.FunctionDefinition{
				Name:        "bash",
				Description: "shell",
				Parameters:  ai.ToolParameter{Type: "object"},
			},
		},
	})

	result, err := loop.Run(context.Background(), ai.ChatOptions{}, nil)
	if err == nil {
		t.Fatal("expected guardrail error")
	}
	if !strings.Contains(err.Error(), "guardrail") {
		t.Fatalf("error = %v, want guardrail error", err)
	}
	if result.StopReason != StopReasonGuardrail {
		t.Fatalf("stop reason = %q, want %q", result.StopReason, StopReasonGuardrail)
	}

	cp, err := checkpoints.Load(context.Background(), loop.HarnessRunID())
	if err != nil {
		t.Fatalf("checkpoint load failed: %v", err)
	}
	if cp.State != HarnessStateStopped {
		t.Fatalf("checkpoint state = %q, want stopped", cp.State)
	}
	if len(cp.Trace) == 0 {
		t.Fatal("expected checkpoint trace")
	}
	blocks, err := memory.Recall(context.Background(), "harness")
	if err != nil {
		t.Fatalf("memory recall failed: %v", err)
	}
	if len(blocks) != 1 || !strings.Contains(blocks[0].Content, "harness engineering") {
		t.Fatalf("expected learned memory, got %+v", blocks)
	}
}
