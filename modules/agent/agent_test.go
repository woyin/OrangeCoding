package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/woyin/OrangeCoding/modules/ai"
	"github.com/woyin/OrangeCoding/modules/core"
	"github.com/woyin/OrangeCoding/modules/tools"
)

// ---------------------------------------------------------------------------
// TestAgentContext
// ---------------------------------------------------------------------------

func TestAgentContext_NewAgentContext(t *testing.T) {
	sid := core.NewSessionId()
	ctx := NewAgentContext(sid, "/tmp/work")

	if ctx.SessionID() != sid {
		t.Errorf("expected session ID %v, got %v", sid, ctx.SessionID())
	}
	if ctx.WorkDir() != "/tmp/work" {
		t.Errorf("expected work dir /tmp/work, got %s", ctx.WorkDir())
	}
	if ctx.Conversation() == nil {
		t.Error("expected non-nil conversation")
	}
}

func TestAgentContext_SetSystemPrompt(t *testing.T) {
	sid := core.NewSessionId()
	ctx := NewAgentContext(sid, "/tmp/work")
	ctx.SetSystemPrompt("you are a helper")

	conv := ctx.Conversation()
	if conv.Len() != 1 {
		t.Fatalf("expected 1 message, got %d", conv.Len())
	}
	msgs := conv.Messages()
	if msgs[0].Role != core.RoleSystem {
		t.Errorf("expected system role, got %v", msgs[0].Role)
	}
	if msgs[0].Content != "you are a helper" {
		t.Errorf("unexpected content: %s", msgs[0].Content)
	}
}

func TestAgentContext_AddUserMessage(t *testing.T) {
	sid := core.NewSessionId()
	ctx := NewAgentContext(sid, "/tmp/work")
	ctx.AddUserMessage("hello")

	conv := ctx.Conversation()
	msgs := conv.Messages()
	if len(msgs) != 1 {
		t.Fatalf("expected 1 message, got %d", len(msgs))
	}
	if msgs[0].Role != core.RoleUser {
		t.Errorf("expected user role, got %v", msgs[0].Role)
	}
	if msgs[0].Content != "hello" {
		t.Errorf("unexpected content: %s", msgs[0].Content)
	}
}

func TestAgentContext_AddAssistantMessage(t *testing.T) {
	sid := core.NewSessionId()
	ctx := NewAgentContext(sid, "/tmp/work")
	ctx.AddAssistantMessage("hi there")

	conv := ctx.Conversation()
	msgs := conv.Messages()
	if len(msgs) != 1 {
		t.Fatalf("expected 1 message, got %d", len(msgs))
	}
	if msgs[0].Role != core.RoleAssistant {
		t.Errorf("expected assistant role, got %v", msgs[0].Role)
	}
}

func TestAgentContext_AddToolResult(t *testing.T) {
	sid := core.NewSessionId()
	ctx := NewAgentContext(sid, "/tmp/work")
	result := core.NewToolResultSuccess("call-123", "output data")
	ctx.AddToolResult(result)

	conv := ctx.Conversation()
	msgs := conv.Messages()
	if len(msgs) != 1 {
		t.Fatalf("expected 1 message, got %d", len(msgs))
	}
	if msgs[0].Role != core.RoleTool {
		t.Errorf("expected tool role, got %v", msgs[0].Role)
	}
	if msgs[0].ToolCallID != "call-123" {
		t.Errorf("expected tool call ID call-123, got %s", msgs[0].ToolCallID)
	}
}

// ---------------------------------------------------------------------------
// TestToolExecutor
// ---------------------------------------------------------------------------

// mockTool implements tools.Tool for testing.
type mockTool struct {
	name    string
	result  string
	err     error
	execute func(ctx context.Context, input json.RawMessage) (string, error)
}

func (m *mockTool) Name() string                       { return m.name }
func (m *mockTool) Description() string                { return "mock tool" }
func (m *mockTool) Parameters() json.RawMessage        { return json.RawMessage(`{}`) }
func (m *mockTool) Metadata() tools.ToolMetadata       { return tools.DefaultMetadata() }
func (m *mockTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	if m.execute != nil {
		return m.execute(ctx, input)
	}
	return m.result, m.err
}

