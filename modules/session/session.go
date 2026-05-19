package session

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// Session represents a conversation session with messages and metadata.
type Session struct {
	ID         core.SessionId    `json:"id"`
	Messages   []core.Message    `json:"messages"`
	Metadata   map[string]string `json:"metadata"`
	TokenUsage core.TokenUsage   `json:"token_usage"`
	CreatedAt  time.Time         `json:"created_at"`
	UpdatedAt  time.Time         `json:"updated_at"`
	ParentID   *core.SessionId   `json:"parent_id,omitempty"`
}

// SessionManager manages session persistence on disk.
type SessionManager struct {
	storageDir string
}

// NewSessionManager creates a SessionManager that stores sessions in the given directory.
func NewSessionManager(storageDir string) *SessionManager {
	return &SessionManager{storageDir: storageDir}
}

// Create creates a new session with a random ID, empty messages, and current timestamps.
// The session is NOT persisted to disk until Update is called.
func (m *SessionManager) Create() *Session {
	now := time.Now().UTC()
	return &Session{
		ID:        core.NewSessionId(),
		Messages:  []core.Message{},
		Metadata:  make(map[string]string),
		CreatedAt: now,
		UpdatedAt: now,
	}
}

// Get loads a session from disk by its ID.
func (m *SessionManager) Get(id core.SessionId) (*Session, error) {
	return ReadSession(m.storageDir, id)
}

// Update persists the session to disk and updates UpdatedAt.
func (m *SessionManager) Update(s *Session) error {
	s.UpdatedAt = time.Now().UTC()
	return WriteSession(m.storageDir, s)
}

// Delete removes a session file from disk.
func (m *SessionManager) Delete(id core.SessionId) error {
	path := filepath.Join(m.storageDir, id.String()+".jsonl")
	if err := os.Remove(path); err != nil {
		return fmt.Errorf("session delete: %w", err)
	}
	return nil
}

// List returns all sessions sorted by UpdatedAt descending (most recent first).
// Skips unreadable or corrupted session files.
func (m *SessionManager) List() ([]*Session, error) {
	if err := os.MkdirAll(m.storageDir, 0o755); err != nil {
		return nil, fmt.Errorf("session list mkdir: %w", err)
	}

	entries, err := os.ReadDir(m.storageDir)
	if err != nil {
		return nil, fmt.Errorf("session list readdir: %w", err)
	}

	var sessions []*Session
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".jsonl" {
			continue
		}

		// Parse session ID from filename.
		baseName := entry.Name()[:len(entry.Name())-len(".jsonl")]
		id, err := core.ParseSessionId(baseName)
		if err != nil {
			continue
		}

		// Single read per file.
		s, err := ReadSession(m.storageDir, id)
		if err != nil {
			continue
		}
		sessions = append(sessions, s)
	}

	sort.Slice(sessions, func(i, j int) bool {
		return sessions[i].UpdatedAt.After(sessions[j].UpdatedAt)
	})

	return sessions, nil
}
