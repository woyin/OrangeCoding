package tui

import (
	"github.com/charmbracelet/glamour"
)

// MarkdownRenderer wraps a glamour.TermRenderer for converting Markdown to
// ANSI terminal output.
type MarkdownRenderer struct {
	renderer *glamour.TermRenderer
}

// NewMarkdownRenderer creates a new MarkdownRenderer with the dark style.
func NewMarkdownRenderer() (*MarkdownRenderer, error) {
	r, err := glamour.NewTermRenderer(
		glamour.WithAutoStyle(),
		glamour.WithWordWrap(80),
	)
	if err != nil {
		return nil, err
	}
	return &MarkdownRenderer{renderer: r}, nil
}

// Render converts the given Markdown content to styled ANSI terminal output.
func (r *MarkdownRenderer) Render(content string) string {
	out, err := r.renderer.Render(content)
	if err != nil {
		// Fall back to raw content on error
		return content
	}
	return out
}
