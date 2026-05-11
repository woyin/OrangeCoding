package mcp

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"strings"
	"sync"
	"testing"
	"time"
)

// --- JSON-RPC tests ---

func TestJSONRPCRequestMarshal(t *testing.T) {
	id, _ := json.Marshal(1)
	req := Request{
		JsonRPC: "2.0",
		ID:      id,
		Method:  "tools/list",
		Params:  json.RawMessage(`{}`),
	}

	data, err := json.Marshal(req)
	if err != nil {
		t.Fatalf("marshal request: %v", err)
	}

	// Verify key fields present
	if !bytes.Contains(data, []byte(`"jsonrpc":"2.0"`)) {
		t.Errorf("missing jsonrpc field in %s", data)
	}
	if !bytes.Contains(data, []byte(`"method":"tools/list"`)) {
		t.Errorf("missing method field in %s", data)
	}
	if !bytes.Contains(data, []byte(`"id":1`)) {
		t.Errorf("missing id field in %s", data)
	}

	// Round-trip
	var got Request
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if got.Method != req.Method {
		t.Errorf("Method = %q, want %q", got.Method, req.Method)
	}
	if got.JsonRPC != "2.0" {
		t.Errorf("JsonRPC = %q, want %q", got.JsonRPC, "2.0")
	}
}

func TestJSONRPCResponseUnmarshal(t *testing.T) {
	raw := `{"jsonrpc":"2.0","id":42,"result":{"name":"test"}}`
	var resp Response
	if err := json.Unmarshal([]byte(raw), &resp); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	// Verify result
	var result map[string]string
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		t.Fatalf("unmarshal result: %v", err)
	}
	if result["name"] != "test" {
		t.Errorf("result name = %q, want %q", result["name"], "test")
	}
	if resp.Error != nil {
		t.Errorf("Error should be nil for success response, got %+v", resp.Error)
	}
}

func TestJSONRPCResponseError(t *testing.T) {
	raw := `{"jsonrpc":"2.0","id":7,"error":{"code":-32600,"message":"Invalid Request"}}`
	var resp Response
	if err := json.Unmarshal([]byte(raw), &resp); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if resp.Error == nil {
		t.Fatal("Error should not be nil")
	}
	if resp.Error.Code != -32600 {
		t.Errorf("Code = %d, want -32600", resp.Error.Code)
	}
	if resp.Error.Message != "Invalid Request" {
		t.Errorf("Message = %q, want %q", resp.Error.Message, "Invalid Request")
	}
	if resp.Result != nil {
		t.Errorf("Result should be nil for error response, got %s", resp.Result)
	}
}

// --- StdioTransport tests ---

func TestStdioTransport(t *testing.T) {
	pr1, pw1 := io.Pipe() // client reads from pr1, server writes to pw1
	pr2, pw2 := io.Pipe() // server reads from pr2, client writes to pw2

	clientTransport := NewStdioTransport(pr2, pw1) // reads pr2 (server->client), writes pw1
	serverTransport := NewStdioTransport(pr1, pw2) // reads pr1 (client->server), writes pw2

	msg := []byte(`{"jsonrpc":"2.0","method":"ping"}`)

	var received []byte
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		var err error
		received, err = serverTransport.Receive()
		if err != nil {
			t.Errorf("server receive: %v", err)
		}
	}()

	if err := clientTransport.Send(msg); err != nil {
		t.Fatalf("client send: %v", err)
	}
	wg.Wait()

	// Normalize whitespace for comparison
	got := strings.TrimSpace(string(received))
	want := strings.TrimSpace(string(msg))
	if got != want {
		t.Errorf("received = %q, want %q", got, want)
	}

	clientTransport.Close()
	serverTransport.Close()
}

// --- McpClient tests ---

func TestMcpClientListTools(t *testing.T) {
	// Set up bidirectional pipes
	pr1, pw1 := io.Pipe() // server reads from pr1
	pr2, pw2 := io.Pipe() // client reads from pr2

	clientTransport := NewStdioTransport(pr2, pw1)
	serverWriter := bufio.NewWriter(pw2)

	client := NewMcpClient(clientTransport)

	// Mock server: read request, send response
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		scanner := bufio.NewScanner(pr1)
		if !scanner.Scan() {
			t.Errorf("scanner failed: %v", scanner.Err())
			return
		}
		line := scanner.Bytes()

		var req Request
		if err := json.Unmarshal(line, &req); err != nil {
			t.Errorf("unmarshal request: %v", err)
			return
		}

		// Build tools/list response
		tools := []ToolInfo{
			{Name: "echo", Description: "echo tool", InputSchema: json.RawMessage(`{"type":"object"}`)},
		}
		resultData, _ := json.Marshal(map[string]interface{}{"tools": tools})
		id, _ := json.Marshal(1)
		resp := Response{
			JsonRPC: "2.0",
			ID:      id,
			Result:  resultData,
		}
		respData, _ := json.Marshal(resp)
		serverWriter.Write(respData)
		serverWriter.WriteByte('\n')
		serverWriter.Flush()
	}()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	tools, err := client.ListTools(ctx)
	if err != nil {
		t.Fatalf("ListTools: %v", err)
	}

	if len(tools) != 1 {
		t.Fatalf("len(tools) = %d, want 1", len(tools))
	}
	if tools[0].Name != "echo" {
		t.Errorf("tool name = %q, want %q", tools[0].Name, "echo")
	}
	if tools[0].Description != "echo tool" {
		t.Errorf("tool description = %q, want %q", tools[0].Description, "echo tool")
	}

	wg.Wait()
	clientTransport.Close()
	pw2.Close()
}