func TestToolExecutor_Execute(t *testing.T) {
	registry := tools.NewToolRegistry()
	registry.Register(&mockTool{
		name:   "echo",
		result: "hello world",
	})

	executor := NewToolExecutor(registry)
	call := core.ToolCall{
		ID:           "call-1",
		FunctionName: "echo",
		Arguments:    json.RawMessage(`{}`),
	}

	result := executor.Execute(context.Background(), call)

	if result.ToolCallID != "call-1" {
		t.Errorf("expected call-1, got %s", result.ToolCallID)
	}
	if result.IsError {
		t.Errorf("expected no error, got %s", result.Content)
	}
	if result.Content != "hello world" {
		t.Errorf("expected 'hello world', got %s", result.Content)
	}
	if result.Duration == 0 {
		t.Error("expected non-zero duration")
	}
}

func TestToolExecutor_ExecuteToolNotFound(t *testing.T) {
	registry := tools.NewToolRegistry()
	executor := NewToolExecutor(registry)

	call := core.ToolCall{
		ID:           "call-1",
		FunctionName: "nonexistent",
		Arguments:    json.RawMessage(`{}`),
	}

	result := executor.Execute(context.Background(), call)
	if !result.IsError {
		t.Error("expected error for nonexistent tool")
	}
}

func TestToolExecutor_ExecuteBatch(t *testing.T) {
	registry := tools.NewToolRegistry()
	registry.Register(&mockTool{name: "tool_a", result: "result_a"})
	registry.Register(&mockTool{name: "tool_b", result: "result_b"})

	executor := NewToolExecutor(registry)
	calls := []core.ToolCall{
		{ID: "call-1", FunctionName: "tool_a", Arguments: json.RawMessage(`{}`)},
		{ID: "call-2", FunctionName: "tool_b", Arguments: json.RawMessage(`{}`)},
	}

	results := executor.ExecuteBatch(context.Background(), calls)
	if len(results) != 2 {
		t.Fatalf("expected 2 results, got %d", len(results))
	}

	foundA, foundB := false, false
	for _, r := range results {
		if r.ToolCallID == "call-1" && r.Content == "result_a" {
			foundA = true
		}
		if r.ToolCallID == "call-2" && r.Content == "result_b" {
			foundB = true
		}
	}
	if !foundA {
		t.Error("missing result for call-1")
	}
	if !foundB {
		t.Error("missing result for call-2")
	}
}

func TestToolExecutor_ExecuteBatchEmpty(t *testing.T) {
	registry := tools.NewToolRegistry()
	executor := NewToolExecutor(registry)

	results := executor.ExecuteBatch(context.Background(), []core.ToolCall{})
	if len(results) != 0 {
		t.Errorf("expected 0 results for empty batch, got %d", len(results))
	}
}

// ---------------------------------------------------------------------------
// TestAgentLoop
// ---------------------------------------------------------------------------

// mockStreamProvider implements ai.AiProvider for testing with streaming.
type mockStreamProvider struct {
	responses [][]ai.StreamEvent
	callCount int
	mu        sync.Mutex
}

func (m *mockStreamProvider) Name() string { return "mock-stream" }

func (m *mockStreamProvider) ChatCompletion(ctx context.Context, messages []ai.ChatMessage, tools []ai.ToolDefinition, opts ai.ChatOptions) (*ai.AiResponse, error) {
	return nil, fmt.Errorf("not implemented")
}

func (m *mockStreamProvider) ChatCompletionStream(ctx context.Context, messages []ai.ChatMessage, tools []ai.ToolDefinition, opts ai.ChatOptions) (<-chan ai.StreamEvent, error) {
	m.mu.Lock()
	idx := m.callCount
	m.callCount++
	m.mu.Unlock()

	if idx >= len(m.responses) {
		ch := make(chan ai.StreamEvent)
		close(ch)
		return ch, nil
	}

	ch := make(chan ai.StreamEvent, len(m.responses[idx])+1)
	for _, ev := range m.responses[idx] {
		ch <- ev
	}
	ch <- ai.StreamEvent{Type: "done"}
	close(ch)
	return ch, nil
}

