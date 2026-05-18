package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

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

// --- Phase 13: Guardrail full activation tests ---

func TestTokenBudgetGuardrail_WarnsOnBudgetExceeded(t *testing.T) {
	g := NewTokenBudgetGuardrail()

	// Should warn when token estimate exceeds max
	result := g.Check(context.Background(), GuardrailContext{
		Phase:         GuardrailPhasePreModel,
		TokenEstimate: 5000,
		MaxTokens:     4000,
	})
	if result.Decision != GuardrailWarn {
		t.Fatalf("decision = %q, want warn", result.Decision)
	}
	if result.Name != "token_budget" {
		t.Fatalf("name = %q, want token_budget", result.Name)
	}

	// Should allow when under budget
	result = g.Check(context.Background(), GuardrailContext{
		Phase:         GuardrailPhasePreModel,
		TokenEstimate: 3000,
		MaxTokens:     4000,
	})
	if result.Decision != GuardrailAllow {
		t.Fatalf("decision = %q, want allow", result.Decision)
	}

	// Should allow for non-applicable phases
	result = g.Check(context.Background(), GuardrailContext{
		Phase:         GuardrailPhasePreTool,
		TokenEstimate: 5000,
		MaxTokens:     4000,
	})
	if result.Decision != GuardrailAllow {
		t.Fatalf("decision = %q, want allow for pre_tool phase", result.Decision)
	}
}

func TestOutputLengthGuardrail_WarnsOnLongOutput(t *testing.T) {
	g := NewOutputLengthGuardrail()

	// Should warn when output exceeds 50000 chars
	longOutput := strings.Repeat("x", 50001)
	result := g.Check(context.Background(), GuardrailContext{
		Phase:  GuardrailPhaseFinalOutput,
		Output: longOutput,
	})
	if result.Decision != GuardrailWarn {
		t.Fatalf("decision = %q, want warn", result.Decision)
	}

	// Should allow short output
	result = g.Check(context.Background(), GuardrailContext{
		Phase:  GuardrailPhaseFinalOutput,
		Output: "short output",
	})
	if result.Decision != GuardrailAllow {
		t.Fatalf("decision = %q, want allow", result.Decision)
	}

	// Should skip non-final_output phases
	result = g.Check(context.Background(), GuardrailContext{
		Phase:  GuardrailPhasePostTool,
		Output: longOutput,
	})
	if result.Decision != GuardrailAllow {
		t.Fatalf("decision = %q, want allow for post_tool phase", result.Decision)
	}
}

func TestOutputLengthGuardrail_CustomLimit(t *testing.T) {
	g := NewOutputLengthGuardrailWithLimit(100)
	result := g.Check(context.Background(), GuardrailContext{
		Phase:  GuardrailPhaseFinalOutput,
		Output: strings.Repeat("x", 101),
	})
	if result.Decision != GuardrailWarn {
		t.Fatalf("decision = %q, want warn with custom limit", result.Decision)
	}
}

func TestGuardrailLogger_RecordsAndRetrieves(t *testing.T) {
	logger := NewGuardrailLogger()

	logger.Log(GuardrailLogEntry{Name: "g1", Decision: GuardrailAllow, Phase: GuardrailPhasePreModel, Timestamp: time.Now().UTC()})
	logger.Log(GuardrailLogEntry{Name: "g2", Decision: GuardrailWarn, Reason: "budget", Phase: GuardrailPhasePreModel, Timestamp: time.Now().UTC()})
	logger.Log(GuardrailLogEntry{Name: "g3", Decision: GuardrailDeny, Reason: "dangerous", Phase: GuardrailPhasePreTool, Timestamp: time.Now().UTC()})
	logger.Log(GuardrailLogEntry{Name: "g4", Decision: GuardrailWarn, Reason: "length", Phase: GuardrailPhaseFinalOutput, Timestamp: time.Now().UTC()})

	if logger.Len() != 4 {
		t.Fatalf("Len() = %d, want 4", logger.Len())
	}

	recent := logger.Recent(2)
	if len(recent) != 2 {
		t.Fatalf("Recent(2) = %d entries, want 2", len(recent))
	}
	if recent[0].Name != "g3" {
		t.Fatalf("Recent(2)[0].Name = %q, want g3", recent[0].Name)
	}

	warnings := logger.Warnings()
	if len(warnings) != 2 {
		t.Fatalf("Warnings() = %d entries, want 2", len(warnings))
	}

	allRecent := logger.Recent(10)
	if len(allRecent) != 4 {
		t.Fatalf("Recent(10) = %d entries, want 4", len(allRecent))
	}
}

