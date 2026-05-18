package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// ---------------------------------------------------------------------------
// TestToolRegistry
// ---------------------------------------------------------------------------

func TestToolRegistry(t *testing.T) {
	r := NewToolRegistry()

	mock := &mockTool{name: "mock", desc: "A mock tool"}

	// Register
	r.Register(mock)

	// Get
	got, ok := r.Get("mock")
	if !ok {
		t.Fatal("expected to find registered tool")
	}
	if got.Name() != "mock" {
		t.Fatalf("expected name 'mock', got %q", got.Name())
	}

	// Get missing
	_, ok = r.Get("nonexistent")
	if ok {
		t.Fatal("expected not to find unregistered tool")
	}

	// List
	list := r.List()
	if len(list) != 1 {
		t.Fatalf("expected 1 tool in list, got %d", len(list))
	}

	// Register second tool
	mock2 := &mockTool{name: "mock2", desc: "Another mock"}
	r.Register(mock2)
	list = r.List()
	if len(list) != 2 {
		t.Fatalf("expected 2 tools in list, got %d", len(list))
	}
}

// ---------------------------------------------------------------------------
// TestToolError
// ---------------------------------------------------------------------------

func TestToolError(t *testing.T) {
	te := &ToolError{Kind: "invalid_params", Message: "missing field"}
	expected := "invalid_params: missing field"
	if te.Error() != expected {
		t.Fatalf("expected %q, got %q", expected, te.Error())
	}
}

// ---------------------------------------------------------------------------
// TestToolMetadataConstructors
// ---------------------------------------------------------------------------

func TestToolMetadataConstructors(t *testing.T) {
	d := DefaultMetadata()
	if !d.IsEnabled || d.IsReadOnly || d.IsConcurrencySafe || d.IsDestructive {
		t.Fatal("DefaultMetadata: only IsEnabled should be true")
	}

	ro := ReadOnlyMetadata()
	if !ro.IsEnabled || !ro.IsReadOnly || !ro.IsConcurrencySafe || ro.IsDestructive {
		t.Fatal("ReadOnlyMetadata: IsEnabled, IsReadOnly, IsConcurrencySafe should be true")
	}

	ds := DestructiveMetadata()
	if !ds.IsEnabled || ds.IsReadOnly || ds.IsConcurrencySafe || !ds.IsDestructive {
		t.Fatal("DestructiveMetadata: only IsEnabled and IsDestructive should be true")
	}
}

// ---------------------------------------------------------------------------
// TestBatchExecution
// ---------------------------------------------------------------------------

func TestBatchExecution(t *testing.T) {
	r := NewToolRegistry()

	echo := &mockTool{
		name: "echo",
		desc: "echoes input",
		meta: ReadOnlyMetadata(),
		execute: func(ctx context.Context, input json.RawMessage) (string, error) {
			var args struct {
				Msg string `json:"msg"`
			}
			if err := json.Unmarshal(input, &args); err != nil {
				return "", err
			}
			return args.Msg, nil
		},
	}
	r.Register(echo)

	calls := []core.ToolCall{
		{ID: "1", FunctionName: "echo", Arguments: json.RawMessage(`{"msg":"hello"}`)},
		{ID: "2", FunctionName: "echo", Arguments: json.RawMessage(`{"msg":"world"}`)},
		{ID: "3", FunctionName: "nonexistent", Arguments: json.RawMessage(`{}`)},
	}

	results := ExecuteBatch(context.Background(), r, calls)

	if len(results) != 3 {
		t.Fatalf("expected 3 results, got %d", len(results))
	}

	// Results may come back in any order; sort by ToolCallID
	sort.Slice(results, func(i, j int) bool {
		return results[i].ToolCallID < results[j].ToolCallID
	})

	if results[0].ToolCallID != "1" || results[0].Content != "hello" || results[0].IsError {
		t.Fatalf("unexpected result[0]: %+v", results[0])
	}
	if results[1].ToolCallID != "2" || results[1].Content != "world" || results[1].IsError {
		t.Fatalf("unexpected result[1]: %+v", results[1])
	}
	if results[2].ToolCallID != "3" || !results[2].IsError {
		t.Fatalf("expected error for nonexistent tool, got: %+v", results[2])
	}
}

// ---------------------------------------------------------------------------
// TestBashTool
// ---------------------------------------------------------------------------

