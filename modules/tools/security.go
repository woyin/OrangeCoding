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

// DefaultBlockedCommands is the default list of dangerous command names.
var DefaultBlockedCommands = []string{
	"rm", "rmdir", "mkfs", "dd", "format", "fdisk",
	"shutdown", "reboot", "halt", "poweroff",
	"kill", "killall", "pkill",
	"chmod", "chown", "chgrp",
	"iptables", "ip", "nft",
	"useradd", "userdel", "usermod", "groupadd", "groupdel",
	"mount", "umount",
	"curl", "wget", // use fetch tool instead
}

// SecurityPolicy defines which commands are blocked from execution.
type SecurityPolicy struct {
	BlockedCommands []string
	blockMap        map[string]bool
}

// NewSecurityPolicy creates a SecurityPolicy that blocks the given command names.
func NewSecurityPolicy(blocked []string) *SecurityPolicy {
	m := make(map[string]bool, len(blocked))
	for _, b := range blocked {
		m[b] = true
	}
	return &SecurityPolicy{BlockedCommands: blocked, blockMap: m}
}

// DefaultSecurityPolicy returns a SecurityPolicy with sensible defaults.
func DefaultSecurityPolicy() *SecurityPolicy {
	return NewSecurityPolicy(DefaultBlockedCommands)
}

// IsAllowed returns true if the command passes all security checks.
func (p *SecurityPolicy) IsAllowed(command string) bool {
	cmd := strings.TrimSpace(command)
	if cmd == "" {
		return false
	}

	// Block shell injection patterns.
	if containsShellInjection(cmd) {
		return false
	}

	// Extract the effective command name.
	effective := extractCommand(cmd)
	effective = filepath.Base(effective)

	if p.blockMap[effective] {
		return false
	}
	return true
}

// extractCommand extracts the first meaningful command token,
// handling pipes, chains, subshells, and env var prefixes.
func extractCommand(cmd string) string {
	// Skip env assignments like FOO=bar cmd ...
	parts := strings.Fields(cmd)
	for _, p := range parts {
		if !strings.Contains(p, "=") {
			return p
		}
	}
	return cmd
}

// containsShellInjection detects common injection patterns.
func containsShellInjection(cmd string) bool {
	dangerous := []string{
		"$(", // command substitution
		"`",  // backtick execution
		"${", // variable expansion (could be injection)
		"|",  // pipe
		"&&", // command chaining
		"||", // command chaining
		";",  // command separator
		"\n", // newline (command separator)
	}
	for _, d := range dangerous {
		if strings.Contains(cmd, d) {
			return true
		}
	}
	// Check eval/exec as word-boundary matches to avoid false positives
	// with words like "evaluate", "execute", "retrieval".
	words := strings.Fields(cmd)
	for _, w := range words {
		base := filepath.Base(w)
		if base == "eval" || base == "exec" {
			return true
		}
	}
	return false
}

// lookPath wraps exec.LookPath for use in tool implementations.
func lookPath(name string) (string, error) {
	return exec.LookPath(name)
}
