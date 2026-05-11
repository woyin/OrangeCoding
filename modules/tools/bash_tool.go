package tools

import (
	"bytes"
	"context"
	"encoding/json"
	"os/exec"
	"time"
)

// BashTool executes shell commands.
type BashTool struct {
	policy *SecurityPolicy
	params json.RawMessage
}

// NewBashTool creates a new BashTool. If policy is non-nil, commands are checked
// against the security policy before execution.
func NewBashTool(policy *SecurityPolicy) *BashTool {
	return &BashTool{
		policy: policy,
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"command": {"type": "string"},
				"timeout": {"type": "integer"}
			},
			"required": ["command"]
		}`),
	}
}

// Name returns "bash".
func (t *BashTool) Name() string { return "bash" }

// Description returns a description of the bash tool.
func (t *BashTool) Description() string {
	return "Execute a shell command and return its output."
}

// Parameters returns the JSON Schema for bash tool parameters.
func (t *BashTool) Parameters() json.RawMessage { return t.params }

// Metadata returns DestructiveMetadata.
func (t *BashTool) Metadata() ToolMetadata { return DestructiveMetadata() }

// Execute runs the given command via the system shell.
func (t *BashTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Command string `json:"command"`
		Timeout int    `json:"timeout"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	if args.Command == "" {
		return "", &ToolError{Kind: "invalid_params", Message: "command is required"}
	}

	// Security check
	if t.policy != nil && !t.policy.IsAllowed(args.Command) {
		return "", &ToolError{Kind: "security_violation", Message: "command is blocked by security policy: " + args.Command}
	}

	// Set up timeout
	if args.Timeout > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, time.Duration(args.Timeout)*time.Millisecond)
		defer cancel()
	}

	cmd := exec.CommandContext(ctx, "sh", "-c", args.Command)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	err := cmd.Run()
	output := stdout.String()
	if stderr.Len() > 0 {
		output += "\n" + stderr.String()
	}

	if err != nil {
		if output == "" {
			output = err.Error()
		}
		return output, err
	}

	return output, nil
}
