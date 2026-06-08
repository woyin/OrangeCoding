// ---------------------------------------------------------------------------
// Wire types for AI provider communication
// ---------------------------------------------------------------------------

/**
 * Represents a single message in a conversation.
 */
export interface ChatMessage {
  role: string;
  content?: string;
  name?: string;
  tool_call_id?: string;
  tool_calls?: ToolCall[];
}

/** Creates a system message. */
export function systemMsg(content: string): ChatMessage {
  return { role: "system", content };
}

/** Creates a user message. */
export function userMsg(content: string): ChatMessage {
  return { role: "user", content };
}

/** Creates an assistant message. */
export function assistantMsg(content: string): ChatMessage {
  return { role: "assistant", content };
}

/** Creates a tool result message. */
export function toolResultMsg(toolCallID: string, content: string): ChatMessage {
  return { role: "tool", tool_call_id: toolCallID, content };
}

/** Creates an assistant message with tool calls. */
export function assistantMsgWithTools(toolCalls: ToolCall[]): ChatMessage {
  return { role: "assistant", tool_calls: toolCalls };
}

// ---------------------------------------------------------------------------
// Tool types
// ---------------------------------------------------------------------------

/**
 * Represents a tool call from the AI model.
 */
export interface ToolCall {
  id: string;
  type: string;
  function: FunctionCall;
}

/**
 * Represents the function name and arguments within a tool call.
 */
export interface FunctionCall {
  name: string;
  arguments: string; // raw JSON string
}

/**
 * Defines a tool that can be offered to the AI model.
 */
export interface ToolDefinition {
  type: string;
  function: FunctionDefinition;
}

/**
 * Describes a function's signature for tool use.
 */
export interface FunctionDefinition {
  name: string;
  description: string;
  parameters: ToolParameter;
}

/**
 * Describes the JSON schema parameters for a tool.
 */
export interface ToolParameter {
  type: string;
  properties: Record<string, unknown>;
  required?: string[];
}

// ---------------------------------------------------------------------------
// Request/Response types
// ---------------------------------------------------------------------------

/**
 * Configures a chat completion request.
 */
export interface ChatOptions {
  model: string;
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  stop_sequences?: string[];
  reasoning_effort?: string;
  reasoning_budget_tokens?: number;
}

/**
 * Represents the full response from an AI provider.
 */
export interface AiResponse {
  content: string;
  tool_calls: ToolCall[];
  usage: AiTokenUsage;
  model: string;
  finish_reason: string;
}

/**
 * Token usage for a single AI call (plain interface, compatible with core TokenUsage).
 */
export interface AiTokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

// ---------------------------------------------------------------------------
// Streaming types
// ---------------------------------------------------------------------------

/**
 * Represents a single event in an SSE stream.
 */
export interface StreamEvent {
  type: "content_delta" | "tool_call_delta" | "usage" | "done";
  content: string;
  tool_call_id: string;
  tool_call_name: string;
  arguments: string;
  usage: AiTokenUsage | null;
}
