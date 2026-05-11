package tui

import (
	"strings"
	"testing"
	"unicode/utf8"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/woyin/OrangeCoding/modules/core"
)

// sendRunes types each rune into the model one at a time, returning the
// updated model.  This simulates user keyboard input.
func sendRunes(m Model, s string) Model {
	for _, ch := range s {
		updated, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{ch}})
		m = updated.(Model)
	}
	return m
}

// pressKey sends a single non-rune key event.
func pressKey(m Model, keyType tea.KeyType) Model {
	updated, _ := m.Update(tea.KeyMsg{Type: keyType})
	return updated.(Model)
}

// sendSpace types a single space character.  We use utf8.RuneLen to be safe.
func sendSpace(m Model) Model {
	r, _ := utf8.DecodeRuneInString(" ")
	updated, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{r}})
	return updated.(Model)
}

// ---------------------------------------------------------------------------
// TestModelInit
// ---------------------------------------------------------------------------

func TestModelInit(t *testing.T) {
	m := NewModel()
	if m.mode != "normal" {
		t.Errorf("expected mode 'normal', got %q", m.mode)
	}
	if m.sidebar {
		t.Error("expected sidebar to be off by default")
	}
	if m.quitting {
		t.Error("expected quitting to be false initially")
	}
	if m.width != 0 || m.height != 0 {
		t.Errorf("expected zero dimensions, got width=%d height=%d", m.width, m.height)
	}
}

// ---------------------------------------------------------------------------
// TestModelInitCmd
// ---------------------------------------------------------------------------

func TestModelInitCmd(t *testing.T) {
	m := NewModel()
	cmd := m.Init()
	if cmd != nil {
		t.Error("expected Init to return nil cmd")
	}
}

// ---------------------------------------------------------------------------
// TestModelUpdateQuit
// ---------------------------------------------------------------------------

func TestModelUpdateQuit(t *testing.T) {
	m := NewModel()
	um := pressKey(m, tea.KeyCtrlC)
	if !um.quitting {
		t.Error("expected quitting=true after Ctrl+C")
	}

	m = NewModel()
	um = pressKey(m, tea.KeyEsc)
	if !um.quitting {
		t.Error("expected quitting=true after Esc")
	}
}

// ---------------------------------------------------------------------------
// TestModelUpdateWindowSize
// ---------------------------------------------------------------------------

func TestModelUpdateWindowSize(t *testing.T) {
	m := NewModel()
	updated, _ := m.Update(tea.WindowSizeMsg{Width: 120, Height: 40})
	um := updated.(Model)
	if um.width != 120 {
		t.Errorf("expected width 120, got %d", um.width)
	}
	if um.height != 40 {
		t.Errorf("expected height 40, got %d", um.height)
	}
}

// ---------------------------------------------------------------------------
// TestModelView
// ---------------------------------------------------------------------------

func TestModelView(t *testing.T) {
	m := NewModel()
	m.width = 80
	m.height = 24
	v := m.View()
	if v == "" {
		t.Error("expected non-empty view")
	}
}

// ---------------------------------------------------------------------------
// TestModelAddMessage
// ---------------------------------------------------------------------------

func TestModelAddMessage(t *testing.T) {
	m := NewModel()
	updated, _ := m.Update(coreMessageMsg{msg: core.NewUserMessage("hello")})
	um := updated.(Model)
	if len(um.messages) != 1 {
		t.Fatalf("expected 1 message, got %d", len(um.messages))
	}
	if um.messages[0].Content != "hello" {
		t.Errorf("expected content 'hello', got %q", um.messages[0].Content)
	}
}

// ---------------------------------------------------------------------------
// TestMarkdownRender
// ---------------------------------------------------------------------------

func TestMarkdownRender(t *testing.T) {
	r, err := NewMarkdownRenderer()
	if err != nil {
		t.Fatalf("NewMarkdownRenderer failed: %v", err)
	}
	out := r.Render("**bold** and *italic*")
	if out == "" {
		t.Error("expected non-empty rendered output")
	}
}

// ---------------------------------------------------------------------------
// TestThemeStatusBar
// ---------------------------------------------------------------------------

func TestThemeStatusBar(t *testing.T) {
	bar := DarkTheme.StatusBar("normal", "session-abc123", 42)
	if bar == "" {
		t.Error("expected non-empty status bar")
	}
	if !strings.Contains(bar, "normal") {
		t.Error("status bar should contain mode 'normal'")
	}
	if !strings.Contains(bar, "session-abc123") {
		t.Error("status bar should contain session ID")
	}
}

// ---------------------------------------------------------------------------
// TestSlashCommands
// ---------------------------------------------------------------------------

func TestSlashCommands(t *testing.T) {
	// /quit
	m := NewModel()
	m = sendRunes(m, "/quit")
	um := pressKey(m, tea.KeyEnter)
	if !um.quitting {
		t.Error("/quit: expected quitting=true")
	}

	// /clear
	m = NewModel()
	m.messages = []core.Message{core.NewUserMessage("test")}
	m = sendRunes(m, "/clear")
	um = pressKey(m, tea.KeyEnter)
	if len(um.messages) != 0 {
		t.Errorf("/clear: expected 0 messages, got %d", len(um.messages))
	}

	// /help
	m = NewModel()
	m = sendRunes(m, "/help")
	um = pressKey(m, tea.KeyEnter)
	if len(um.messages) == 0 {
		t.Error("/help: expected at least one message")
	}
	if um.messages[0].Role != core.RoleSystem {
		t.Errorf("/help: expected system role, got %v", um.messages[0].Role)
	}
}

// ---------------------------------------------------------------------------
// TestModelToggleSidebar
// ---------------------------------------------------------------------------

func TestModelToggleSidebar(t *testing.T) {
	m := NewModel()
	if m.sidebar {
		t.Error("expected sidebar off initially")
	}
	um := pressKey(m, tea.KeyTab)
	if !um.sidebar {
		t.Error("expected sidebar on after Tab")
	}
	um = pressKey(um, tea.KeyTab)
	if um.sidebar {
		t.Error("expected sidebar off after second Tab")
	}
}

// ---------------------------------------------------------------------------
// TestThemeChatMessage
// ---------------------------------------------------------------------------

func TestThemeChatMessage(t *testing.T) {
	msg := core.NewUserMessage("Hello, world!")
	out := DarkTheme.ChatMessage(msg)
	if out == "" {
		t.Error("expected non-empty chat message output")
	}
	if !strings.Contains(out, "Hello, world!") {
		t.Error("chat message output should contain the original content")
	}
}

// ---------------------------------------------------------------------------
// TestNewApp
// ---------------------------------------------------------------------------

func TestNewApp(t *testing.T) {
	app := NewApp()
	if app == nil {
		t.Fatal("expected non-nil App")
	}
}

// ---------------------------------------------------------------------------
// TestStatusMsg
// ---------------------------------------------------------------------------

func TestStatusMsg(t *testing.T) {
	m := NewModel()
	updated, _ := m.Update(statusMsg{status: "ready"})
	um := updated.(Model)
	if um.status != "ready" {
		t.Errorf("expected status 'ready', got %q", um.status)
	}
}

// ---------------------------------------------------------------------------
// TestModeSwitching
// ---------------------------------------------------------------------------

func TestModeSwitching(t *testing.T) {
	m := NewModel()
	m = sendRunes(m, "/mode")
	m = sendSpace(m)
	m = sendRunes(m, "plan")
	um := pressKey(m, tea.KeyEnter)
	if um.mode != "plan" {
		t.Errorf("expected mode 'plan', got %q", um.mode)
	}
}
