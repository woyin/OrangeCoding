package tui

import (
	tea "github.com/charmbracelet/bubbletea"
)

// App wraps the Bubble Tea program and provides a simple entry point.
type App struct {
	model Model
}

// NewApp creates a new TUI application.
func NewApp() *App {
	return &App{
		model: NewModel(),
	}
}

// Run starts the Bubble Tea program and blocks until it exits.
func (a *App) Run() error {
	p := tea.NewProgram(a.model,
		tea.WithAltScreen(),
	)
	_, err := p.Run()
	return err
}
