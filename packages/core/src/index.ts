// Types
export {
  AgentId,
  SessionId,
  ToolName,
  TokenUsage,
  AgentRole,
  parseAgentRole,
  AgentStatus,
  parseAgentStatus,
  isTerminalStatus,
  isActiveStatus,
  Role,
  parseRole,
  supportsTool,
} from "./types.js";
export type { AgentCapability, AgentRole as AgentRoleType, AgentStatus as AgentStatusType, Role as RoleType, TokenUsageJSON } from "./types.js";

// Error
export {
  OrangeError,
  ErrorKind,
  wrapError,
  newConfigError,
  newIOError,
  newNetworkError,
  newProviderError,
  newProtocolError,
  newSerializationError,
  newAuthError,
  newInternalError,
  newToolError,
  newAgentError,
} from "./error.js";
export type { ErrorKind as ErrorKindType } from "./error.js";

// Message
export {
  Message,
  ToolResult,
  Conversation,
  newSystemMessage,
  newUserMessage,
  newAssistantMessage,
  newAssistantMessageWithToolCalls,
  newToolResultMessage,
  newToolResultSuccess,
  newToolResultError,
} from "./message.js";
export type { ToolCall, MessageJSON, ToolResultJSON } from "./message.js";

// Event
export {
  BaseEvent,
  EventBus,
  StartedEvent,
  CompletedEvent,
  MessageReceivedEvent,
  ToolCallRequestedEvent,
  ToolCallCompletedEvent,
  TokenUsageUpdatedEvent,
  StreamChunkEvent,
  ErrorEvent,
  GoalPhaseChangedEvent,
  GoalTaskCompletedEvent,
  GoalCycleCompleteEvent,
  GuardrailDecisionEvent,
} from "./event.js";
export type { AgentEvent, EventHandler, BaseEventJSON } from "./event.js";

// Task
export {
  newTaskId,
  TaskType,
  TaskStatus,
  isTaskError,
} from "./task.js";
export type { TaskId, TaskType as TaskTypeType, TaskStatus as TaskStatusType, Task, TaskResult } from "./task.js";
