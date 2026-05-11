package controlprotocol

// ClientCommand is the interface for commands from web UI to server.
type ClientCommand interface {
	CommandType() string
}

// SendTaskCommand instructs the server to send a task to an agent session.
type SendTaskCommand struct {
	SessionID string `json:"session_id"`
	Task      string `json:"task"`
}

// CommandType returns the command type identifier.
func (c *SendTaskCommand) CommandType() string { return "send_task" }

// ApproveCommand responds to an approval request.
type ApproveCommand struct {
	RequestID string `json:"request_id"`
	Approved  bool   `json:"approved"`
}

// CommandType returns the command type identifier.
func (c *ApproveCommand) CommandType() string { return "approve" }

// CancelCommand requests cancellation of a session.
type CancelCommand struct {
	SessionID string `json:"session_id"`
}

// CommandType returns the command type identifier.
func (c *CancelCommand) CommandType() string { return "cancel" }

// ServerEvent is the interface for events from server to web UI.
type ServerEvent interface {
	EventType() string
}

// TaskUpdateEvent reports a status change for a task.
type TaskUpdateEvent struct {
	SessionID string `json:"session_id"`
	Status    string `json:"status"`
	Message   string `json:"message"`
}

// EventType returns the event type identifier.
func (e *TaskUpdateEvent) EventType() string { return "task_update" }

// ToolCallEvent reports a tool invocation and its result.
type ToolCallEvent struct {
	SessionID string `json:"session_id"`
	ToolName  string `json:"tool_name"`
	Input     string `json:"input"`
	Output    string `json:"output"`
	IsError   bool   `json:"is_error"`
}

// EventType returns the event type identifier.
func (e *ToolCallEvent) EventType() string { return "tool_call" }

// ApprovalRequestEvent asks the user to approve a tool call.
type ApprovalRequestEvent struct {
	RequestID string `json:"request_id"`
	ToolName  string `json:"tool_name"`
	Input     string `json:"input"`
	Message   string `json:"message"`
}

// EventType returns the event type identifier.
func (e *ApprovalRequestEvent) EventType() string { return "approval_request" }

// ErrorEvent reports an error for a session.
type ErrorEvent struct {
	SessionID string `json:"session_id"`
	Error     string `json:"error"`
}

// EventType returns the event type identifier.
func (e *ErrorEvent) EventType() string { return "error" }