func TestLLMGuardrail_RejectsUnsafeContent(t *testing.T) {
	g := NewLLMGuardrail(LLMGuardrailConfig{
		Phase:  GuardrailPhasePostTool,
		Prompt: "Is this content safe?",
		Provider: func(ctx context.Context, prompt, content string) (bool, error) {
			return !strings.Contains(content, "DANGEROUS"), nil
		},
	})

	// Safe content
	result := g.Check(context.Background(), GuardrailContext{
		Phase:  GuardrailPhasePostTool,
		Output: "all good here",
	})
	if result.Decision != GuardrailAllow {
		t.Fatalf("safe content: decision = %q, want allow", result.Decision)
	}

	// Unsafe content
	result = g.Check(context.Background(), GuardrailContext{
		Phase:  GuardrailPhasePostTool,
		Output: "DANGEROUS output detected",
	})
	if result.Decision != GuardrailDeny {
		t.Fatalf("unsafe content: decision = %q, want deny", result.Decision)
	}

	// Wrong phase should pass
	result = g.Check(context.Background(), GuardrailContext{
		Phase:  GuardrailPhasePreModel,
		Output: "DANGEROUS output detected",
	})
	if result.Decision != GuardrailAllow {
		t.Fatalf("wrong phase: decision = %q, want allow", result.Decision)
	}
}

func TestLLMGuardrail_HandlesProviderError(t *testing.T) {
	g := NewLLMGuardrail(LLMGuardrailConfig{
		Phase:  GuardrailPhaseFinalOutput,
		Prompt: "check",
		Provider: func(ctx context.Context, prompt, content string) (bool, error) {
			return false, fmt.Errorf("provider unavailable")
		},
	})
	result := g.Check(context.Background(), GuardrailContext{
		Phase:  GuardrailPhaseFinalOutput,
		Output: "test",
	})
	if result.Decision != GuardrailDeny {
		t.Fatalf("provider error: decision = %q, want deny", result.Decision)
	}
}

func TestAgentLoop_GuardrailLogger_CapturesAllPhases(t *testing.T) {
	logger := NewGuardrailLogger()

	// A simple loop that completes without tool calls
	provider := &mockStreamProvider{
		responses: [][]ai.StreamEvent{
			{
				{Type: "content_delta", Content: "done"},
				{Type: "usage", Usage: &ai.TokenUsage{PromptTokens: 5, CompletionTokens: 5, TotalTokens: 10}},
			},
		},
	}
	registry := tools.NewToolRegistry()
	executor := NewToolExecutor(registry)
	sid := core.NewSessionId()
	agentCtx := NewAgentContext(sid, "/tmp/work")
	agentCtx.AddUserMessage("test")

	config := DefaultLoopConfig()
	config.GuardrailLogger = logger
	config.Guardrails = NewGuardrailPipeline(
		NewDangerousToolGuardrail(),
		NewRepeatedToolGuardrail(3),
		NewTokenBudgetGuardrail(),
		NewOutputLengthGuardrail(),
	)

	loop := NewAgentLoop(core.NewAgentId(), provider, executor, agentCtx, config, nil)
	result, err := loop.Run(context.Background(), ai.ChatOptions{}, nil)
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}
	if result.StopReason != StopReasonCompleted {
		t.Fatalf("StopReason = %q, want completed", result.StopReason)
	}

	// Should have logged pre_model + final_output phases
	if logger.Len() < 2 {
		t.Fatalf("expected at least 2 guardrail log entries, got %d", logger.Len())
	}

	phases := map[GuardrailPhase]bool{}
	for _, entry := range logger.Recent(logger.Len()) {
		phases[entry.Phase] = true
	}
	if !phases[GuardrailPhasePreModel] {
		t.Fatal("missing pre_model phase in guardrail log")
	}
	if !phases[GuardrailPhaseFinalOutput] {
		t.Fatal("missing final_output phase in guardrail log")
	}
}