func TestMcpClientInitialize(t *testing.T) {
	pr1, pw1 := io.Pipe()
	pr2, pw2 := io.Pipe()

	clientTransport := NewStdioTransport(pr2, pw1)
	serverWriter := bufio.NewWriter(pw2)

	client := NewMcpClient(clientTransport)

	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		scanner := bufio.NewScanner(pr1)
		if !scanner.Scan() {
			t.Errorf("scanner: %v", scanner.Err())
			return
		}
		var req Request
		json.Unmarshal(scanner.Bytes(), &req)

		// Respond to initialize
		capabilities := map[string]interface{}{}
		serverInfo := map[string]interface{}{"name": "test-server", "version": "1.0.0"}
		result, _ := json.Marshal(map[string]interface{}{
			"capabilities":  capabilities,
			"serverInfo":    serverInfo,
		})
		id, _ := json.Marshal(1)
		resp := Response{JsonRPC: "2.0", ID: id, Result: result}
		data, _ := json.Marshal(resp)
		serverWriter.Write(data)
		serverWriter.WriteByte('\n')
		serverWriter.Flush()

		// Read the "initialized" notification
		if !scanner.Scan() {
			return
		}
	}()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	info, err := client.Initialize(ctx)
	if err != nil {
		t.Fatalf("Initialize: %v", err)
	}
	if info.Name != "test-server" {
		t.Errorf("server name = %q, want %q", info.Name, "test-server")
	}
	if info.Version != "1.0.0" {
		t.Errorf("server version = %q, want %q", info.Version, "1.0.0")
	}

	wg.Wait()
	clientTransport.Close()
	pw2.Close()
}

func TestMcpClientCallTool(t *testing.T) {
	pr1, pw1 := io.Pipe()
	pr2, pw2 := io.Pipe()

	clientTransport := NewStdioTransport(pr2, pw1)
	serverWriter := bufio.NewWriter(pw2)

	client := NewMcpClient(clientTransport)

	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		scanner := bufio.NewScanner(pr1)
		if !scanner.Scan() {
			t.Errorf("scanner: %v", scanner.Err())
			return
		}
		var req Request
		json.Unmarshal(scanner.Bytes(), &req)

		// Parse params to extract the tool name
		var params struct {
			Name string          `json:"name"`
			Args json.RawMessage `json:"arguments"`
		}
		json.Unmarshal(req.Params, &params)

		if params.Name != "add" {
			t.Errorf("tool name = %q, want %q", params.Name, "add")
		}

		// Return result
		content := []map[string]interface{}{
			{"type": "text", "text": "42"},
		}
		result, _ := json.Marshal(map[string]interface{}{"content": content})
		resp := Response{JsonRPC: "2.0", ID: req.ID, Result: result}
		data, _ := json.Marshal(resp)
		serverWriter.Write(data)
		serverWriter.WriteByte('\n')
		serverWriter.Flush()
	}()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	result, err := client.CallTool(ctx, "add", json.RawMessage(`{"a":1,"b":2}`))
	if err != nil {
		t.Fatalf("CallTool: %v", err)
	}

	var res map[string]interface{}
	if err := json.Unmarshal(result, &res); err != nil {
		t.Fatalf("unmarshal result: %v", err)
	}
	content, ok := res["content"].([]interface{})
	if !ok || len(content) == 0 {
		t.Fatal("missing content in result")
	}

	wg.Wait()
	clientTransport.Close()
	pw2.Close()
}

// --- McpServer tests ---

