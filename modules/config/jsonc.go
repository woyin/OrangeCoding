package config

import (
	"fmt"
	"strings"
)

// ParseJSONC strips // line comments and /* */ block comments from a JSONC
// input string while preserving comments inside quoted strings. It returns
// clean JSON suitable for standard json.Unmarshal.
func ParseJSONC(input string) (string, error) {
	var sb strings.Builder
	inString := false
	escape := false
	i := 0
	n := len(input)

	for i < n {
		ch := input[i]

		if inString {
			sb.WriteByte(ch)
			if escape {
				escape = false
			} else if ch == '\\' {
				escape = true
			} else if ch == '"' {
				inString = false
			}
			i++
			continue
		}

		// Outside strings
		switch {
		case ch == '"':
			sb.WriteByte(ch)
			inString = true
			i++

		case ch == '/' && i+1 < n && input[i+1] == '/':
			// Line comment: skip to end of line
			end := strings.IndexByte(input[i:], '\n')
			if end == -1 {
				// Rest of input is a comment
				i = n
			} else {
				i += end + 1 // skip past \n
			}

		case ch == '/' && i+1 < n && input[i+1] == '*':
			// Block comment: skip to */
			end := strings.Index(input[i:], "*/")
			if end == -1 {
				return "", fmt.Errorf("unterminated block comment starting at position %d", i)
			}
			i += end + 2 // skip past */

		default:
			sb.WriteByte(ch)
			i++
		}
	}

	if inString {
		return "", fmt.Errorf("unterminated string literal")
	}

	return sb.String(), nil
}
