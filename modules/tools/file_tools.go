package tools

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// ---------------------------------------------------------------------------
// ReadFileTool
// ---------------------------------------------------------------------------

// ReadFileTool reads the contents of a file, with optional offset and limit.
type ReadFileTool struct {
	params  json.RawMessage
	pathVal *PathValidator
}

// NewReadFileTool creates a new ReadFileTool.
func NewReadFileTool() *ReadFileTool {
	return &ReadFileTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"path": {"type": "string"},
				"offset": {"type": "integer"},
				"limit": {"type": "integer"}
			},
			"required": ["path"]
		}`),
	}
}

// WithPathValidator sets the path validator for this tool.
func (t *ReadFileTool) WithPathValidator(pv *PathValidator) *ReadFileTool {
	t.pathVal = pv
	return t
}

func (t *ReadFileTool) Name() string                        { return "read_file" }
func (t *ReadFileTool) Description() string                  { return "Read the contents of a file." }
func (t *ReadFileTool) Parameters() json.RawMessage          { return t.params }
func (t *ReadFileTool) Metadata() ToolMetadata               { return ReadOnlyMetadata() }

func (t *ReadFileTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Path   string `json:"path"`
		Offset int    `json:"offset"`
		Limit  int    `json:"limit"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	if t.pathVal != nil {
		if err := t.pathVal.Validate(args.Path); err != nil {
			return "", &ToolError{Kind: "security_violation", Message: err.Error()}
		}
	}

	f, err := os.Open(args.Path)
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	var lines []string
	lineNum := 0

	for scanner.Scan() {
		lineNum++
		if args.Offset > 0 && lineNum < args.Offset {
			continue
		}
		if args.Limit > 0 && len(lines) >= args.Limit {
			break
		}
		lines = append(lines, scanner.Text())
	}

	if err := scanner.Err(); err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	return strings.Join(lines, "\n"), nil
}

// ---------------------------------------------------------------------------
// WriteFileTool
// ---------------------------------------------------------------------------

// WriteFileTool writes content to a file, creating parent directories as needed.
type WriteFileTool struct {
	params  json.RawMessage
	pathVal *PathValidator
}

// NewWriteFileTool creates a new WriteFileTool.
func NewWriteFileTool() *WriteFileTool {
	return &WriteFileTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"path": {"type": "string"},
				"content": {"type": "string"}
			},
			"required": ["path", "content"]
		}`),
	}
}

// WithPathValidator sets the path validator for this tool.
func (t *WriteFileTool) WithPathValidator(pv *PathValidator) *WriteFileTool {
	t.pathVal = pv
	return t
}

func (t *WriteFileTool) Name() string                        { return "write_file" }
func (t *WriteFileTool) Description() string                  { return "Write content to a file, creating parent directories as needed." }
func (t *WriteFileTool) Parameters() json.RawMessage          { return t.params }
func (t *WriteFileTool) Metadata() ToolMetadata               { return DestructiveMetadata() }

func (t *WriteFileTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Path    string `json:"path"`
		Content string `json:"content"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	if args.Path == "" {
		return "", &ToolError{Kind: "invalid_params", Message: "path is required"}
	}

	if t.pathVal != nil {
		if err := t.pathVal.Validate(args.Path); err != nil {
			return "", &ToolError{Kind: "security_violation", Message: err.Error()}
		}
	}

	dir := filepath.Dir(args.Path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	if err := os.WriteFile(args.Path, []byte(args.Content), 0644); err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	return fmt.Sprintf("Successfully wrote %d bytes to %s", len(args.Content), args.Path), nil
}

// ---------------------------------------------------------------------------
// EditFileTool
// ---------------------------------------------------------------------------

// EditFileTool performs string replacement in a file.
type EditFileTool struct {
	params  json.RawMessage
	pathVal *PathValidator
}

