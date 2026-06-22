/** Pane lifecycle states. */
// Pane types
export { PaneState } from "./pane.js";
export type { PaneInfo } from "./pane.js";

/** Terminal multiplexer backend abstraction. */
// Backend
export { detectBackend, newBackendFromConfig, runCommand } from "./backend.js";
export type { Backend } from "./backend.js";

/** Multiplexer configuration types and defaults. */
// Config
export { defaultMultiplexerConfig, normalizeConfig } from "./config.js";
export type { MultiplexerConfig } from "./config.js";

/** Backend implementations for tmux and zellij terminal multiplexers. */
// Tmux / Zellij backends
export { TmuxBackend } from "./tmux.js";
export { ZellijBackend } from "./zellij.js";

/** IPC transport for inter-pane communication via Unix domain sockets. */
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

/** High-level pane lifecycle manager. */
// Manager
export { PaneManager } from "./manager.js";
export type { ManagedPane } from "./manager.js";

/** Adapter for running agents inside multiplexer panes. */
// Agent adapter
export { MultiplexerAgentAdapter } from "./agent-adapter.js";