func TestAgentLoop_PreModelGuardrailDeny(t *testing.T) {
	provider := &mockStreamProvider{
		responses: [][]ai.StreamEvent{
			{
				{Type: "content_delta", Content: "should not reach here"},
			},
		},
	}
	registry := tools.NewToolRegistry()
	executor := NewToolExecutor(registry)
	sid := core.NewSessionId()
	agentCtx := NewAgentContext(sid, "/tmp/work")
	agentCtx.AddUserMessage("test")

	config := DefaultLoopConfig()
	// Create a guardrail that denies all pre_model calls
	config.Guardrails = NewGuardrailPipeline(&denyAllGuardrail{phase: GuardrailPhasePreModel})

	loop := NewAgentLoop(core.NewAgentId(), provider, executor, agentCtx, config, nil)
	result, err := loop.Run(context.Background(), ai.ChatOptions{}, nil)
	if err == nil {
		t.Fatal("expected pre-model guardrail error")
	}
	if !strings.Contains(err.Error(), "pre-model guardrail") {
		t.Fatalf("error = %v, want pre-model guardrail error", err)
	}
	if result.StopReason != StopReasonGuardrail {
		t.Fatalf("StopReason = %q, want %q", result.StopReason, StopReasonGuardrail)
	}
}

func TestAgentLoop_FinalOutputGuardrailDeny(t *testing.T) {
	provider := &mockStreamProvider{
		responses: [][]ai.StreamEvent{
			{
				{Type: "content_delta", Content: "DANGEROUS final output"},
				{Type: "usage", Usage: &ai.TokenUsage{PromptTokens: 5, CompletionTokens: 5, TotalTokens: 10}},
			},
		},
	}
	registry := tools.NewToolRegistry()
	executor := NewToolExecutor(registry)
	sid := core.NewSessionId()
	agentCtx := NewAgentContext(sid, "/tmp/work")
	agentCtx.AddUserMessage("test")

	config := DefaultLoopConfig()
	config.Guardrails = NewGuardrailPipeline(
		NewDangerousToolGuardrail(),
		NewLLMGuardrail(LLMGuardrailConfig{
			Phase:  GuardrailPhaseFinalOutput,
			Prompt: "Is this safe?",
			Provider: func(ctx context.Context, prompt, content string) (bool, error) {
				return !strings.Contains(content, "DANGEROUS"), nil
			},
		}),
	)

	loop := NewAgentLoop(core.NewAgentId(), provider, executor, agentCtx, config, nil)
	result, err := loop.Run(context.Background(), ai.ChatOptions{}, nil)
	if err == nil {
		t.Fatal("expected final-output guardrail error")
	}
	if !strings.Contains(err.Error(), "final-output guardrail") {
		t.Fatalf("error = %v, want final-output guardrail error", err)
	}
	if result.StopReason != StopReasonGuardrail {
		t.Fatalf("StopReason = %q, want %q", result.StopReason, StopReasonGuardrail)
	}
}

// denyAllGuardrail is a test helper that denies a specific phase.
type denyAllGuardrail struct {
	phase GuardrailPhase
}

func (d *denyAllGuardrail) Name() string { return "deny_all" }
func (d *denyAllGuardrail) Check(ctx context.Context, input GuardrailContext) GuardrailResult {
	if input.Phase == d.phase {
		return GuardrailResult{Decision: GuardrailDeny, Reason: "denied by test guardrail", Name: d.Name()}
	}
	return GuardrailResult{Decision: GuardrailAllow, Name: d.Name()}
}

// --- Phase 14: Checkpoint production tests ---

func TestMemoryCheckpointStore_ListAndDelete(t *testing.T) {
	store := NewMemoryCheckpointStore()
	sid := core.NewSessionId()

	// Save multiple checkpoints
	for i := 0; i < 3; i++ {
		cp := HarnessCheckpoint{
			RunID:     fmt.Sprintf("run-prefix-%d", i),
			SessionID: sid,
			Task:      fmt.Sprintf("task %d", i),
			State:     HarnessStateCompleted,
		}
		if err := store.Save(context.Background(), cp); err != nil {
			t.Fatalf("Save %d failed: %v", i, err)
		}
	}

	// List all
	summaries, err := store.List(context.Background(), "")
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}
	if len(summaries) != 3 {
		t.Fatalf("List() = %d, want 3", len(summaries))
	}

	// List with prefix
	summaries, err = store.List(context.Background(), "run-prefix-1")
	if err != nil {
		t.Fatalf("List with prefix failed: %v", err)
	}
	if len(summaries) != 1 {
		t.Fatalf("List(prefix) = %d, want 1", len(summaries))
	}
	if summaries[0].RunID != "run-prefix-1" {
		t.Fatalf("RunID = %q, want run-prefix-1", summaries[0].RunID)
	}

	// Delete one
	if err := store.Delete(context.Background(), "run-prefix-0"); err != nil {
		t.Fatalf("Delete failed: %v", err)
	}
	summaries, _ = store.List(context.Background(), "")
	if len(summaries) != 2 {
		t.Fatalf("List after delete = %d, want 2", len(summaries))
	}

	// Delete non-existent
	err = store.Delete(context.Background(), "nonexistent")
	if err == nil {
		t.Fatal("expected error deleting nonexistent checkpoint")
	}
}

