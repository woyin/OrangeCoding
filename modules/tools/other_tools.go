package tools

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"
	"unicode/utf8"
)

// ---------------------------------------------------------------------------
// FetchTool
// ---------------------------------------------------------------------------

// FetchTool makes HTTP requests and returns the response body.
type FetchTool struct {
	params json.RawMessage
	client *http.Client
}

// NewFetchTool creates a new FetchTool.
func NewFetchTool() *FetchTool {
	return &FetchTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"url": {"type": "string"},
				"method": {"type": "string"}
			},
			"required": ["url"]
		}`),
		client: &http.Client{Timeout: 30 * time.Second},
	}
}

func (t *FetchTool) Name() string                { return "fetch" }
func (t *FetchTool) Description() string         { return "Fetch content from a URL." }
func (t *FetchTool) Parameters() json.RawMessage { return t.params }
func (t *FetchTool) Metadata() ToolMetadata      { return DefaultMetadata() }

const maxFetchSize = 100 * 1024 // 100KB

// blockedHostPrefixes are host prefixes that FetchTool will refuse to access.
var blockedHostPrefixes = []string{
	"169.254.", // cloud metadata
	"10.",
	"172.16.", "172.17.", "172.18.", "172.19.",
	"172.20.", "172.21.", "172.22.", "172.23.",
	"172.24.", "172.25.", "172.26.", "172.27.",
	"172.28.", "172.29.", "172.30.", "172.31.",
	"192.168.",
	"0.0.0.0",
	"localhost",
	"127.",
}

func (t *FetchTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		URL    string `json:"url"`
		Method string `json:"method"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}
	if args.URL == "" {
		return "", &ToolError{Kind: "invalid_params", Message: "url is required"}
	}

	// Block requests to internal/private networks.
	if !strings.HasPrefix(strings.ToLower(args.URL), "http://") && !strings.HasPrefix(strings.ToLower(args.URL), "https://") {
		return "", &ToolError{Kind: "security_violation", Message: "only http/https URLs are allowed"}
	}
	// Extract host from URL and check against blocked prefixes.
	host := extractHost(args.URL)
	for _, prefix := range blockedHostPrefixes {
		if strings.HasPrefix(host, prefix) {
			return "", &ToolError{Kind: "security_violation", Message: "access to internal/private network addresses is blocked"}
		}
	}

	if args.Method == "" {
		args.Method = "GET"
	}

	req, err := http.NewRequestWithContext(ctx, args.Method, args.URL, nil)
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	resp, err := t.client.Do(req)
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(io.LimitReader(resp.Body, maxFetchSize+1))
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	result := string(body)
	if len(body) > maxFetchSize {
		// Find a safe UTF-8 truncation point.
		truncateAt := maxFetchSize
		for truncateAt > 0 && !utf8.RuneStart(body[truncateAt]) {
			truncateAt--
		}
		result = string(body[:truncateAt]) + "\n... (truncated)"
	}

	return result, nil
}

// ---------------------------------------------------------------------------
// PythonTool
// ---------------------------------------------------------------------------

// PythonTool executes Python code by writing it to a temp file and running python3.
type PythonTool struct {
	params json.RawMessage
}

// NewPythonTool creates a new PythonTool.
func NewPythonTool() *PythonTool {
	return &PythonTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"code": {"type": "string"}
			},
			"required": ["code"]
		}`),
	}
}

func (t *PythonTool) Name() string                { return "python" }
func (t *PythonTool) Description() string         { return "Execute Python code." }
func (t *PythonTool) Parameters() json.RawMessage { return t.params }
func (t *PythonTool) Metadata() ToolMetadata      { return DefaultMetadata() }

func (t *PythonTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Code    string `json:"code"`
		Timeout int    `json:"timeout"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}
	if args.Code == "" {
		return "", &ToolError{Kind: "invalid_params", Message: "code is required"}
	}

	// Enforce a default timeout of 30 seconds.
	timeout := 30 * time.Second
	if args.Timeout > 0 {
		timeout = time.Duration(args.Timeout) * time.Millisecond
	}
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	tmpFile, err := os.CreateTemp("", "python-*.py")
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}
	defer os.Remove(tmpFile.Name())

	if _, err := tmpFile.WriteString(args.Code); err != nil {
		tmpFile.Close()
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}
	tmpFile.Close()

	cmd := exec.CommandContext(ctx, "python3", tmpFile.Name())
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &limitedWriter{buf: &stdout, max: 1024 * 1024}
	cmd.Stderr = &limitedWriter{buf: &stderr, max: 256 * 1024}

	err = cmd.Run()
	output := stdout.String()
	if stderr.Len() > 0 {
		output += "\n" + stderr.String()
	}
	if err != nil {
		return output, &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	return output, nil
}

// limitedWriter is defined in bash_tool.go

// ---------------------------------------------------------------------------
// CalcTool
// ---------------------------------------------------------------------------

// CalcTool evaluates arithmetic expressions.
type CalcTool struct {
	params json.RawMessage
}

