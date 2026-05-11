package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// view renders the full TUI layout.
func view(m Model) string {
	if m.quitting {
		return "Goodbye!\n"
	}

	w := m.width
	h := m.height

	// Minimum usable dimensions
	if w < 20 {
		w = 80
	}
	if h < 10 {
		h = 24
	}

	// Reserve 1 row for status bar, 2 rows for input area.
	chatHeight := h - 3
	if chatHeight < 1 {
		chatHeight = 1
	}

	sidebarWidth := 0
	if m.sidebar {
		sidebarWidth = 20
	}
	chatWidth := w - sidebarWidth
	if chatWidth < 20 {
		chatWidth = 20
	}

	// Render sidebar
	sidebarStr := ""
	if m.sidebar {
		sidebarContent := m.theme.Dim.Render("Sessions\n\n") +
			m.theme.Secondary.Render("  (no sessions)")
		sidebarStyle := lipgloss.NewStyle().
			Width(sidebarWidth).
			Height(chatHeight).
			BorderRight(true).
			BorderStyle(lipgloss.RoundedBorder())
		sidebarStr = sidebarStyle.Render(sidebarContent)
	}

	// Render chat messages
	chatContent := renderChatArea(m, chatWidth, chatHeight)
	chatStyle := lipgloss.NewStyle().
		Width(chatWidth).
		Height(chatHeight)
	chatStr := chatStyle.Render(chatContent)

	// Combine sidebar + chat horizontally
	mainArea := lipgloss.JoinHorizontal(lipgloss.Top, sidebarStr, chatStr)

	// Input area
	inputStyle := m.theme.Input.
		Width(w - 4)
	inputStr := inputStyle.Render(fmt.Sprintf("> %s", m.input))

	// Status bar
	statusStr := m.theme.StatusBar(m.mode, "", 0)

	// Combine vertically
	return lipgloss.JoinVertical(lipgloss.Top,
		mainArea,
		inputStr,
		statusStr,
	)
}

// renderChatArea renders all messages into a string.
func renderChatArea(m Model, width, height int) string {
	var lines []string

	for _, msg := range m.messages {
		rendered := m.theme.ChatMessage(msg)
		// Truncate each rendered message to fit width
		for _, line := range strings.Split(rendered, "\n") {
			if len(line) > width {
				line = line[:width]
			}
			lines = append(lines, line)
		}
	}

	// Pad or trim to fit height
	for len(lines) < height {
		lines = append(lines, "")
	}
	if len(lines) > height {
		lines = lines[len(lines)-height:]
	}

	return strings.Join(lines, "\n")
}
