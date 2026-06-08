/** PaneState represents the lifecycle state of a managed pane. */
export enum PaneState {
  Created = "created",
  Running = "running",
  Exited = "exited",
  Error = "error",
}

/** PaneInfo holds metadata about a managed terminal pane. */
export interface PaneInfo {
  /** Backend-specific pane identifier */
  id: string;
  /** Human-readable agent name */
  name: string;
  /** OS process ID of the pane's shell (if available) */
  pid?: number;
  state: PaneState;
  createdAt: Date;
  /** "zellij" or "tmux" */
  backend: string;
}