// NewCalcTool creates a new CalcTool.
func NewCalcTool() *CalcTool {
	return &CalcTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"expression": {"type": "string"}
			},
			"required": ["expression"]
		}`),
	}
}

func (t *CalcTool) Name() string                { return "calc" }
func (t *CalcTool) Description() string         { return "Evaluate an arithmetic expression." }
func (t *CalcTool) Parameters() json.RawMessage { return t.params }
func (t *CalcTool) Metadata() ToolMetadata      { return ReadOnlyMetadata() }

func (t *CalcTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Expression string `json:"expression"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}
	if args.Expression == "" {
		return "", &ToolError{Kind: "invalid_params", Message: "expression is required"}
	}

	result, err := evalExpression(args.Expression)
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	return fmt.Sprintf("%s = %s", args.Expression, formatNumber(result)), nil
}

// formatNumber formats a float64 nicely (removes trailing zeros for integers).
func formatNumber(f float64) string {
	if f == float64(int64(f)) {
		return strconv.FormatInt(int64(f), 10)
	}
	return strconv.FormatFloat(f, 'f', -1, 64)
}

// evalExpression evaluates a simple arithmetic expression with +, -, *, /, ().
func evalExpression(expr string) (float64, error) {
	parser := newExprParser(strings.TrimSpace(expr))
	return parser.parse()
}

// Simple recursive-descent arithmetic parser.
type exprParser struct {
	tokens []string
	pos    int
}

func newExprParser(expr string) *exprParser {
	var tokens []string
	var buf strings.Builder
	for _, ch := range expr {
		switch ch {
		case ' ', '\t', '\n':
			if buf.Len() > 0 {
				tokens = append(tokens, buf.String())
				buf.Reset()
			}
		case '+', '-', '*', '/', '(', ')':
			if buf.Len() > 0 {
				tokens = append(tokens, buf.String())
				buf.Reset()
			}
			tokens = append(tokens, string(ch))
		default:
			buf.WriteRune(ch)
		}
	}
	if buf.Len() > 0 {
		tokens = append(tokens, buf.String())
	}
	return &exprParser{tokens: tokens}
}

func (p *exprParser) peek() string {
	if p.pos >= len(p.tokens) {
		return ""
	}
	return p.tokens[p.pos]
}

func (p *exprParser) next() string {
	t := p.peek()
	p.pos++
	return t
}

func (p *exprParser) parse() (float64, error) {
	return p.parseAddSub()
}

func (p *exprParser) parseAddSub() (float64, error) {
	left, err := p.parseMulDiv()
	if err != nil {
		return 0, err
	}
	for {
		op := p.peek()
		if op != "+" && op != "-" {
			break
		}
		p.next()
		right, err := p.parseMulDiv()
		if err != nil {
			return 0, err
		}
		if op == "+" {
			left += right
		} else {
			left -= right
		}
	}
	return left, nil
}

func (p *exprParser) parseMulDiv() (float64, error) {
	left, err := p.parsePrimary()
	if err != nil {
		return 0, err
	}
	for {
		op := p.peek()
		if op != "*" && op != "/" {
			break
		}
		p.next()
		right, err := p.parsePrimary()
		if err != nil {
			return 0, err
		}
		if op == "*" {
			left *= right
		} else {
			if right == 0 {
				return 0, fmt.Errorf("division by zero")
			}
			left /= right
		}
	}
	return left, nil
}

func (p *exprParser) parsePrimary() (float64, error) {
	tok := p.peek()
	if tok == "(" {
		p.next()
		val, err := p.parseAddSub()
		if err != nil {
			return 0, err
		}
		if p.peek() != ")" {
			return 0, fmt.Errorf("expected ')', got %q", p.peek())
		}
		p.next()
		return val, nil
	}

	// Handle unary minus
	if tok == "-" {
		p.next()
		val, err := p.parsePrimary()
		if err != nil {
			return 0, err
		}
		return -val, nil
	}

	// Number
	p.next()
	f, err := strconv.ParseFloat(tok, 64)
	if err != nil {
		return 0, fmt.Errorf("expected number, got %q", tok)
	}
	return f, nil
}

// ---------------------------------------------------------------------------
// TaskTool
// ---------------------------------------------------------------------------

// taskEntry represents an in-memory task.
type taskEntry struct {
	ID          string
	Description string
	Status      string
}

// TaskTool manages an in-memory task list.
type TaskTool struct {
	params json.RawMessage
	mu     sync.Mutex
	tasks  map[string]*taskEntry
	nextID int
}