func TestMcpServerRegisterAndCall(t *testing.T) {
	pr1, pw1 := io.Pipe() // server reads from pr1
	pr2, pw2 := io.Pipe() // client reads from pr2

	serverTransport := NewStdioTransport(pr1, pw2)
	server := NewMcpServer(serverTransport)

	// Register an "add" tool
	addTool := ToolInfo{
		Name:        "add",
		Description: "adds two numbers",
		InputSchema: json.RawMessage(`{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}}}`),
	}
	server.RegisterTool(addTool, func(args json.RawMessage) (json.RawMessage, error) {
		var params struct {
			A float64 `json:"a"`
			B float64 `json:"b"`
		}
		if err := json.Unmarshal(args, &params); err != nil {
			return nil, err
		}
		result := fmt.Sprintf("%.0f", params.A+params.B)
		return json.RawMessage(`{"content":[{"type":"text","text":"` + result + `"}]}`), nil
	})

	// Start server in background
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	serverDone := make(chan struct{})
	go func() {
		defer close(serverDone)
		server.Serve(ctx)
	}()

	// Client sends a tools/call request
	clientWriter := bufio.NewWriter(pw1)
	argsRaw := json.RawMessage(`{"a":3,"b":4}`)
	params, _ := json.Marshal(struct {
		Name      string          `json:"name"`
		Arguments json.RawMessage `json:"arguments"`
	}{
		Name:      "add",
		Arguments: argsRaw,
	})
	id, _ := json.Marshal(99)
	req := Request{
		JsonRPC: "2.0",
		ID:      id,
		Method:  "tools/call",
		Params:  params,
	}
	reqData, _ := json.Marshal(req)
	clientWriter.Write(reqData)
	clientWriter.WriteByte('\n')
	clientWriter.Flush()

	// Read response
	scanner := bufio.NewScanner(pr2)
	if !scanner.Scan() {
		t.Fatal("no response from server")
	}

	var resp Response
	if err := json.Unmarshal(scanner.Bytes(), &resp); err != nil {
		t.Fatalf("unmarshal response: %v", err)
	}

	if resp.Error != nil {
		t.Fatalf("server error: %s", resp.Error.Message)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		t.Fatalf("unmarshal result: %v", err)
	}
	content, ok := result["content"].([]interface{})
	if !ok || len(content) == 0 {
		t.Fatal("missing content")
	}
	entry := content[0].(map[string]interface{})
	if entry["text"] != "7" {
		t.Errorf("result text = %v, want 7", entry["text"])
	}

	cancel()
	<-serverDone

	serverTransport.Close()
	pw1.Close()
}

func TestMcpServerInitialize(t *testing.T) {
	pr1, pw1 := io.Pipe()
	pr2, pw2 := io.Pipe()

	serverTransport := NewStdioTransport(pr1, pw2)
	server := NewMcpServer(serverTransport)
	server.SetServerInfo("test-server", "0.1.0")

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	go server.Serve(ctx)

	// Send initialize request
	clientWriter := bufio.NewWriter(pw1)
	id, _ := json.Marshal(1)
	req := Request{
		JsonRPC: "2.0",
		ID:      id,
		Method:  "initialize",
		Params:  json.RawMessage(`{"capabilities":{},"clientInfo":{"name":"test-client","version":"1.0"}}`),
	}
	data, _ := json.Marshal(req)
	clientWriter.Write(data)
	clientWriter.WriteByte('\n')
	clientWriter.Flush()

	scanner := bufio.NewScanner(pr2)
	if !scanner.Scan() {
		t.Fatal("no response")
	}

	var resp Response
	if err := json.Unmarshal(scanner.Bytes(), &resp); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if resp.Error != nil {
		t.Fatalf("error: %s", resp.Error.Message)
	}

	var result map[string]interface{}
	json.Unmarshal(resp.Result, &result)
	si, ok := result["serverInfo"].(map[string]interface{})
	if !ok {
		t.Fatal("missing serverInfo")
	}
	if si["name"] != "test-server" {
		t.Errorf("name = %v, want test-server", si["name"])
	}

	// Read the "initialized" notification that server should echo back
	// (optional — server may or may not send this depending on implementation)

	cancel()
	serverTransport.Close()
	pw1.Close()
}

func TestMcpServerListTools(t *testing.T) {
	pr1, pw1 := io.Pipe()
	pr2, pw2 := io.Pipe()

	serverTransport := NewStdioTransport(pr1, pw2)
	server := NewMcpServer(serverTransport)

	server.RegisterTool(ToolInfo{
		Name:        "greet",
		Description: "says hello",
		InputSchema: json.RawMessage(`{"type":"object"}`),
	}, func(args json.RawMessage) (json.RawMessage, error) {
		return json.RawMessage(`{"content":[{"type":"text","text":"hello"}]}`), nil
	})

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	go server.Serve(ctx)

	// Send tools/list
	clientWriter := bufio.NewWriter(pw1)
	id, _ := json.Marshal(2)
	req := Request{
		JsonRPC: "2.0",
		ID:      id,
		Method:  "tools/list",
	}
	data, _ := json.Marshal(req)
	clientWriter.Write(data)
	clientWriter.WriteByte('\n')
	clientWriter.Flush()

	scanner := bufio.NewScanner(pr2)
	if !scanner.Scan() {
		t.Fatal("no response")
	}

	var resp Response
	if err := json.Unmarshal(scanner.Bytes(), &resp); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if resp.Error != nil {
		t.Fatalf("error: %s", resp.Error.Message)
	}

	var result struct {
		Tools []ToolInfo `json:"tools"`
	}
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		t.Fatalf("unmarshal tools: %v", err)
	}
	if len(result.Tools) != 1 {
		t.Fatalf("tools count = %d, want 1", len(result.Tools))
	}
	if result.Tools[0].Name != "greet" {
		t.Errorf("tool name = %q, want %q", result.Tools[0].Name, "greet")
	}

	cancel()
	serverTransport.Close()
	pw1.Close()
}
