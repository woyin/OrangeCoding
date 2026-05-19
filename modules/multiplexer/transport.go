package multiplexer

import (
	"bufio"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// IPC message types exchanged over the socket.
const (
	IPCTask      = "task"
	IPCResult    = "result"
	IPCEvent     = "event"
	IPCKeepalive = "keepalive"
)

// IPCMessage is the envelope for all inter-pane communication.
type IPCMessage struct {
	Type    string          `json:"type"`
	ID      string          `json:"id"`
	Payload json.RawMessage `json:"payload"`
}

// TaskPayload is sent parent -> child to assign work.
type TaskPayload struct {
	Task      string            `json:"task"`
	AgentID   string            `json:"agent_id"`
	SessionID string            `json:"session_id"`
	Tools     []string          `json:"tools"`
	Env       map[string]string `json:"env,omitempty"`
}

// ResultPayload is sent child -> parent when the task finishes.
type ResultPayload struct {
	Success bool   `json:"success"`
	Content string `json:"content"`
	Error   string `json:"error,omitempty"`
}

// EventPayload is sent child -> parent for streaming updates.
type EventPayload struct {
	EventType string `json:"event_type"`
	Data      string `json:"data"`
}

// SocketTransport implements bidirectional IPC over a Unix domain socket.
// It satisfies a line-delimited JSON protocol compatible with the mcp.Transport pattern.
// Send is safe for concurrent use; Receive is not.
type SocketTransport struct {
	conn    net.Conn
	reader  *bufio.Reader
	writer  *bufio.Writer
	writeMu sync.Mutex
}

// NewSocketTransport wraps an existing net.Conn.
func NewSocketTransport(conn net.Conn) *SocketTransport {
	return &SocketTransport{
		conn:   conn,
		reader: bufio.NewReader(conn),
		writer: bufio.NewWriter(conn),
	}
}

// Send writes a JSON-encoded message as a single newline-terminated line.
// Safe for concurrent use.
func (t *SocketTransport) Send(msg IPCMessage) error {
	data, err := json.Marshal(msg)
	if err != nil {
		return fmt.Errorf("marshal IPC message: %w", err)
	}
	t.writeMu.Lock()
	defer t.writeMu.Unlock()
	if _, err := t.writer.Write(data); err != nil {
		return err
	}
	if err := t.writer.WriteByte('\n'); err != nil {
		return err
	}
	return t.writer.Flush()
}

// Receive reads the next newline-terminated JSON message.
func (t *SocketTransport) Receive() (IPCMessage, error) {
	line, err := t.reader.ReadBytes('\n')
	if err != nil {
		return IPCMessage{}, err
	}
	var msg IPCMessage
	if err := json.Unmarshal(line[:len(line)-1], &msg); err != nil {
		return IPCMessage{}, fmt.Errorf("unmarshal IPC message: %w", err)
	}
	return msg, nil
}

// Close closes the underlying connection.
func (t *SocketTransport) Close() error {
	return t.conn.Close()
}

// CreateListener creates a Unix domain socket listener at the given path.
// Parent side calls this to wait for the child process to connect.
func CreateListener(socketPath string) (net.Listener, error) {
	if err := os.MkdirAll(filepath.Dir(socketPath), 0755); err != nil {
		return nil, fmt.Errorf("create socket dir: %w", err)
	}
	// Remove stale socket if present.
	os.Remove(socketPath)

	ln, err := net.Listen("unix", socketPath)
	if err != nil {
		return nil, fmt.Errorf("listen on %s: %w", socketPath, err)
	}
	return ln, nil
}

// WaitForConnection accepts a single connection with a timeout.
func WaitForConnection(ln net.Listener, timeout time.Duration) (net.Conn, error) {
	type result struct {
		conn net.Conn
		err  error
	}
	ch := make(chan result, 1)
	go func() {
		conn, err := ln.Accept()
		ch <- result{conn, err}
	}()

	select {
	case r := <-ch:
		return r.conn, r.err
	case <-time.After(timeout):
		return nil, fmt.Errorf("timeout waiting for pane connection after %s", timeout)
	}
}

// ConnectSocket connects to a Unix domain socket at the given path.
// Child side calls this to establish IPC with the parent.
func ConnectSocket(socketPath string, timeout time.Duration) (net.Conn, error) {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		conn, err := net.Dial("unix", socketPath)
		if err == nil {
			return conn, nil
		}
		time.Sleep(50 * time.Millisecond)
	}
	return nil, fmt.Errorf("timeout connecting to socket %s after %s", socketPath, timeout)
}

// SocketPath returns the standard socket path for a given pane ID.
func SocketPath(socketDir string, paneID string) string {
	return filepath.Join(socketDir, paneID+".sock")
}

// CleanupSocket removes the socket file and its directory if empty.
func CleanupSocket(socketPath string) {
	os.Remove(socketPath)
	dir := filepath.Dir(socketPath)
	// Try to remove dir; ignore error if not empty.
	os.Remove(dir)
}