// NewTaskTool creates a new TaskTool.
func NewTaskTool() *TaskTool {
	return &TaskTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"action": {"type": "string"},
				"id": {"type": "string"},
				"description": {"type": "string"},
				"subagent_type": {"type": "string", "description": "Suggested sub-agent role such as explorer, reviewer, implementer, verifier, or documenter."},
				"scope": {"type": "string", "description": "Files, modules, or problem boundary the sub-agent should own."},
				"expected_output": {"type": "string", "description": "Concrete artifact or answer the sub-agent should return."}
			},
			"required": ["action"]
		}`),
		tasks: make(map[string]*taskEntry),
	}
}

func (t *TaskTool) Name() string                { return "task" }
func (t *TaskTool) Description() string         { return "Manage an in-memory task list." }
func (t *TaskTool) Parameters() json.RawMessage { return t.params }
func (t *TaskTool) Metadata() ToolMetadata      { return ReadOnlyMetadata() }

func (t *TaskTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Action         string `json:"action"`
		ID             string `json:"id"`
		Description    string `json:"description"`
		SubagentType   string `json:"subagent_type"`
		Scope          string `json:"scope"`
		ExpectedOutput string `json:"expected_output"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	t.mu.Lock()
	defer t.mu.Unlock()

	switch args.Action {
	case "create":
		t.nextID++
		id := args.ID
		if id == "" {
			id = fmt.Sprintf("task-%d", t.nextID)
		}
		entry := &taskEntry{ID: id, Description: args.Description, Status: "pending"}
		t.tasks[id] = entry
		return fmt.Sprintf("Task created: %s - %s", id, args.Description), nil

	case "update":
		if args.ID == "" {
			return "", &ToolError{Kind: "invalid_params", Message: "id is required for update"}
		}
		task, ok := t.tasks[args.ID]
		if !ok {
			return "", &ToolError{Kind: "not_found", Message: "task not found: " + args.ID}
		}
		if args.Description != "" {
			task.Description = args.Description
		}
		return fmt.Sprintf("Task updated: %s", args.ID), nil

	case "list":
		if len(t.tasks) == 0 {
			return "No tasks.", nil
		}
		var lines []string
		for _, task := range t.tasks {
			lines = append(lines, fmt.Sprintf("%s\t%s\t%s", task.ID, task.Status, task.Description))
		}
		return strings.Join(lines, "\n"), nil

	case "delete":
		if args.ID == "" {
			return "", &ToolError{Kind: "invalid_params", Message: "id is required for delete"}
		}
		if _, ok := t.tasks[args.ID]; !ok {
			return "", &ToolError{Kind: "not_found", Message: "task not found: " + args.ID}
		}
		delete(t.tasks, args.ID)
		return fmt.Sprintf("Task deleted: %s", args.ID), nil

	case "delegate":
		if args.Description == "" {
			return "", &ToolError{Kind: "invalid_params", Message: "description is required for delegate"}
		}
		if args.SubagentType == "" {
			args.SubagentType = "generalist"
		}
		var lines []string
		lines = append(lines, "Sub-agent delegation")
		lines = append(lines, "type: "+args.SubagentType)
		lines = append(lines, "task: "+args.Description)
		if args.Scope != "" {
			lines = append(lines, "scope: "+args.Scope)
		}
		if args.ExpectedOutput != "" {
			lines = append(lines, "expected_output: "+args.ExpectedOutput)
		}
		lines = append(lines, "coordination: keep ownership narrow, return evidence, changed files, verification commands, and unresolved risks.")
		return strings.Join(lines, "\n"), nil

	default:
		return "", &ToolError{Kind: "invalid_params", Message: "unknown action: " + args.Action}
	}
}

// ---------------------------------------------------------------------------
// Stub tools
// ---------------------------------------------------------------------------

// StubTool is a placeholder tool that returns a "not implemented" error.
type StubTool struct {
	name   string
	desc   string
	params json.RawMessage
}

func newStubTool(name, desc string) *StubTool {
	return &StubTool{
		name:   name,
		desc:   desc,
		params: json.RawMessage(`{"type":"object","properties":{}}`),
	}
}

func (t *StubTool) Name() string                { return t.name }
func (t *StubTool) Description() string         { return t.desc }
func (t *StubTool) Parameters() json.RawMessage { return t.params }
func (t *StubTool) Metadata() ToolMetadata      { return DefaultMetadata() }

func (t *StubTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	return "", &ToolError{Kind: "execution_error", Message: t.name + " tool is not yet implemented"}
}

// NewBrowserTool creates a stub BrowserTool.
func NewBrowserTool() *StubTool {
	return newStubTool("browser", "Interact with a web browser (not implemented).")
}

// NewSshTool creates a stub SshTool.
func NewSshTool() *StubTool {
	return newStubTool("ssh", "Execute commands via SSH (not implemented).")
}

// NewLspTool creates a stub LspTool.
func NewLspTool() *StubTool {
	return newStubTool("lsp", "Language Server Protocol operations (not implemented).")
}

// NewWebSearchTool creates a stub WebSearchTool.
func NewWebSearchTool() *StubTool {
	return newStubTool("web_search", "Search the web (not implemented).")
}

// NewNotebookTool creates a stub NotebookTool.
func NewNotebookTool() *StubTool {
	return newStubTool("notebook", "Jupyter notebook operations (not implemented).")
}

// extractHost extracts the hostname from a URL string.
func extractHost(rawURL string) string {
	u, err := url.Parse(rawURL)
	if err != nil {
		return ""
	}
	host := u.Hostname()
	// Normalize to lowercase for comparison.
	return strings.ToLower(host)
}

// Ensure unused imports are referenced.
var (
	_ = filepath.Join
	_ = io.ReadAll
	_ = (*http.Client)(nil)
	_ = time.Duration(0)
)
