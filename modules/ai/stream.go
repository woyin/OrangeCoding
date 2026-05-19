package ai

import (
	"bufio"
	"fmt"
	"io"
	"strings"
)

// ---------------------------------------------------------------------------
// SSE stream parser
// ---------------------------------------------------------------------------

// ParseSSEStream reads Server-Sent Events from r and returns the data payloads.
// It reads lines prefixed with "data: " and collects the payload after the prefix.
// Empty lines and comment lines (starting with ":") are skipped.
// Lines with "data: [DONE]" are skipped (this is the stream terminator).
func ParseSSEStream(r io.Reader) ([]string, error) {
	var payloads []string
	scanner := bufio.NewScanner(r)
	// Allow larger lines for big tool call payloads.
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for scanner.Scan() {
		line := scanner.Text()

		if strings.TrimSpace(line) == "" {
			continue
		}
		if strings.HasPrefix(line, ":") {
			continue
		}
		if !strings.HasPrefix(line, "data: ") {
			continue
		}

		payload := strings.TrimPrefix(line, "data: ")
		if strings.TrimSpace(payload) == "[DONE]" {
			continue
		}

		payloads = append(payloads, payload)
	}
	if err := scanner.Err(); err != nil {
		return payloads, fmt.Errorf("SSE stream read error: %w", err)
	}
	return payloads, nil
}
