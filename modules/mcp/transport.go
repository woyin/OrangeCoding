package mcp

import (
	"bufio"
	"io"
)

// Transport abstracts the underlying communication channel for JSON-RPC messages.
type Transport interface {
	// Send writes a JSON-RPC message (as bytes) to the transport.
	Send(data []byte) error
	// Receive reads the next JSON-RPC message from the transport.
	Receive() ([]byte, error)
	// Close releases any resources held by the transport.
	Close() error
}

// StdioTransport implements Transport over stdin/stdout-style I/O.
// Messages are line-delimited JSON: each message is terminated by a newline.
type StdioTransport struct {
	scanner *bufio.Scanner
	writer  *bufio.Writer
	closer  io.Closer
}

// NewStdioTransport creates a StdioTransport that reads from r and writes to w.
func NewStdioTransport(r io.Reader, w io.Writer) *StdioTransport {
	// Use a larger buffer for the scanner to handle bigger messages.
	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	var closer io.Closer
	if c, ok := w.(io.Closer); ok {
		closer = c
	}
	return &StdioTransport{
		scanner: scanner,
		writer:  bufio.NewWriter(w),
		closer:  closer,
	}
}

// Send writes data as a single line to the underlying writer.
func (t *StdioTransport) Send(data []byte) error {
	if _, err := t.writer.Write(data); err != nil {
		return err
	}
	if err := t.writer.WriteByte('\n'); err != nil {
		return err
	}
	return t.writer.Flush()
}

// Receive reads the next line from the underlying reader.
func (t *StdioTransport) Receive() ([]byte, error) {
	if !t.scanner.Scan() {
		if err := t.scanner.Err(); err != nil {
			return nil, err
		}
		return nil, io.EOF
	}
	// Return a copy since scanner reuses its buffer.
	data := make([]byte, len(t.scanner.Bytes()))
	copy(data, t.scanner.Bytes())
	return data, nil
}

// Close releases the underlying writer (if it implements io.Closer).
func (t *StdioTransport) Close() error {
	if t.closer != nil {
		return t.closer.Close()
	}
	return nil
}
