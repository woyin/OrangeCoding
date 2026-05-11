package ai

import (
	"bufio"
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
func ParseSSEStream(r io.Reader) []string {
	var payloads []string
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		line := scanner.Text()

		// Skip empty lines
		if strings.TrimSpace(line) == "" {
			continue
		}

		// Skip comment lines (SSE spec: lines starting with ":")
		if strings.HasPrefix(line, ":") {
			continue
		}

		// Only process data lines
		if !strings.HasPrefix(line, "data: ") {
			continue
		}

		payload := strings.TrimPrefix(line, "data: ")

		// Skip the DONE sentinel
		if strings.TrimSpace(payload) == "[DONE]" {
			continue
		}

		payloads = append(payloads, payload)
	}
	return payloads
}