func TestFileCheckpointStore_AtomicWrite(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "atomic-test")
	store := NewFileCheckpointStore(dir)

	cp := HarnessCheckpoint{
		RunID:     "run-atomic",
		SessionID: core.NewSessionId(),
		Task:      "atomic write test",
		State:     HarnessStateCheckpoint,
	}

	// Save should create the file atomically
	if err := store.Save(context.Background(), cp); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Verify no temp file remains
	tmpPath := filepath.Join(dir, "run-atomic.json.tmp")
	if _, err := os.Stat(tmpPath); !os.IsNotExist(err) {
		t.Fatal("temp file should not exist after atomic save")
	}

	// Load should work
	loaded, err := store.Load(context.Background(), "run-atomic")
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}
	if loaded.RunID != "run-atomic" {
		t.Fatalf("loaded RunID = %q", loaded.RunID)
	}
}

func TestFileCheckpointStore_ListAndDelete(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "list-test")
	store := NewFileCheckpointStore(dir)
	sid := core.NewSessionId()

	// Save multiple
	for i := 0; i < 4; i++ {
		cp := HarnessCheckpoint{
			RunID:     fmt.Sprintf("run-list-%d", i),
			SessionID: sid,
			Task:      fmt.Sprintf("list task %d", i),
			State:     HarnessStateCompleted,
		}
		if err := store.Save(context.Background(), cp); err != nil {
			t.Fatalf("Save %d failed: %v", i, err)
		}
	}

	// List all
	summaries, err := store.List(context.Background(), "")
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}
	if len(summaries) != 4 {
		t.Fatalf("List() = %d, want 4", len(summaries))
	}

	// List should be sorted by UpdatedAt descending
	for i := 1; i < len(summaries); i++ {
		if summaries[i].UpdatedAt.After(summaries[i-1].UpdatedAt) {
			t.Fatalf("List not sorted by UpdatedAt desc at index %d", i)
		}
	}

	// List with prefix
	summaries, err = store.List(context.Background(), "run-list-2")
	if err != nil {
		t.Fatalf("List prefix failed: %v", err)
	}
	if len(summaries) != 1 || summaries[0].RunID != "run-list-2" {
		t.Fatalf("List(prefix) = %+v, want run-list-2", summaries)
	}

	// Delete one
	if err := store.Delete(context.Background(), "run-list-1"); err != nil {
		t.Fatalf("Delete failed: %v", err)
	}
	summaries, _ = store.List(context.Background(), "")
	if len(summaries) != 3 {
		t.Fatalf("List after delete = %d, want 3", len(summaries))
	}

	// Delete non-existent (should not error)
	if err := store.Delete(context.Background(), "nonexistent"); err != nil {
		t.Fatalf("Delete nonexistent failed: %v", err)
	}
}

