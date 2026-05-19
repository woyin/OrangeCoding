package session

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// WriteSession writes a session to a JSONL file atomically (write-to-temp + rename).
func WriteSession(dir string, s *Session) error {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return fmt.Errorf("session storage mkdir: %w", err)
	}

	path := filepath.Join(dir, s.ID.String()+".jsonl")
	tmpPath := path + ".tmp"
	f, err := os.Create(tmpPath)
	if err != nil {
		return fmt.Errorf("session storage create: %w", err)
	}

	writer := bufio.NewWriter(f)

	// Write session header as first line
	header := sessionHeader{
		ID:         s.ID,
		Metadata:   s.Metadata,
		TokenUsage: s.TokenUsage,
		CreatedAt:  s.CreatedAt,
		UpdatedAt:  s.UpdatedAt,
		ParentID:   s.ParentID,
	}
	headerBytes, err := json.Marshal(header)
	if err != nil {
		return fmt.Errorf("session storage marshal header: %w", err)
	}
	if _, err := writer.Write(headerBytes); err != nil {
		return fmt.Errorf("session storage write header: %w", err)
	}
	if _, err := writer.Write([]byte("\n")); err != nil {
		return fmt.Errorf("session storage write newline: %w", err)
	}

	// Write each message as a JSON line
	for _, msg := range s.Messages {
		msgBytes, err := json.Marshal(msg)
		if err != nil {
			return fmt.Errorf("session storage marshal message: %w", err)
		}
		if _, err := writer.Write(msgBytes); err != nil {
			return fmt.Errorf("session storage write message: %w", err)
		}
		if _, err := writer.Write([]byte("\n")); err != nil {
			return fmt.Errorf("session storage write newline: %w", err)
		}
	}

	if err := writer.Flush(); err != nil {
		f.Close()
		os.Remove(tmpPath)
		return fmt.Errorf("session storage flush: %w", err)
	}
	if err := f.Close(); err != nil {
		os.Remove(tmpPath)
		return fmt.Errorf("session storage close: %w", err)
	}
	return os.Rename(tmpPath, path)
}

// ReadSession reads a session from a JSONL file in the given directory.
func ReadSession(dir string, id core.SessionId) (*Session, error) {
	path := filepath.Join(dir, id.String()+".jsonl")
	f, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("session storage open: %w", err)
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024) // 1MB max line

	// Read header line
	if !scanner.Scan() {
		return nil, fmt.Errorf("session storage: empty file %s", path)
	}

	var header sessionHeader
	if err := json.Unmarshal(scanner.Bytes(), &header); err != nil {
		return nil, fmt.Errorf("session storage unmarshal header: %w", err)
	}

	s := &Session{
		ID:         header.ID,
		Messages:   make([]core.Message, 0),
		Metadata:   header.Metadata,
		TokenUsage: header.TokenUsage,
		CreatedAt:  header.CreatedAt,
		UpdatedAt:  header.UpdatedAt,
		ParentID:   header.ParentID,
	}

	// Read message lines
	for scanner.Scan() {
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}
		var msg core.Message
		if err := json.Unmarshal(line, &msg); err != nil {
			return nil, fmt.Errorf("session storage unmarshal message: %w", err)
		}
		s.Messages = append(s.Messages, msg)
	}

	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("session storage scan: %w", err)
	}

	return s, nil
}

// sessionHeader is the first line of a JSONL session file, containing
// all session metadata except messages.
type sessionHeader struct {
	ID         core.SessionId    `json:"id"`
	Metadata   map[string]string `json:"metadata"`
	TokenUsage core.TokenUsage   `json:"token_usage"`
	CreatedAt  time.Time         `json:"created_at"`
	UpdatedAt  time.Time         `json:"updated_at"`
	ParentID   *core.SessionId   `json:"parent_id,omitempty"`
}
