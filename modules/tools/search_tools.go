package tools

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

// ---------------------------------------------------------------------------
// GrepTool
// ---------------------------------------------------------------------------

// GrepTool searches for a regex pattern in files within a directory.
type GrepTool struct {
	params json.RawMessage
}

// NewGrepTool creates a new GrepTool.
func NewGrepTool() *GrepTool {
	return &GrepTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"pattern": {"type": "string"},
				"path": {"type": "string"},
				"include": {"type": "string"}
			},
			"required": ["pattern"]
		}`),
	}
}

func (t *GrepTool) Name() string                        { return "grep" }
func (t *GrepTool) Description() string                  { return "Search for a regex pattern in files." }
func (t *GrepTool) Parameters() json.RawMessage          { return t.params }
func (t *GrepTool) Metadata() ToolMetadata               { return ReadOnlyMetadata() }

func (t *GrepTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Pattern string `json:"pattern"`
		Path    string `json:"path"`
		Include string `json:"include"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	if args.Pattern == "" {
		return "", &ToolError{Kind: "invalid_params", Message: "pattern is required"}
	}
	if args.Path == "" {
		args.Path = "."
	}

	re, err := regexp.Compile(args.Pattern)
	if err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: "invalid regex: " + err.Error()}
	}

	var matches []string
	const maxMatches = 1000
	includeRe := (*regexp.Regexp)(nil)
	if args.Include != "" {
		includeRe, err = regexp.Compile(args.Include)
		if err != nil {
			return "", &ToolError{Kind: "invalid_params", Message: "invalid include pattern: " + err.Error()}
		}
	}

	err = filepath.WalkDir(args.Path, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if d.IsDir() {
			return nil
		}

		// Apply include filter
		if includeRe != nil && !includeRe.MatchString(d.Name()) {
			return nil
		}

		f, err := os.Open(path)
		if err != nil {
			return nil
		}
		defer f.Close()

		scanner := bufio.NewScanner(f)
		lineNum := 0
		for scanner.Scan() {
			lineNum++
			line := scanner.Text()
			if re.MatchString(line) {
				// Use relative path if possible
				relPath := path
				if rel, err := filepath.Rel(args.Path, path); err == nil {
					relPath = rel
				}
				matches = append(matches, fmt.Sprintf("%s:%d: %s", relPath, lineNum, line))
				if len(matches) >= maxMatches {
					return filepath.SkipAll
				}
			}
		}
		return nil
	})

	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	if len(matches) == 0 {
		return "No matches found.", nil
	}

	return strings.Join(matches, "\n"), nil
}

// ---------------------------------------------------------------------------
// FindTool
// ---------------------------------------------------------------------------

// FindTool walks a directory tree and finds files/directories matching criteria.
type FindTool struct {
	params json.RawMessage
}

// NewFindTool creates a new FindTool.
func NewFindTool() *FindTool {
	return &FindTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"path": {"type": "string"},
				"name": {"type": "string"},
				"type": {"type": "string"}
			},
			"required": ["path"]
		}`),
	}
}

func (t *FindTool) Name() string                        { return "find" }
func (t *FindTool) Description() string                  { return "Find files and directories matching criteria." }
func (t *FindTool) Parameters() json.RawMessage          { return t.params }
func (t *FindTool) Metadata() ToolMetadata               { return ReadOnlyMetadata() }

func (t *FindTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Path string `json:"path"`
		Name string `json:"name"`
		Type string `json:"type"` // "file" or "dir"
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	var nameRe *regexp.Regexp
	if args.Name != "" {
		var err error
		// Convert glob-style pattern to regex
		pattern := globToRegex(args.Name)
		nameRe, err = regexp.Compile(pattern)
		if err != nil {
			return "", &ToolError{Kind: "invalid_params", Message: "invalid name pattern: " + err.Error()}
		}
	}

	var results []string

	err := filepath.WalkDir(args.Path, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}

		// Type filter
		if args.Type == "file" && d.IsDir() {
			return nil
		}
		if args.Type == "dir" && !d.IsDir() {
			return nil
		}

		// Name filter
		if nameRe != nil && !nameRe.MatchString(d.Name()) {
			return nil
		}

		results = append(results, path)
		return nil
	})

	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	if len(results) == 0 {
		return "No results found.", nil
	}

	return strings.Join(results, "\n"), nil
}

// globToRegex converts a simple glob pattern (e.g. "*.go") to a regex pattern.
func globToRegex(glob string) string {
	var buf strings.Builder
	for _, ch := range glob {
		switch ch {
		case '*':
			buf.WriteString(".*")
		case '?':
			buf.WriteString(".")
		case '.', '(', ')', '+', '|', '^', '$', '@', '%', '{', '}', '[', ']':
			buf.WriteRune('\\')
			buf.WriteRune(ch)
		default:
			buf.WriteRune(ch)
		}
	}
	return "^" + buf.String() + "$"
}

// ---------------------------------------------------------------------------
// GlobTool
// ---------------------------------------------------------------------------

// GlobTool finds files matching a glob pattern.
type GlobTool struct {
	params json.RawMessage
}

// NewGlobTool creates a new GlobTool.
func NewGlobTool() *GlobTool {
	return &GlobTool{
		params: json.RawMessage(`{
			"type": "object",
			"properties": {
				"pattern": {"type": "string"},
				"path": {"type": "string"}
			},
			"required": ["pattern"]
		}`),
	}
}

func (t *GlobTool) Name() string                        { return "glob" }
func (t *GlobTool) Description() string                  { return "Find files matching a glob pattern." }
func (t *GlobTool) Parameters() json.RawMessage          { return t.params }
func (t *GlobTool) Metadata() ToolMetadata               { return ReadOnlyMetadata() }

func (t *GlobTool) Execute(ctx context.Context, input json.RawMessage) (string, error) {
	var args struct {
		Pattern string `json:"pattern"`
		Path    string `json:"path"`
	}
	if err := json.Unmarshal(input, &args); err != nil {
		return "", &ToolError{Kind: "invalid_params", Message: err.Error()}
	}

	if args.Pattern == "" {
		return "", &ToolError{Kind: "invalid_params", Message: "pattern is required"}
	}

	// If path is given, make pattern relative to it
	pattern := args.Pattern
	if args.Path != "" {
		pattern = filepath.Join(args.Path, args.Pattern)
	}

	matches, err := filepath.Glob(pattern)
	if err != nil {
		return "", &ToolError{Kind: "execution_error", Message: err.Error()}
	}

	if len(matches) == 0 {
		return "No matches found.", nil
	}

	return strings.Join(matches, "\n"), nil
}