func TestFileCheckpointStore_TTLExpiry(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "ttl-test")
	ttl := 1 * time.Hour
	store := NewFileCheckpointStoreWithTTL(dir, ttl)

	// Save a fresh checkpoint via store.Save (sets UpdatedAt=now)
	newCp := HarnessCheckpoint{
		RunID:     "run-new",
		SessionID: core.NewSessionId(),
		Task:      "should survive",
		State:     HarnessStateCompleted,
	}
	if err := store.Save(context.Background(), newCp); err != nil {
		t.Fatalf("Save new failed: %v", err)
	}

	// Manually write an old checkpoint with expired timestamp
	oldCp := HarnessCheckpoint{
		RunID:     "run-old",
		SessionID: core.NewSessionId(),
		Task:      "should expire",
		State:     HarnessStateCompleted,
		UpdatedAt: time.Now().UTC().Add(-2 * time.Hour), // expired by TTL=1h
	}
	data, _ := json.MarshalIndent(oldCp, "", "  ")
	os.MkdirAll(dir, 0o755)
	os.WriteFile(filepath.Join(dir, "run-old.json"), data, 0o644)

	// List should exclude expired, keep fresh
	summaries, err := store.List(context.Background(), "")
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}
	if len(summaries) != 1 {
		t.Fatalf("List with TTL returned %d entries, want 1", len(summaries))
	}
	if summaries[0].RunID != "run-new" {
		t.Fatalf("RunID = %q, want run-new", summaries[0].RunID)
	}
}

func TestFileCheckpointStore_CleanupExpired(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "cleanup-test")
	ttl := 50 * time.Millisecond
	store := NewFileCheckpointStoreWithTTL(dir, ttl)

	// Write expired checkpoint
	old := HarnessCheckpoint{
		RunID:     "run-expired",
		SessionID: core.NewSessionId(),
		Task:      "expired",
		State:     HarnessStateCompleted,
	}
	old.UpdatedAt = time.Now().UTC().Add(-1 * time.Hour)
	data, _ := json.MarshalIndent(old, "", "  ")
	os.MkdirAll(dir, 0o755)
	os.WriteFile(filepath.Join(dir, "run-expired.json"), data, 0o644)

	time.Sleep(100 * time.Millisecond)

	cleaned, err := store.CleanupExpired(context.Background())
	if err != nil {
		t.Fatalf("CleanupExpired failed: %v", err)
	}
	if cleaned != 1 {
		t.Fatalf("cleaned = %d, want 1", cleaned)
	}
}

// --- Phase 15: Trace store tests ---

func TestMemoryTraceStore_AppendAndQuery(t *testing.T) {
	store := NewMemoryTraceStore()
	sid := core.NewSessionId()
	ctx := context.Background()

	// Append events
	for i := 0; i < 5; i++ {
		store.Append(ctx, TraceEvent{
			SchemaVersion: TraceSchemaVersion,
			RunID:         fmt.Sprintf("run-%d", i),
			SessionID:     sid,
			FromState:     HarnessStateInit,
			ToState:       HarnessStateBuildContext,
			Reason:        fmt.Sprintf("event %d", i),
			CreatedAt:     time.Now().UTC().Add(time.Duration(i) * time.Minute),
		})
	}

	// Query all
	events, err := store.Query(ctx, TraceQuery{})
	if err != nil {
		t.Fatalf("Query failed: %v", err)
	}
	if len(events) != 5 {
		t.Fatalf("Query() = %d, want 5", len(events))
	}
	// Should be sorted descending
	if events[0].Reason != "event 4" {
		t.Fatalf("first event reason = %q, want event 4", events[0].Reason)
	}

	// Query by RunID
	events, err = store.Query(ctx, TraceQuery{RunID: "run-2"})
	if err != nil {
		t.Fatalf("Query RunID failed: %v", err)
	}
	if len(events) != 1 || events[0].RunID != "run-2" {
		t.Fatalf("Query(RunID) = %+v", events)
	}

	// Query by state
	events, err = store.Query(ctx, TraceQuery{ToState: HarnessStateBuildContext})
	if err != nil {
		t.Fatalf("Query state failed: %v", err)
	}
	if len(events) != 5 {
		t.Fatalf("Query(ToState) = %d, want 5", len(events))
	}

	// Query with limit
	events, err = store.Query(ctx, TraceQuery{Limit: 2})
	if err != nil {
		t.Fatalf("Query limit failed: %v", err)
	}
	if len(events) != 2 {
		t.Fatalf("Query(Limit=2) = %d, want 2", len(events))
	}
}

