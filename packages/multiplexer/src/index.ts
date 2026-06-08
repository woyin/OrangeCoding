// Pane types
export { PaneState } from "./pane.js";
export type { PaneInfo } from "./pane.js";

// Backend
export { detectBackend, newBackendFromConfig, runCommand } from "./backend.js";
export type { Backend } from "./backend.js";

// Config
export { defaultMultiplexerConfig, normalizeConfig } from "./config.js";
export type { MultiplexerConfig } from "./config.js";

// Tmux / Zellij backends
export { TmuxBackend } from "./tmux.js";
export { ZellijBackend } from "./zellij.js";

// Transport (IPC)
export {
  IPCMessageType,
  SocketTransport,
  createListener,
  waitForConnection,
  connectSocket,
  socketPath,
  cleanupSocket,
} from "./transport.js";
export type {
  IPCMessage,
  IPCMessageType as IPCMessageTypeType,
  TaskPayload,
  ResultPayload,
  EventPayload,
} from "./transport.js";

// Manager
export { PaneManager } from "./manager.js";
export type { ManagedPane } from "./manager.js";

// Agent adapter
export { MultiplexerAgentAdapter } from "./agent-adapter.js";
