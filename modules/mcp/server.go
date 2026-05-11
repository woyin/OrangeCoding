package mcp

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"sync"
)

// ToolHandler is the function signature for handling a tool invocation.
type ToolHandler func(args json.RawMessage) (json.RawMessage, error)

type registeredTool struct {
	Info    ToolInfo
	Handler ToolHandler
}

// McpServer is an MCP protocol server that handles requests over a Transport.
type McpServer struct {
	transport Transport
	tools     map[string]registeredTool
	mu        sync.RWMutex
	serverInfo ServerInfo
}

// NewMcpServer creates a new MCP server using the given transport.
func NewMcpServer(transport Transport) *McpServer {
	return &McpServer{
		transport: transport,
		tools:     make(map[string]registeredTool),
		serverInfo: ServerInfo{
			Name:    "orange-mcp-server",
			Version: "0.1.0",
		},
	}
}

// SetServerInfo configures the server identity returned during initialization.
func (s *McpServer) SetServerInfo(name, version string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.serverInfo = ServerInfo{Name: name, Version: version}
}

// RegisterTool registers a tool with its handler.
func (s *McpServer) RegisterTool(tool ToolInfo, handler ToolHandler) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.tools[tool.Name] = registeredTool{
		Info:    tool,
		Handler: handler,
	}
}

// Serve starts the server's main loop. It reads requests from the transport
// and dispatches them to the appropriate handler. It blocks until the context
// is cancelled or the transport returns an error.
func (s *McpServer) Serve(ctx context.Context) error {
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		raw, err := s.transport.Receive()
		if err != nil {
			select {
			case <-ctx.Done():
				return ctx.Err()
			default:
				return fmt.Errorf("receive: %w", err)
			}
		}

		// Try to determine if this is a request (has "id") or notification.
		var partial struct {
			ID     json.RawMessage `json:"id"`
			Method string          `json:"method"`
		}
		if err := json.Unmarshal(raw, &partial); err != nil {
			s.sendError(nil, ErrorCodeParseError, "parse error")
			continue
		}

		// If no ID, it's a notification — ignore for now.
		if len(partial.ID) == 0 {
			continue
		}

		// Parse full request.
		var req Request
		if err := json.Unmarshal(raw, &req); err != nil {
			s.sendError(partial.ID, ErrorCodeInvalidRequest, "invalid request")
			continue
		}

		s.handleRequest(req)
	}
}

// handleRequest dispatches a request to the appropriate handler.
func (s *McpServer) handleRequest(req Request) {
	switch req.Method {
	case "initialize":
		s.handleInitialize(req)
	case "tools/list":
		s.handleListTools(req)
	case "tools/call":
		s.handleCallTool(req)
	default:
		s.sendError(req.ID, ErrorCodeMethodNotFound, fmt.Sprintf("method not found: %s", req.Method))
	}
}

func (s *McpServer) handleInitialize(req Request) {
	s.mu.RLock()
	info := s.serverInfo
	s.mu.RUnlock()

	result, _ := json.Marshal(map[string]interface{}{
		"capabilities": map[string]interface{}{
			"tools": map[string]interface{}{},
		},
		"serverInfo": info,
	})

	s.sendResponse(req.ID, result)

	// Send "notifications/initialized" as per MCP spec.
	notif := Notification{
		JsonRPC: JSONRPCVersion,
		Method:  "notifications/initialized",
	}
	data, _ := json.Marshal(notif)
	if err := s.transport.Send(data); err != nil {
		log.Printf("mcp: failed to send initialized notification: %v", err)
	}
}

func (s *McpServer) handleListTools(req Request) {
	s.mu.RLock()
	tools := make([]ToolInfo, 0, len(s.tools))
	for _, rt := range s.tools {
		tools = append(tools, rt.Info)
	}
	s.mu.RUnlock()

	result, _ := json.Marshal(map[string]interface{}{
		"tools": tools,
	})
	s.sendResponse(req.ID, result)
}

func (s *McpServer) handleCallTool(req Request) {
	var params struct {
		Name      string          `json:"name"`
		Arguments json.RawMessage `json:"arguments"`
	}
	if err := json.Unmarshal(req.Params, &params); err != nil {
		s.sendError(req.ID, ErrorCodeInvalidParams, "invalid params")
		return
	}

	s.mu.RLock()
	rt, ok := s.tools[params.Name]
	s.mu.RUnlock()

	if !ok {
		s.sendError(req.ID, ErrorCodeMethodNotFound, fmt.Sprintf("tool not found: %s", params.Name))
		return
	}

	result, err := rt.Handler(params.Arguments)
	if err != nil {
		s.sendError(req.ID, ErrorCodeInternalError, err.Error())
		return
	}

	s.sendResponse(req.ID, result)
}

func (s *McpServer) sendResponse(id json.RawMessage, result json.RawMessage) {
	resp := Response{
		JsonRPC: JSONRPCVersion,
		ID:      id,
		Result:  result,
	}
	data, err := json.Marshal(resp)
	if err != nil {
		log.Printf("mcp: failed to marshal response: %v", err)
		return
	}
	if err := s.transport.Send(data); err != nil {
		log.Printf("mcp: failed to send response: %v", err)
	}
}

func (s *McpServer) sendError(id json.RawMessage, code int, message string) {
	resp := Response{
		JsonRPC: JSONRPCVersion,
		ID:      id,
		Error: &ResponseError{
			Code:    code,
			Message: message,
		},
	}
	data, err := json.Marshal(resp)
	if err != nil {
		log.Printf("mcp: failed to marshal error response: %v", err)
		return
	}
	if err := s.transport.Send(data); err != nil {
		log.Printf("mcp: failed to send error response: %v", err)
	}
}
