package tui

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/woyin/OrangeCoding/modules/core"
)

// Theme holds the colour/style palette for the TUI.
type Theme struct {
	Name      string
	Primary   lipgloss.Style
	Secondary lipgloss.Style
	Success   lipgloss.Style
	Error     lipgloss.Style
	Dim       lipgloss.Style
	Border    lipgloss.Style
	Input     lipgloss.Style
}

// DarkTheme is the default dark colour scheme.
var DarkTheme = Theme{
	Name: "dark",
	Primary: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#7D56F4")).
		Bold(true),
	Secondary: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#9B9B9B")),
	Success: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#04B575")),
	Error: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#FF5F87")),
	Dim: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#626262")),
	Border: lipgloss.NewStyle().
		BorderForeground(lipgloss.Color("#4A4A4A")),
	Input: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#FAFAFA")).
		Background(lipgloss.Color("#3C3C3C")),
}

// LightTheme is a light colour scheme.
var LightTheme = Theme{
	Name: "light",
	Primary: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#7D56F4")).
		Bold(true),
	Secondary: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#666666")),
	Success: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#04B575")),
	Error: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#CC0000")),
	Dim: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#AAAAAA")),
	Border: lipgloss.NewStyle().
		BorderForeground(lipgloss.Color("#CCCCCC")),
	Input: lipgloss.NewStyle().
		Foreground(lipgloss.Color("#1A1A1A")).
		Background(lipgloss.Color("#E0E0E0")),
}

// StatusBar renders the bottom status bar with mode, session, and token info.
func (t Theme) StatusBar(mode, sessionID string, tokens uint64) string {
	modeStr := t.Primary.Render(fmt.Sprintf("mode=%s", mode))
	sessionStr := ""
	if sessionID != "" {
		sessionStr = t.Secondary.Render(fmt.Sprintf(" session=%s", sessionID))
	}
	tokenStr := t.Dim.Render(fmt.Sprintf(" tokens=%d", tokens))

	return fmt.Sprintf("%s%s%s", modeStr, sessionStr, tokenStr)
}

// ChatMessage formats a single core.Message for display in the chat area.
func (t Theme) ChatMessage(msg core.Message) string {
	var roleLabel string
	switch msg.Role {
	case core.RoleSystem:
		roleLabel = t.Dim.Render("[system]")
	case core.RoleUser:
		roleLabel = t.Primary.Render("[user]")
	case core.RoleAssistant:
		roleLabel = t.Success.Render("[assistant]")
	case core.RoleTool:
		roleLabel = t.Secondary.Render("[tool]")
	default:
		roleLabel = t.Dim.Render("[?]")
	}

	return fmt.Sprintf("%s %s", roleLabel, msg.Content)
}