func TestAgentLoop_BasicCompletion(t *testing.T) {
	// Provider returns: first a response with a tool call, then a final text response.
	provider := &mockStreamProvider{
		responses: [][]ai.StreamEvent{
			// First call: text + tool call
			{
				{Type: "content_delta", Content: "Let me check "},
				{Type: "tool_call_delta", ToolCallID: "tc-1", ToolCallName: "echo", Arguments: `{}`},
				{Type: "usage", Usage: &ai.TokenUsage{PromptTokens: 100, CompletionTokens: 50, TotalTokens: 150}},
			},
			// Second call: final text
			{
				{Type: "content_delta", Content: "Done!"},
				{Type: "usage", Usage: &ai.TokenUsage{PromptTokens: 200, CompletionTokens: 30, TotalTokens: 230}},
			},
		},
	}

	registry := tools.NewToolRegistry()
	registry.Register(&mockTool{name: "echo", result: "echoed"})
	executor := NewToolExecutor(registry)

	sid := core.NewSessionId()
	agentCtx := NewAgentContext(sid, "/tmp/work")
	agentCtx.AddUserMessage("test message")

	agentID := core.NewAgentId()
	toolDefs := []ai.ToolDefinition{
		{
			Type: "function",
			Function: ai.FunctionDefinition{
				Name:        "echo",
				Description: "echo tool",
				Parameters:  ai.ToolParameter{Type: "object"},
			},
		},
	}

	loop := NewAgentLoop(agentID, provider, executor, agentCtx, DefaultLoopConfig(), toolDefs)

	eventCh := make(chan core.AgentEvent, 100)
	result, err := loop.Run(context.Background(), ai.ChatOptions{}, eventCh)
	close(eventCh)

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.ToolCallsMade != 1 {
		t.Errorf("expected 1 tool call, got %d", result.ToolCallsMade)
	}
	if result.TokensUsed.TotalTokens == 0 {
		t.Error("expected non-zero token usage")
	}
	if result.Duration == 0 {
		t.Error("expected non-zero duration")
	}

	// Verify the conversation has the right structure
	conv := agentCtx.Conversation()
	msgs := conv.Messages()
	// user msg, assistant (tool call), tool result, assistant (final)
	if len(msgs) < 3 {
		t.Fatalf("expected at least 3 messages, got %d", len(msgs))
	}
}

func TestAgentLoop_MaxIterations(t *testing.T) {
	// Provider always returns tool calls, causing the loop to hit MaxIterations.
	// Provide enough responses for all iterations (MaxIterations=2).
	toolCallResponse := []ai.StreamEvent{
		{Type: "tool_call_delta", ToolCallID: "tc-1", ToolCallName: "echo", Arguments: `{}`},
		{Type: "usage", Usage: &ai.TokenUsage{PromptTokens: 10, CompletionTokens: 10, TotalTokens: 20}},
	}
	provider := &mockStreamProvider{
		responses: [][]ai.StreamEvent{
			toolCallResponse,
			toolCallResponse,
			toolCallResponse,
		},
	}

	registry := tools.NewToolRegistry()
	registry.Register(&mockTool{name: "echo", result: "ok"})
	executor := NewToolExecutor(registry)

	sid := core.NewSessionId()
	agentCtx := NewAgentContext(sid, "/tmp/work")
	agentCtx.AddUserMessage("test")

	agentID := core.NewAgentId()
	config := AgentLoopConfig{MaxIterations: 2, Timeout: 10 * time.Second}

	toolDefs := []ai.ToolDefinition{
		{
			Type: "function",
			Function: ai.FunctionDefinition{
				Name:        "echo",
				Description: "echo tool",
				Parameters:  ai.ToolParameter{Type: "object"},
			},
		},
	}

	loop := NewAgentLoop(agentID, provider, executor, agentCtx, config, toolDefs)
	eventCh := make(chan core.AgentEvent, 100)
	_, err := loop.Run(context.Background(), ai.ChatOptions{}, eventCh)
	close(eventCh)

	if err == nil {
		t.Error("expected error when max iterations exceeded")
	}
	if !strings.Contains(err.Error(), "max iterations") {
		t.Errorf("expected max iterations error, got: %v", err)
	}
}