func TestBashTool(t *testing.T) {
	tool := NewBashTool(nil)
	if tool.Name() != "bash" {
		t.Fatalf("expected name 'bash', got %q", tool.Name())
	}

	out, err := tool.Execute(context.Background(), json.RawMessage(`{"command":"echo hello"}`))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "hello") {
		t.Fatalf("expected output containing 'hello', got %q", out)
	}
}

func TestBashTool_SecurityPolicy(t *testing.T) {
	policy := NewSecurityPolicy([]string{"rm"})
	tool := NewBashTool(policy)

	_, err := tool.Execute(context.Background(), json.RawMessage(`{"command":"rm -rf /"}`))
	if err == nil {
		t.Fatal("expected security violation for blocked command")
	}
}

// ---------------------------------------------------------------------------
// TestReadFileTool
// ---------------------------------------------------------------------------

func TestReadFileTool(t *testing.T) {
	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.txt")
	content := "line1\nline2\nline3\nline4\nline5\n"
	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	tool := NewReadFileTool()

	// Full read
	out, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"path":%q}`, testFile,
	)))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "line1") || !strings.Contains(out, "line5") {
		t.Fatalf("expected full file content, got %q", out)
	}

	// Partial read with offset and limit
	out, err = tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"path":%q,"offset":2,"limit":2}`, testFile,
	)))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "line2") || !strings.Contains(out, "line3") {
		t.Fatalf("expected partial content, got %q", out)
	}
	if strings.Contains(out, "line1") {
		t.Fatalf("should not contain line1, got %q", out)
	}
}

// ---------------------------------------------------------------------------
// TestWriteFileTool
// ---------------------------------------------------------------------------

func TestWriteFileTool(t *testing.T) {
	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "subdir", "output.txt")

	tool := NewWriteFileTool()
	out, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"path":%q,"content":"hello world"}`, testFile,
	)))
	if err != nil {
		t.Fatal(err)
	}
	if out == "" {
		t.Fatal("expected some output")
	}

	data, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "hello world" {
		t.Fatalf("expected 'hello world', got %q", string(data))
	}
}

// ---------------------------------------------------------------------------
// TestEditFileTool
// ---------------------------------------------------------------------------

func TestEditFileTool(t *testing.T) {
	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "edit.txt")
	if err := os.WriteFile(testFile, []byte("foo bar baz"), 0644); err != nil {
		t.Fatal(err)
	}

	tool := NewEditFileTool()
	_, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"path":%q,"old_string":"bar","new_string":"BAR"}`, testFile,
	)))
	if err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "foo BAR baz" {
		t.Fatalf("expected 'foo BAR baz', got %q", string(data))
	}

	// Test: old_string not found
	_, err = tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"path":%q,"old_string":"nonexistent","new_string":"x"}`, testFile,
	)))
	if err == nil {
		t.Fatal("expected error when old_string not found")
	}
}

// ---------------------------------------------------------------------------
// TestDeleteFileTool
// ---------------------------------------------------------------------------

func TestDeleteFileTool(t *testing.T) {
	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "delete_me.txt")
	if err := os.WriteFile(testFile, []byte("bye"), 0644); err != nil {
		t.Fatal(err)
	}

	tool := NewDeleteFileTool()
	_, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"path":%q}`, testFile,
	)))
	if err != nil {
		t.Fatal(err)
	}

	if _, err := os.Stat(testFile); !os.IsNotExist(err) {
		t.Fatal("expected file to be deleted")
	}
}

// ---------------------------------------------------------------------------
// TestListDirectoryTool
// ---------------------------------------------------------------------------

func TestListDirectoryTool(t *testing.T) {
	tmpDir := t.TempDir()
	os.Mkdir(filepath.Join(tmpDir, "subdir"), 0755)
	os.WriteFile(filepath.Join(tmpDir, "file1.txt"), []byte("a"), 0644)

	tool := NewListDirectoryTool()
	out, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"path":%q}`, tmpDir,
	)))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "subdir") || !strings.Contains(out, "file1.txt") {
		t.Fatalf("expected directory listing with subdir and file1.txt, got %q", out)
	}
}

// ---------------------------------------------------------------------------
// TestGrepTool
// ---------------------------------------------------------------------------

func TestGrepTool(t *testing.T) {
	tmpDir := t.TempDir()
	os.WriteFile(filepath.Join(tmpDir, "a.txt"), []byte("hello world\nfoo bar\n"), 0644)
	os.WriteFile(filepath.Join(tmpDir, "b.txt"), []byte("hello Go\nbaz\n"), 0644)

	tool := NewGrepTool()
	out, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"pattern":"hello","path":%q}`, tmpDir,
	)))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "hello") {
		t.Fatalf("expected grep results containing 'hello', got %q", out)
	}
}

