package tui

import (
	"strings"

	"github.com/charmbracelet/bubbletea"
	"github.com/woyin/OrangeCoding/modules/core"
)

// coreMessageMsg is a custom tea.Msg that carries a core.Message into the model.
type coreMessageMsg struct {
	msg core.Message
}

// statusMsg is a custom tea.Msg that updates the status bar text.
type statusMsg struct {
	status string
}

// knownSlashCommands is the set of slash commands the TUI recognises.
var knownSlashCommands = map[string]bool{
	"/help":  true,
	"/quit":  true,
	"/clear": true,
	"/model": true,
	"/mode":  true,
	"/think": true,
	"/plan":  true,
}

// update is the central Bubble Tea update function.
func update(m Model, msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case coreMessageMsg:
		m.messages = append(m.messages, msg.msg)
		return m, nil

	case statusMsg:
		m.status = msg.status
		return m, nil

	case tea.KeyMsg:
		return handleKey(m, msg)
	}

	return m, nil
}

// handleKey processes key events.
func handleKey(m Model, msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.Type {

	case tea.KeyCtrlC:
		m.quitting = true
		return m, tea.Quit

	case tea.KeyEsc:
		m.quitting = true
		return m, tea.Quit

	case tea.KeyTab:
		m.sidebar = !m.sidebar
		return m, nil

	case tea.KeyEnter:
		return handleInput(m)

	case tea.KeyRunes:
		m.input += string(msg.Runes)
		return m, nil

	case tea.KeyBackspace:
		if len(m.input) > 0 {
			m.input = m.input[:len(m.input)-1]
		}
		return m, nil

	default:
		return m, nil
	}
}

// handleInput processes the current input buffer when Enter is pressed.
func handleInput(m Model) (tea.Model, tea.Cmd) {
	text := strings.TrimSpace(m.input)
	m.input = ""

	if text == "" {
		return m, nil
	}

	// Slash commands
	if strings.HasPrefix(text, "/") {
		return handleSlashCommand(m, text)
	}

	// Regular user message
	m.messages = append(m.messages, core.NewUserMessage(text))
	return m, nil
}

// handleSlashCommand dispatches recognised slash commands.
func handleSlashCommand(m Model, text string) (tea.Model, tea.Cmd) {
	parts := strings.Fields(text)
	cmd := parts[0]

	switch cmd {
	case "/quit":
		m.quitting = true
		return m, tea.Quit

	case "/clear":
		m.messages = m.messages[:0]
		return m, nil

	case "/help":
		helpText := `Available commands:
  /help   - Show this help message
  /quit   - Quit the application
  /clear  - Clear conversation history
  /mode   - Switch mode (normal, plan, goal, ultra)
  /model  - Switch model
  /think  - Toggle thinking mode
  /plan   - Enter plan mode`
		m.messages = append(m.messages, core.NewSystemMessage(helpText))
		return m, nil

	case "/mode":
		if len(parts) >= 2 {
			newMode := parts[1]
			switch newMode {
			case "normal", "plan", "goal", "ultra":
				m.mode = newMode
				m.status = "mode=" + newMode
			default:
				m.messages = append(m.messages, core.NewSystemMessage("unknown mode: "+newMode))
			}
		} else {
			m.messages = append(m.messages, core.NewSystemMessage("usage: /mode <normal|plan|goal|ultra>"))
		}
		return m, nil

	case "/model":
		if len(parts) >= 2 {
			m.status = "model=" + parts[1]
		} else {
			m.messages = append(m.messages, core.NewSystemMessage("usage: /model <name>"))
		}
		return m, nil

	case "/think":
		m.status = "thinking enabled"
		return m, nil

	case "/plan":
		m.mode = "plan"
		m.status = "mode=plan"
		return m, nil

	default:
		if knownSlashCommands[cmd] {
			m.messages = append(m.messages, core.NewSystemMessage("command not yet implemented: "+cmd))
		} else {
			m.messages = append(m.messages, core.NewSystemMessage("unknown command: "+cmd))
		}
		return m, nil
	}
}