func TestAgentLoop_ContextCancellation(t *testing.T) {
	provider := &mockStreamProvider{
		responses: [][]ai.StreamEvent{
			{
				{Type: "content_delta", Content: "hello"},
				{Type: "usage", Usage: &ai.TokenUsage{PromptTokens: 10, CompletionTokens: 10, TotalTokens: 20}},
			},
		},
	}

	registry := tools.NewToolRegistry()
	executor := NewToolExecutor(registry)

	sid := core.NewSessionId()
	agentCtx := NewAgentContext(sid, "/tmp/work")
	agentCtx.AddUserMessage("test")

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately

	agentID := core.NewAgentId()
	loop := NewAgentLoop(agentID, provider, executor, agentCtx, DefaultLoopConfig(), nil)
	eventCh := make(chan core.AgentEvent, 100)
	_, err := loop.Run(ctx, ai.ChatOptions{}, eventCh)
	close(eventCh)

	if err == nil {
		t.Error("expected error from cancelled context")
	}
}

// ---------------------------------------------------------------------------
// TestIntentGate
// ---------------------------------------------------------------------------

func TestIntentGate_Classify(t *testing.T) {
	gate := NewIntentGate()

	tests := []struct {
		input    string
		expected IntentCategory
	}{
		{"write a function that sorts an array", IntentCoding},
		{"create a new file for the module", IntentCoding},
		{"refactor the code in main.go", IntentCoding},
		{"plan the new feature", IntentPlanning},
		{"design the architecture for auth", IntentPlanning},
		{"review the changes in PR #42", IntentReview},
		{"audit the security settings", IntentReview},
		{"what is the purpose of this?", IntentQuestion},
		{"how does the router work?", IntentQuestion},
		{"why did the test fail?", IntentQuestion},
		{"search for TODO comments", IntentExplore},
		{"explore the project structure", IntentExplore},
		{"find all routes in the app", IntentExplore},
		{"hello there", IntentGeneral},
		{"thanks!", IntentGeneral},
	}

	for _, tt := range tests {
		result := gate.Classify(tt.input)
		if result != tt.expected {
			t.Errorf("Classify(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

// ---------------------------------------------------------------------------
// TestMemoryStore
// ---------------------------------------------------------------------------

func TestMemoryStore_WriteRead(t *testing.T) {
	dir := t.TempDir()
	store := NewMemoryStore(dir)

	err := store.Write("test-key", "test-value")
	if err != nil {
		t.Fatalf("Write failed: %v", err)
	}

	val, err := store.Read("test-key")
	if err != nil {
		t.Fatalf("Read failed: %v", err)
	}
	if val != "test-value" {
		t.Errorf("expected 'test-value', got %q", val)
	}
}

func TestMemoryStore_ReadNotFound(t *testing.T) {
	dir := t.TempDir()
	store := NewMemoryStore(dir)

	_, err := store.Read("nonexistent")
	if err == nil {
		t.Error("expected error for nonexistent key")
	}
}

func TestMemoryStore_List(t *testing.T) {
	dir := t.TempDir()
	store := NewMemoryStore(dir)

	store.Write("key1", "val1")
	store.Write("key2", "val2")
	store.Write("key3", "val3")

	keys, err := store.List()
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}
	if len(keys) != 3 {
		t.Errorf("expected 3 keys, got %d", len(keys))
	}
}

func TestMemoryStore_Recall(t *testing.T) {
	dir := t.TempDir()
	store := NewMemoryStore(dir)

	store.Write("go-tips", "use goroutines")
	store.Write("python-tips", "use list comprehensions")
	store.Write("rust-tips", "use ownership")

	keys, err := store.Recall("go")
	if err != nil {
		t.Fatalf("Recall failed: %v", err)
	}
	if len(keys) != 1 {
		t.Errorf("expected 1 key matching 'go', got %d: %v", len(keys), keys)
	}
	if keys[0] != "go-tips" {
		t.Errorf("expected 'go-tips', got %q", keys[0])
	}
}

// ---------------------------------------------------------------------------
// TestHookManager
// ---------------------------------------------------------------------------

func TestHookManager_RegisterAndRun(t *testing.T) {
	hm := NewHookManager()
	tmpFile := filepath.Join(t.TempDir(), "hook_output.txt")

	hm.Register(Hook{
		Point:   HookPreToolCall,
		Command: fmt.Sprintf("touch %s", tmpFile),
	})

	err := hm.Run(context.Background(), HookPreToolCall, "test-data")
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}

	if _, err := os.Stat(tmpFile); os.IsNotExist(err) {
		t.Error("hook command did not execute (file not created)")
	}
}

func TestHookManager_NoHooksForPoint(t *testing.T) {
	hm := NewHookManager()
	err := hm.Run(context.Background(), HookPostToolCall, "data")
	if err != nil {
		t.Errorf("expected no error when no hooks registered, got: %v", err)
	}
}

// ---------------------------------------------------------------------------
// TestSkillRegistry
// ---------------------------------------------------------------------------

func TestSkillRegistry_RegisterAndGet(t *testing.T) {
	reg := NewSkillRegistry()

	skill := Skill{
		Name:        "custom",
		Description: "A custom skill",
		Tools:       []string{"bash", "read_file"},
		Prompt:      "You are a custom agent",
	}
	reg.Register(skill)

	got, ok := reg.Get("custom")
	if !ok {
		t.Fatal("expected to find 'custom' skill")
	}
	if got.Description != "A custom skill" {
		t.Errorf("unexpected description: %s", got.Description)
	}
}

func TestSkillRegistry_GetNotFound(t *testing.T) {
	reg := NewSkillRegistry()
	_, ok := reg.Get("nonexistent")
	if ok {
		t.Error("expected not found for nonexistent skill")
	}
}

func TestSkillRegistry_List(t *testing.T) {
	reg := NewSkillRegistry()
	list := reg.List()
	if len(list) == 0 {
		t.Error("expected built-in skills to be registered")
	}

	// Check for built-in skills
	names := make(map[string]bool)
	for _, s := range list {
		names[s.Name] = true
	}

	expectedSkills := []string{"code", "debug", "review", "plan", "explore", "refactor"}
	for _, name := range expectedSkills {
		if !names[name] {
			t.Errorf("missing built-in skill: %s", name)
		}
	}
}

func TestSkillRegistry_BuiltInSkillTools(t *testing.T) {
	reg := NewSkillRegistry()

	code, ok := reg.Get("code")
	if !ok {
		t.Fatal("missing 'code' skill")
	}
	expectedTools := []string{"bash", "read_file", "write_file", "edit_file"}
	for _, tool := range expectedTools {
		found := false
		for _, t := range code.Tools {
			if t == tool {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("code skill missing tool: %s", tool)
		}
	}
}

// ---------------------------------------------------------------------------
// TestCompactor
// ---------------------------------------------------------------------------

func TestCompactor_CompactRemovesOldMessages(t *testing.T) {
	compactor := NewCompactor(50) // very low token limit

	conv := core.NewConversation()
	conv.AddMessage(core.NewSystemMessage("system prompt"))
	// Add many messages to exceed limit
	for i := 0; i < 20; i++ {
		conv.AddMessage(core.NewUserMessage(fmt.Sprintf("user message number %d with some padding content to use tokens", i)))
		conv.AddMessage(core.NewAssistantMessage(fmt.Sprintf("assistant response number %d with some padding content to use tokens", i)))
	}

	err := compactor.Compact(conv)
	if err != nil {
		t.Fatalf("Compact failed: %v", err)
	}

	msgs := conv.Messages()
	// System prompt should be preserved
	if msgs[0].Role != core.RoleSystem {
		t.Error("system prompt was removed")
	}
	// Should have fewer messages than original
	if len(msgs) >= 41 { // 1 system + 20*2
		t.Errorf("expected compaction to reduce messages, still have %d", len(msgs))
	}
	// Last 5 messages should be preserved
	last5Count := 0
	total := len(msgs)
	for i := total - 5; i < total; i++ {
		if i >= 0 {
			last5Count++
		}
	}
	if total >= 5 && last5Count < 5 {
		t.Error("expected last 5 messages to be preserved")
	}
}

func TestCompactor_CompactUnderLimit(t *testing.T) {
	compactor := NewCompactor(100000) // very high limit

	conv := core.NewConversation()
	conv.AddMessage(core.NewSystemMessage("system"))
	conv.AddMessage(core.NewUserMessage("hi"))

	originalLen := conv.Len()
	err := compactor.Compact(conv)
	if err != nil {
		t.Fatalf("Compact failed: %v", err)
	}
	if conv.Len() != originalLen {
		t.Errorf("expected no compaction, length changed from %d to %d", originalLen, conv.Len())
	}
}

// ---------------------------------------------------------------------------
// TestTTSR
// ---------------------------------------------------------------------------

func TestTTSR_Check(t *testing.T) {
	ttsr := NewTTSR([]Rule{
		{Pattern: regexp.MustCompile(`(?i)TODO`), Rule: "Always address TODOs"},
		{Pattern: regexp.MustCompile(`(?i)FIXME`), Rule: "Fix all FIXMEs immediately"},
		{Pattern: regexp.MustCompile(`(?i)HACK`), Rule: "Remove all hacks"},
	})

	matches := ttsr.Check("This has a TODO item and a FIXME")
	if len(matches) != 2 {
		t.Fatalf("expected 2 matching rules, got %d", len(matches))
	}

	foundTODO, foundFIXME := false, false
	for _, r := range matches {
		if strings.Contains(r, "TODO") {
			foundTODO = true
		}
		if strings.Contains(r, "FIXME") {
			foundFIXME = true
		}
	}
	if !foundTODO {
		t.Error("missing TODO rule match")
	}
	if !foundFIXME {
		t.Error("missing FIXME rule match")
	}
}

func TestTTSR_CheckNoMatch(t *testing.T) {
	ttsr := NewTTSR([]Rule{
		{Pattern: regexp.MustCompile(`(?i)URGENT`), Rule: "Handle urgently"},
	})

	matches := ttsr.Check("just a normal message")
	if len(matches) != 0 {
		t.Errorf("expected 0 matches, got %d", len(matches))
	}
}

// ---------------------------------------------------------------------------
// TestConversationToAIMessages
// ---------------------------------------------------------------------------

func TestConversationToAIMessages(t *testing.T) {
	conv := core.NewConversation()
	conv.AddMessage(core.NewSystemMessage("system prompt"))
	conv.AddMessage(core.NewUserMessage("hello"))
	conv.AddMessage(core.NewAssistantMessage("hi there"))

	aiMsgs := conversationToAIMessages(conv)
	if len(aiMsgs) != 3 {
		t.Fatalf("expected 3 messages, got %d", len(aiMsgs))
	}
	if aiMsgs[0].Role != "system" {
		t.Errorf("expected system role, got %s", aiMsgs[0].Role)
	}
	if aiMsgs[1].Role != "user" {
		t.Errorf("expected user role, got %s", aiMsgs[1].Role)
	}
	if aiMsgs[1].Content != "hello" {
		t.Errorf("expected 'hello', got %s", aiMsgs[1].Content)
	}
	if aiMsgs[2].Role != "assistant" {
		t.Errorf("expected assistant role, got %s", aiMsgs[2].Role)
	}
}

func TestConversationToAIMessages_WithToolCalls(t *testing.T) {
	conv := core.NewConversation()
	conv.AddMessage(core.NewAssistantMessageWithToolCalls("let me check", []core.ToolCall{
		{ID: "tc-1", FunctionName: "echo", Arguments: json.RawMessage(`{}`)},
	}))

	aiMsgs := conversationToAIMessages(conv)
	if len(aiMsgs) != 1 {
		t.Fatalf("expected 1 message, got %d", len(aiMsgs))
	}
	// The assistant message with tool calls should have ai.ToolCalls populated
	if len(aiMsgs[0].ToolCalls) != 1 {
		t.Errorf("expected 1 tool call, got %d", len(aiMsgs[0].ToolCalls))
	}
}

func TestConversationToAIMessages_ToolResult(t *testing.T) {
	conv := core.NewConversation()
	conv.AddMessage(core.NewToolResultMessage("tc-1", "result data", false))

	aiMsgs := conversationToAIMessages(conv)
	if len(aiMsgs) != 1 {
		t.Fatalf("expected 1 message, got %d", len(aiMsgs))
	}
	if aiMsgs[0].Role != "tool" {
		t.Errorf("expected tool role, got %s", aiMsgs[0].Role)
	}
	if aiMsgs[0].ToolCallID != "tc-1" {
		t.Errorf("expected tool call ID tc-1, got %s", aiMsgs[0].ToolCallID)
	}
}

// ---------------------------------------------------------------------------
// TestDefaultLoopConfig
// ---------------------------------------------------------------------------

func TestDefaultLoopConfig(t *testing.T) {
	config := DefaultLoopConfig()
	if config.MaxIterations != 20 {
		t.Errorf("expected MaxIterations=20, got %d", config.MaxIterations)
	}
	if config.Timeout != 300*time.Second {
		t.Errorf("expected Timeout=300s, got %v", config.Timeout)
	}
}