// ---------------------------------------------------------------------------
// TestFindTool
// ---------------------------------------------------------------------------

func TestFindTool(t *testing.T) {
	tmpDir := t.TempDir()
	os.MkdirAll(filepath.Join(tmpDir, "src", "pkg"), 0755)
	os.WriteFile(filepath.Join(tmpDir, "src", "main.go"), []byte("package main"), 0644)
	os.WriteFile(filepath.Join(tmpDir, "src", "pkg", "util.go"), []byte("package pkg"), 0644)

	tool := NewFindTool()

	// Find by name pattern
	out, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"path":%q,"name":"*.go"}`, tmpDir,
	)))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "main.go") || !strings.Contains(out, "util.go") {
		t.Fatalf("expected find results with .go files, got %q", out)
	}
}

// ---------------------------------------------------------------------------
// TestGlobTool
// ---------------------------------------------------------------------------

func TestGlobTool(t *testing.T) {
	tmpDir := t.TempDir()
	os.WriteFile(filepath.Join(tmpDir, "a.go"), []byte(""), 0644)
	os.WriteFile(filepath.Join(tmpDir, "b.txt"), []byte(""), 0644)
	os.WriteFile(filepath.Join(tmpDir, "c.go"), []byte(""), 0644)

	tool := NewGlobTool()
	pattern := filepath.Join(tmpDir, "*.go")
	out, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"pattern":%q}`, pattern,
	)))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "a.go") || !strings.Contains(out, "c.go") {
		t.Fatalf("expected glob results with .go files, got %q", out)
	}
	if strings.Contains(out, "b.txt") {
		t.Fatalf("should not contain b.txt, got %q", out)
	}
}

// ---------------------------------------------------------------------------
// TestFetchTool
// ---------------------------------------------------------------------------

func TestFetchTool(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
		w.Write([]byte("response body"))
	}))
	defer ts.Close()

	tool := NewFetchTool()
	out, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
		`{"url":%q}`, ts.URL,
	)))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "response body") {
		t.Fatalf("expected fetch result containing 'response body', got %q", out)
	}
}

// ---------------------------------------------------------------------------
// TestCalcTool
// ---------------------------------------------------------------------------

func TestCalcTool(t *testing.T) {
	tool := NewCalcTool()

	tests := []struct {
		expr     string
		expected string
	}{
		{"2 + 3", "5"},
		{"10 - 4", "6"},
		{"3 * 7", "21"},
		{"20 / 4", "5"},
		{"(2 + 3) * 4", "20"},
	}

	for _, tt := range tests {
		out, err := tool.Execute(context.Background(), json.RawMessage(fmt.Sprintf(
			`{"expression":%q}`, tt.expr,
		)))
		if err != nil {
			t.Fatalf("expression %q: %v", tt.expr, err)
		}
		if !strings.Contains(out, tt.expected) {
			t.Fatalf("expression %q: expected result containing %q, got %q", tt.expr, tt.expected, out)
		}
	}
}

// ---------------------------------------------------------------------------
// TestPathValidator
// ---------------------------------------------------------------------------

func TestPathValidator(t *testing.T) {
	tmpDir := t.TempDir()
	v := NewPathValidator([]string{tmpDir})

	// Valid path
	if err := v.Validate(filepath.Join(tmpDir, "subdir", "file.txt")); err != nil {
		t.Fatalf("expected valid path, got error: %v", err)
	}

	// Traversal attack
	if err := v.Validate(filepath.Join(tmpDir, "..", "etc", "passwd")); err == nil {
		t.Fatal("expected error for path traversal")
	}

	// Path outside allowed dirs
	if err := v.Validate("/etc/passwd"); err == nil {
		t.Fatal("expected error for path outside allowed dirs")
	}
}

// ---------------------------------------------------------------------------
// TestSecurityPolicy
// ---------------------------------------------------------------------------

func TestSecurityPolicy(t *testing.T) {
	policy := NewSecurityPolicy([]string{"rm", "format", "mkfs"})

	if !policy.IsAllowed("ls") {
		t.Fatal("ls should be allowed")
	}
	if !policy.IsAllowed("echo hello") {
		t.Fatal("echo hello should be allowed")
	}
	if policy.IsAllowed("rm") {
		t.Fatal("rm should be blocked")
	}
	if policy.IsAllowed("format C:") {
		t.Fatal("format should be blocked")
	}
}

