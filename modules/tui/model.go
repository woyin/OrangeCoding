package tui

import (
	"github.com/charmbracelet/bubbletea"
	"github.com/woyin/OrangeCoding/modules/core"
)

// Model is the Bubble Tea model for the TUI application.
type Model struct {
	messages []core.Message
	input    string
	width    int
	height   int
	sidebar  bool
	status   string
	mode     string // "normal", "plan", "goal", "ultra"
	theme    Theme
	err      error
	quitting bool
}

// NewModel creates a new Model with sensible defaults.
func NewModel() Model {
	return Model{
		messages: []core.Message{},
		mode:     "normal",
		status:   "ready",
		theme:    DarkTheme,
		sidebar:  false,
	}
}

// Init satisfies tea.Model. Returns nil — no initial commands.
func (m Model) Init() tea.Cmd {
	return nil
}

// Update satisfies tea.Model. Delegates to the package-level update function.
func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	return update(m, msg)
}

// View satisfies tea.Model. Delegates to the package-level view function.
func (m Model) View() string {
	return view(m)
}