func TestFileTraceStore_AppendAndQuery(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "traces")
	store := NewFileTraceStore(dir)
	sid := core.NewSessionId()
	ctx := context.Background()

	// Append events for one run
	for i := 0; i < 3; i++ {
		store.Append(ctx, TraceEvent{
			RunID:     "run-trace-1",
			SessionID: sid,
			FromState: HarnessStateBuildContext,
			ToState:   HarnessStateModelCall,
			Reason:    fmt.Sprintf("step %d", i),
			CreatedAt: time.Now().UTC().Add(time.Duration(i) * time.Second),
		})
	}

	// Append event for another run
	store.Append(ctx, TraceEvent{
		RunID:     "run-trace-2",
		SessionID: sid,
		FromState: HarnessStateInit,
		ToState:   HarnessStateBuildContext,
		CreatedAt: time.Now().UTC(),
	})

	// Query by run ID
	events, err := store.Query(ctx, TraceQuery{RunID: "run-trace-1"})
	if err != nil {
		t.Fatalf("Query failed: %v", err)
	}
	if len(events) != 3 {
		t.Fatalf("Query(run-trace-1) = %d, want 3", len(events))
	}

	// Query all runs
	events, err = store.Query(ctx, TraceQuery{})
	if err != nil {
		t.Fatalf("Query all failed: %v", err)
	}
	if len(events) != 4 {
		t.Fatalf("Query() = %d, want 4", len(events))
	}
}

func TestTraceEventsToSpans(t *testing.T) {
	events := []TraceEvent{
		{RunID: "run-1", FromState: HarnessStateInit, ToState: HarnessStateBuildContext, Reason: "start", CreatedAt: time.Now().UTC()},
	}
	spans := TraceEventsToSpans(events)
	if len(spans) != 1 {
		t.Fatalf("spans = %d, want 1", len(spans))
	}
	if spans[0].TraceID != "run-1" {
		t.Fatalf("span TraceID = %q, want run-1", spans[0].TraceID)
	}
	if spans[0].Name != "init -> build_context" {
		t.Fatalf("span Name = %q", spans[0].Name)
	}
	if spans[0].Attrs["run_id"] != "run-1" {
		t.Fatalf("span Attrs = %+v", spans[0].Attrs)
	}
}

// --- Phase 16: Handoff and orchestration tests ---

func TestToolUseBudget_AllowsAndBlocks(t *testing.T) {
	budget := NewToolUseBudget()
	budget.SetMaxUses("grep", 2)

	// First two calls allowed
	if !budget.RecordCall("grep") {
		t.Fatal("first call should be allowed")
	}
	if !budget.RecordCall("grep") {
		t.Fatal("second call should be allowed")
	}
	// Third call blocked
	if budget.RecordCall("grep") {
		t.Fatal("third call should be blocked")
	}

	// Unlimited tool
	if !budget.RecordCall("read") {
		t.Fatal("unlimited tool should always be allowed")
	}
	if !budget.RecordCall("read") {
		t.Fatal("unlimited tool should always be allowed")
	}
}

func TestToolUseBudget_Remaining(t *testing.T) {
	budget := NewToolUseBudget()
	budget.SetMaxUses("grep", 3)

	if budget.Remaining("grep") != 3 {
		t.Fatalf("remaining = %d, want 3", budget.Remaining("grep"))
	}
	budget.RecordCall("grep")
	if budget.Remaining("grep") != 2 {
		t.Fatalf("remaining = %d, want 2", budget.Remaining("grep"))
	}
	// Unlimited
	if budget.Remaining("read") != ^uint32(0) {
		t.Fatalf("unlimited remaining should be max uint32")
	}
}

func TestToolUseBudget_Used(t *testing.T) {
	budget := NewToolUseBudget()
	if budget.Used("grep") != 0 {
		t.Fatalf("used = %d, want 0", budget.Used("grep"))
	}
	budget.RecordCall("grep")
	budget.RecordCall("grep")
	if budget.Used("grep") != 2 {
		t.Fatalf("used = %d, want 2", budget.Used("grep"))
	}
}

func TestAgentModelSettings_ApplyToChatOptions(t *testing.T) {
	opts := ai.ChatOptions{}
	temp := 0.5
	maxTokens := uint32(4096)
	reasonBudget := uint32(8192)

	settings := AgentModelSettings{
		Model:           "gpt-4o",
		Temperature:     &temp,
		MaxTokens:       &maxTokens,
		ReasoningEffort: ReasoningEffortHigh,
		ReasoningBudget: &reasonBudget,
	}

	result := settings.ApplyToChatOptions(opts)
	if result.Model != "gpt-4o" {
		t.Fatalf("Model = %q, want gpt-4o", result.Model)
	}
	if result.Temperature == nil || *result.Temperature != 0.5 {
		t.Fatalf("Temperature = %v, want 0.5", result.Temperature)
	}
	if result.MaxTokens == nil || *result.MaxTokens != 4096 {
		t.Fatalf("MaxTokens = %d, want 4096", result.MaxTokens)
	}
	if result.ReasoningEffort != "high" {
		t.Fatalf("ReasoningEffort = %q, want high", result.ReasoningEffort)
	}
	if result.ReasoningBudgetTokens == nil || *result.ReasoningBudgetTokens != 8192 {
		t.Fatalf("ReasoningBudgetTokens = %v, want 8192", result.ReasoningBudgetTokens)
	}
}

