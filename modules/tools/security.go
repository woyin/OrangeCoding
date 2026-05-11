package tools

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"
)

// PathValidator ensures that file paths are within allowed directories.
type PathValidator struct {
	AllowedDirs []string
}

// NewPathValidator creates a PathValidator that restricts access to the given directories.
// Each allowed directory is converted to an absolute path.
func NewPathValidator(allowedDirs []string) *PathValidator {
	absDirs := make([]string, len(allowedDirs))
	for i, d := range allowedDirs {
		abs, _ := filepath.Abs(d)
		absDirs[i] = abs
	}
	return &PathValidator{AllowedDirs: absDirs}
}

// Validate checks that the given path is within one of the allowed directories
// and does not contain path traversal sequences.
func (v *PathValidator) Validate(path string) error {
	abs, err := filepath.Abs(path)
	if err != nil {
		return fmt.Errorf("invalid path: %w", err)
	}

	// Check for traversal sequences in the original path.
	if strings.Contains(path, "..") {
		// After cleaning, verify the resolved path is still within allowed dirs.
		cleaned := filepath.Clean(path)
		absCleaned, _ := filepath.Abs(cleaned)
		if absCleaned != abs {
			return fmt.Errorf("path traversal detected: %q resolves outside allowed directories", path)
		}
	}

	for _, dir := range v.AllowedDirs {
		if strings.HasPrefix(abs, dir+string(filepath.Separator)) || abs == dir {
			return nil
		}
	}
	return fmt.Errorf("path %q is outside allowed directories", path)
}

// SecurityPolicy defines which commands are blocked from execution.
type SecurityPolicy struct {
	BlockedCommands []string // exact command names (first token) that are denied
}

// NewSecurityPolicy creates a SecurityPolicy that blocks the given command names.
func NewSecurityPolicy(blocked []string) *SecurityPolicy {
	return &SecurityPolicy{BlockedCommands: blocked}
}

// IsAllowed returns true if the command is not in the blocked list.
// It extracts the first token of the command for comparison.
func (p *SecurityPolicy) IsAllowed(command string) bool {
	// Extract the base command name.
	cmd := strings.TrimSpace(command)
	if idx := strings.Index(cmd, " "); idx >= 0 {
		cmd = cmd[:idx]
	}
	// Get just the binary name (handle paths like /usr/bin/rm)
	cmd = filepath.Base(cmd)

	for _, blocked := range p.BlockedCommands {
		if cmd == blocked {
			return false
		}
	}
	return true
}

// lookPath wraps exec.LookPath for use in tool implementations.
func lookPath(name string) (string, error) {
	return exec.LookPath(name)
}