// ---------------------------------------------------------------------------
// TestCreateDefaultRegistry
// ---------------------------------------------------------------------------

func TestCreateDefaultRegistry(t *testing.T) {
	r := CreateDefaultRegistry()

	expectedTools := []string{
		"bash", "read_file", "write_file", "edit_file", "delete_file",
		"list_directory", "grep", "find", "glob", "fetch",
		"python", "calc", "task",
		"browser", "ssh", "lsp", "web_search", "notebook",
	}

	for _, name := range expectedTools {
		tool, ok := r.Get(name)
		if !ok {
			t.Errorf("expected tool %q in default registry", name)
			continue
		}
		if tool.Name() != name {
			t.Errorf("expected tool name %q, got %q", name, tool.Name())
		}
	}
}

// ---------------------------------------------------------------------------
// TestPermissionDecision
// ---------------------------------------------------------------------------

func TestPermissionDecision(t *testing.T) {
	if DecisionAllow >= DecisionDeny {
		t.Fatal("DecisionAllow should be less than DecisionDeny in iota ordering")
	}
}

// ---------------------------------------------------------------------------
// TestPythonTool
// ---------------------------------------------------------------------------

func TestPythonTool(t *testing.T) {
	// Skip if python3 is not available
	if _, err := execLookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}

	tool := NewPythonTool()
	out, err := tool.Execute(context.Background(), json.RawMessage(`{"code":"print(2+2)"}`))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "4") {
		t.Fatalf("expected output containing '4', got %q", out)
	}
}

// ---------------------------------------------------------------------------
// TestTaskTool
// ---------------------------------------------------------------------------

func TestTaskTool(t *testing.T) {
	tool := NewTaskTool()

	// Create a task
	out, err := tool.Execute(context.Background(), json.RawMessage(
		`{"action":"create","description":"test task"}`,
	))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "created") && !strings.Contains(out, "test task") {
		t.Fatalf("expected create confirmation, got %q", out)
	}

	// List tasks
	out, err = tool.Execute(context.Background(), json.RawMessage(
		`{"action":"list"}`,
	))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out, "test task") {
		t.Fatalf("expected task list containing 'test task', got %q", out)
	}
}

func TestTaskToolDelegateActionBuildsSubAgentBrief(t *testing.T) {
	tool := NewTaskTool()

	out, err := tool.Execute(context.Background(), json.RawMessage(`{
		"action":"delegate",
		"description":"review auth module",
		"subagent_type":"reviewer",
		"scope":"modules/auth",
		"expected_output":"risks and patch suggestions"
	}`))
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"Sub-agent delegation", "reviewer", "modules/auth", "risks and patch suggestions"} {
		if !strings.Contains(out, want) {
			t.Fatalf("delegate output = %q, want to contain %q", out, want)
		}
	}
}

// ---------------------------------------------------------------------------
// TestStubTools
// ---------------------------------------------------------------------------

func TestStubTools(t *testing.T) {
	stubs := []string{"browser", "ssh", "lsp", "web_search", "notebook"}
	for _, name := range stubs {
		t.Run(name, func(t *testing.T) {
			r := CreateDefaultRegistry()
			tool, ok := r.Get(name)
			if !ok {
				t.Fatalf("tool %q not found", name)
			}
			_, err := tool.Execute(context.Background(), json.RawMessage(`{}`))
			if err == nil {
				t.Fatalf("expected 'not implemented' error for stub tool %q", name)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// mockTool for testing
// ---------------------------------------------------------------------------

type mockTool struct {
	name    string
	desc    string
	meta    ToolMetadata
	execute func(ctx context.Context, input json.RawMessage) (string, error)
}

func (m *mockTool) Name() string                { return m.name }
func (m *mockTool) Description() string         { return m.desc }
func (m *mockTool) Parameters() json.RawMessage { return json.RawMessage(`{}`) }
func (m *mockTool) Metadata() ToolMetadata      { return m.meta }
func (m *mockTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	if m.execute != nil {
		return m.execute(ctx, input)
	}
	return "", nil
}

// execLookPath is a wrapper to allow testing without os/exec import in test.
// (We use the real exec.LookPath in production code.)
var execLookPath = func(name string) (string, error) {
	return filepath.Abs(name)
}

// Suppress unused import warning for time
var _ = time.Duration(0)