func TestOrchestrator_AddAndExecute(t *testing.T) {
	orch := NewOrchestrator()

	// Add tasks with dependencies
	orch.AddTask(OrchestratorTask{ID: "t1", Description: "first task"})
	orch.AddTask(OrchestratorTask{ID: "t2", Description: "second task", DependsOn: []string{"t1"}})
	orch.AddTask(OrchestratorTask{ID: "t3", Description: "third task", DependsOn: []string{"t1", "t2"}})

	// Initially only t1 is ready
	ready := orch.ReadyTasks()
	if len(ready) != 1 || ready[0].ID != "t1" {
		t.Fatalf("ready tasks = %+v, want t1", ready)
	}

	// Complete t1
	orch.RecordResult(OrchestratorResult{TaskID: "t1", Success: true})

	// Now t2 is ready
	ready = orch.ReadyTasks()
	if len(ready) != 1 || ready[0].ID != "t2" {
		t.Fatalf("ready tasks after t1 = %+v, want t2", ready)
	}

	// Complete t2
	orch.RecordResult(OrchestratorResult{TaskID: "t2", Success: true})

	// Now t3 is ready
	ready = orch.ReadyTasks()
	if len(ready) != 1 || ready[0].ID != "t3" {
		t.Fatalf("ready tasks after t2 = %+v, want t3", ready)
	}

	if orch.AllCompleted() {
		t.Fatal("should not be completed yet")
	}

	orch.RecordResult(OrchestratorResult{TaskID: "t3", Success: true})
	if !orch.AllCompleted() {
		t.Fatal("should be completed")
	}

	summary := orch.Summary()
	if summary != "tasks: 3 total, 3 completed, 0 failed" {
		t.Fatalf("summary = %q", summary)
	}
}

func TestOrchestrator_FailedDependency(t *testing.T) {
	orch := NewOrchestrator()
	orch.AddTask(OrchestratorTask{ID: "t1", Description: "will fail"})
	orch.AddTask(OrchestratorTask{ID: "t2", Description: "depends on t1", DependsOn: []string{"t1"}})

	orch.RecordResult(OrchestratorResult{TaskID: "t1", Success: false, Error: "boom"})

	// t2 should not be ready because t1 failed
	ready := orch.ReadyTasks()
	if len(ready) != 0 {
		t.Fatalf("ready tasks after failed dep = %+v, want empty", ready)
	}
}

// --- Phase 17: Semantic memory tests ---

type mockEmbeddingProvider struct {
	dimension int
	embedFn   func(ctx context.Context, text string) (EmbeddingVector, error)
}

func (m *mockEmbeddingProvider) Embed(ctx context.Context, text string) (EmbeddingVector, error) {
	if m.embedFn != nil {
		return m.embedFn(ctx, text)
	}
	// Simple hash-based embedding for testing
	vec := make(EmbeddingVector, m.dimension)
	for i, r := range text {
		vec[i%m.dimension] += float32(r)
	}
	// Normalize
	var norm float32
	for _, v := range vec {
		norm += v * v
	}
	if norm > 0 {
		for i := range vec {
			vec[i] /= float32(math.Sqrt(float64(norm)))
		}
	}
	return vec, nil
}

func (m *mockEmbeddingProvider) Dimension() int { return m.dimension }