// NewEditFileTool creates a new EditFileTool.
func NewEditFileTool() *EditFileTool {
	return &EditFileTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"path": {"type": "string"},
				"old_string": {"type": "string"},
				"new_string": {"type": "string"}
			},
			"required": ["path", "old_string", "new_string"]
		}`),
	}
}

// WithPathValidator sets the path validator for this tool.
func (t *EditFileTool) WithPathValidator(pv *PathValidator) *EditFileTool {
	t.pathVal = pv
	return t
}

func (t *EditFileTool) Name() string                        { return "edit_file" }
func (t *EditFileTool) Description() string                  { return "Edit a file by replacing old_string with new_string." }
func (t *EditFileTool) Parameters() json.RawMessage          { return t.params }
func (t *EditFileTool) Metadata() ToolMetadata               { return DestructiveMetadata() }

func (t *EditFileTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Path      string `json:"path"`
		OldString string `json:"old_string"`
		NewString string `json:"new_string"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	if t.pathVal != nil {
		if err := t.pathVal.Validate(args.Path); err != nil {
			return "", &ToolError{Kind: "security_violation", Message: err.Error()}
		}
	}

	data, err := os.ReadFile(args.Path)
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	content := string(data)

	count := strings.Count(content, args.OldString)
	if count == 0 {
		return "", &ToolError{Kind: "execution_error", Message: "old_string not found in file"}
	}
	if count > 1 {
		return "", &ToolError{Kind: "execution_error", Message: fmt.Sprintf("old_string found %d times; it must be unique", count)}
	}

	newContent := strings.Replace(content, args.OldString, args.NewString, 1)
	if err := os.WriteFile(args.Path, []byte(newContent), 0644); err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	return fmt.Sprintf("Successfully replaced text in %s", args.Path), nil
}

// ---------------------------------------------------------------------------
// DeleteFileTool
// ---------------------------------------------------------------------------

// DeleteFileTool removes a file from the filesystem.
type DeleteFileTool struct {
	params  json.RawMessage
	pathVal *PathValidator
}

// NewDeleteFileTool creates a new DeleteFileTool.
func NewDeleteFileTool() *DeleteFileTool {
	return &DeleteFileTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"path": {"type": "string"}
			},
			"required": ["path"]
		}`),
	}
}

// WithPathValidator sets the path validator for this tool.
func (t *DeleteFileTool) WithPathValidator(pv *PathValidator) *DeleteFileTool {
	t.pathVal = pv
	return t
}

func (t *DeleteFileTool) Name() string                        { return "delete_file" }
func (t *DeleteFileTool) Description() string                  { return "Delete a file." }
func (t *DeleteFileTool) Parameters() json.RawMessage          { return t.params }
func (t *DeleteFileTool) Metadata() ToolMetadata               { return DestructiveMetadata() }

func (t *DeleteFileTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Path string `json:"path"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	if t.pathVal != nil {
		if err := t.pathVal.Validate(args.Path); err != nil {
			return "", &ToolError{Kind: "security_violation", Message: err.Error()}
		}
	}

	if err := os.Remove(args.Path); err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	return fmt.Sprintf("Successfully deleted %s", args.Path), nil
}

// ---------------------------------------------------------------------------
// ListDirectoryTool
// ---------------------------------------------------------------------------

// ListDirectoryTool lists the contents of a directory.
type ListDirectoryTool struct {
	params json.RawMessage
}

// NewListDirectoryTool creates a new ListDirectoryTool.
func NewListDirectoryTool() *ListDirectoryTool {
	return &ListDirectoryTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"path": {"type": "string"}
			},
			"required": ["path"]
		}`),
	}
}

func (t *ListDirectoryTool) Name() string                        { return "list_directory" }
func (t *ListDirectoryTool) Description() string                  { return "List the contents of a directory." }
func (t *ListDirectoryTool) Parameters() json.RawMessage          { return t.params }
func (t *ListDirectoryTool) Metadata() ToolMetadata               { return ReadOnlyMetadata() }

func (t *ListDirectoryTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Path string `json:"path"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	entries, err := os.ReadDir(args.Path)
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	var lines []string
	for _, entry := range entries {
		info, err := entry.Info()
		if err != nil {
			continue
		}
		isDir := entry.IsDir()
		typeStr := "file"
		if isDir {
			typeStr = "dir"
		}
		lines = append(lines, fmt.Sprintf("%s\t%d\t%s", entry.Name(), info.Size(), typeStr))
	}

	return strings.Join(lines, "\n"), nil
}
