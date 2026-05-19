package mcp

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"sync/atomic"
)

// ServerInfo describes the MCP server identity returned by Initialize.
type ServerInfo struct {
	Name    string `json:"name"`
	Version string `json:"version"`
}

// ToolInfo describes a tool exposed by an MCP server.
type ToolInfo struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	InputSchema json.RawMessage `json:"inputSchema"`
}

// McpClient is an MCP protocol client that communicates over a Transport.
type McpClient struct {
	transport Transport
	idCounter atomic.Int64
	pending   sync.Map // map[json.RawMessage]chan Response
	closed    chan struct{}
	closeOnce sync.Once
}

// NewMcpClient creates a new MCP client using the given transport.
func NewMcpClient(transport Transport) *McpClient {
	return &McpClient{
		transport: transport,
		closed:    make(chan struct{}),
	}
}

// Close shuts down the client and unblocks any pending receives.
func (c *McpClient) Close() error {
	c.closeOnce.Do(func() {
		close(c.closed)
	})
	return c.transport.Close()
}

// nextID generates the next request ID as JSON.
func (c *McpClient) nextID() json.RawMessage {
	n := c.idCounter.Add(1)
	id, _ := json.Marshal(n)
	return id
}

// sendRequest sends a JSON-RPC request and waits for the matching response.
func (c *McpClient) sendRequest(ctx context.Context, method string, params interface{}) (Response, error) {
	id := c.nextID()

	var paramsRaw json.RawMessage
	if params != nil {
		p, err := json.Marshal(params)
		if err != nil {
			return Response{}, fmt.Errorf("marshal params: %w", err)
		}
		paramsRaw = p
	}

	req := Request{
		JsonRPC: JSONRPCVersion,
		ID:      id,
		Method:  method,
		Params:  paramsRaw,
	}

	data, err := json.Marshal(req)
	if err != nil {
		return Response{}, fmt.Errorf("marshal request: %w", err)
	}

	if err := c.transport.Send(data); err != nil {
		return Response{}, fmt.Errorf("send: %w", err)
	}

	// Read the response.
	type result struct {
		resp Response
		err  error
	}
	ch := make(chan result, 1)

	go func() {
		for {
			raw, err := c.transport.Receive()
			if err != nil {
				ch <- result{err: fmt.Errorf("receive: %w", err)}
				return
			}

			var resp Response
			if err := json.Unmarshal(raw, &resp); err != nil {
				ch <- result{err: fmt.Errorf("unmarshal response: %w", err)}
				return
			}

			// Check if this is a notification (no ID) — skip it.
			if len(resp.ID) == 0 {
				continue
			}

			ch <- result{resp: resp}
			return
		}
	}()

	select {
	case <-ctx.Done():
		return Response{}, ctx.Err()
	case <-c.closed:
		return Response{}, fmt.Errorf("client closed")
	case r := <-ch:
		return r.resp, r.err
	}
}

// Initialize sends the MCP "initialize" request and returns server info.
func (c *McpClient) Initialize(ctx context.Context) (ServerInfo, error) {
	params := map[string]interface{}{
		"capabilities": map[string]interface{}{},
		"clientInfo": map[string]interface{}{
			"name":    "orange-mcp-client",
			"version": "0.1.0",
		},
	}

	resp, err := c.sendRequest(ctx, "initialize", params)
	if err != nil {
		return ServerInfo{}, err
	}

	if resp.Error != nil {
		return ServerInfo{}, fmt.Errorf("initialize error [%d]: %s", resp.Error.Code, resp.Error.Message)
	}

	var result struct {
		ServerInfo ServerInfo `json:"serverInfo"`
	}
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return ServerInfo{}, fmt.Errorf("parse server info: %w", err)
	}

	// Send "notifications/initialized" as per MCP spec.
	notif := Notification{
		JsonRPC: JSONRPCVersion,
		Method:  "notifications/initialized",
	}
	data, _ := json.Marshal(notif)
	_ = c.transport.Send(data)

	return result.ServerInfo, nil
}

// ListTools sends the MCP "tools/list" request and returns the available tools.
func (c *McpClient) ListTools(ctx context.Context) ([]ToolInfo, error) {
	resp, err := c.sendRequest(ctx, "tools/list", nil)
	if err != nil {
		return nil, err
	}

	if resp.Error != nil {
		return nil, fmt.Errorf("tools/list error [%d]: %s", resp.Error.Code, resp.Error.Message)
	}

	var result struct {
		Tools []ToolInfo `json:"tools"`
	}
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return nil, fmt.Errorf("parse tools: %w", err)
	}

	return result.Tools, nil
}

// CallTool sends the MCP "tools/call" request and returns the raw result.
func (c *McpClient) CallTool(ctx context.Context, name string, args json.RawMessage) (json.RawMessage, error) {
	params := map[string]interface{}{
		"name":      name,
		"arguments": args,
	}

	resp, err := c.sendRequest(ctx, "tools/call", params)
	if err != nil {
		return nil, err
	}

	if resp.Error != nil {
		return nil, fmt.Errorf("tools/call error [%d]: %s", resp.Error.Code, resp.Error.Message)
	}

	return resp.Result, nil
}