func TestSemanticMemory_StoreAndRecall(t *testing.T) {
	provider := &mockEmbeddingProvider{dimension: 8}
	mem := NewSemanticMemoryManager(SemanticMemoryConfig{
		Provider:   provider,
		MaxEntries: 10,
	})
	ctx := context.Background()

	mem.Store(ctx, "fact-go-version", "FACT: 当前项目主分支是 Go 版本。")
	mem.Store(ctx, "fact-chinese-pref", "FACT: 用户偏好用简体中文回答。")
	mem.Store(ctx, "fact-harness-engineering", "FACT: Harness engineering 是核心设计理念。")

	if mem.Len() != 3 {
		t.Fatalf("Len() = %d, want 3", mem.Len())
	}

	// Recall by semantic similarity
	results, err := mem.Recall(ctx, "Go 语言项目", 5)
	if err != nil {
		t.Fatalf("Recall failed: %v", err)
	}
	if len(results) == 0 {
		t.Fatal("Recall returned no results")
	}
	// Results should be non-empty — the exact ranking depends on the embedding provider
	t.Logf("Top recall result: %q (score-based)", results[0].Content)
}

func TestSemanticMemory_Deduplication(t *testing.T) {
	provider := &mockEmbeddingProvider{dimension: 4}
	mem := NewSemanticMemoryManager(SemanticMemoryConfig{
		Provider:            provider,
		SimilarityThreshold: 0.9,
	})
	ctx := context.Background()

	mem.Store(ctx, "fact-1", "项目使用 Go 语言开发")
	mem.Store(ctx, "fact-2", "项目使用 Go 语言编写的") // near-duplicate

	// Should have deduplicated — only 1 entry
	if mem.Len() != 1 {
		t.Fatalf("Len() = %d after dedup, want 1", mem.Len())
	}
}

func TestSemanticMemory_TTLExpiry(t *testing.T) {
	mem := NewSemanticMemoryManager(SemanticMemoryConfig{
		DefaultTTL: 100 * time.Millisecond,
	})
	ctx := context.Background()

	mem.Store(ctx, "fact-expired", "This will expire soon")

	// Should be available immediately
	results, _ := mem.Recall(ctx, "expire", 10)
	if len(results) != 1 {
		t.Fatalf("before expiry: results = %d, want 1", len(results))
	}

	time.Sleep(150 * time.Millisecond)

	// Should be expired now
	results, _ = mem.Recall(ctx, "expire", 10)
	if len(results) != 0 {
		t.Fatalf("after expiry: results = %d, want 0", len(results))
	}
}

func TestSemanticMemory_CleanupExpired(t *testing.T) {
	mem := NewSemanticMemoryManager(SemanticMemoryConfig{
		DefaultTTL: 50 * time.Millisecond,
	})
	ctx := context.Background()

	mem.Store(ctx, "exp-1", "entry 1")
	mem.Store(ctx, "exp-2", "entry 2")

	time.Sleep(100 * time.Millisecond)

	cleaned := mem.CleanupExpired()
	if cleaned != 2 {
		t.Fatalf("cleaned = %d, want 2", cleaned)
	}
	if mem.Len() != 0 {
		t.Fatalf("Len() after cleanup = %d, want 0", mem.Len())
	}
}

func TestSemanticMemory_MaxCapacity(t *testing.T) {
	mem := NewSemanticMemoryManager(SemanticMemoryConfig{
		MaxEntries: 2,
	})
	ctx := context.Background()

	mem.Store(ctx, "old", "old entry")
	mem.Store(ctx, "mid", "mid entry")
	mem.Store(ctx, "new", "new entry") // should evict oldest

	if mem.Len() != 2 {
		t.Fatalf("Len() = %d, want 2", mem.Len())
	}
}

func TestCosineSimilarity(t *testing.T) {
	// Identical vectors
	sim := cosineSimilarity(EmbeddingVector{1, 0, 0}, EmbeddingVector{1, 0, 0})
	if sim != 1.0 {
		t.Fatalf("identical similarity = %f, want 1.0", sim)
	}
	// Orthogonal
	sim = cosineSimilarity(EmbeddingVector{1, 0, 0}, EmbeddingVector{0, 1, 0})
	if sim != 0.0 {
		t.Fatalf("orthogonal similarity = %f, want 0.0", sim)
	}
	// Opposite
	sim = cosineSimilarity(EmbeddingVector{1, 0}, EmbeddingVector{-1, 0})
	if sim != -1.0 {
		t.Fatalf("opposite similarity = %f, want -1.0", sim)
	}
	// Empty
	sim = cosineSimilarity(nil, EmbeddingVector{1, 0})
	if sim != 0 {
		t.Fatalf("empty similarity = %f, want 0", sim)
	}
}
