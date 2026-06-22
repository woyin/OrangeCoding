import type { Backend } from "./backend.js";
import type { MultiplexerConfig } from "./config.js";
import { normalizeConfig } from "./config.js";
import type { PaneInfo } from "./pane.js";
import {
  SocketTransport,
  type IPCMessage,
  type TaskPayload,
  IPCMessageType,
  createListener,
  waitForConnection,
  socketPath,
  cleanupSocket,
} from "./transport.js";
import { newIOError } from "@orangecoding/core";

/**
 * ManagedPane tracks a single pane's state and IPC channel.
 */
export interface ManagedPane {
  info: PaneInfo;
  transport: SocketTransport;
  socketPath: string;
  abortController: AbortController;
}

/**
 * PaneManager coordinates pane creation, IPC setup, and cleanup.
 */
/**
 * PaneManager coordinates terminal pane lifecycle and IPC communication.
 *
 * Manages the creation, tracking, and cleanup of terminal panes:
 * 1. Creates Unix domain socket listeners for IPC
 * 2. Spawns panes via the terminal multiplexer backend (tmux/zellij)
 * 3. Waits for child process connections
 * 4. Sends task payloads and manages bidirectional communication
 * 5. Cleans up sockets and pane resources on completion
 */
export class PaneManager {
  private panes = new Map<string, ManagedPane>();
  private paneCounter = 0;
  private config: MultiplexerConfig;

  constructor(
    private readonly backend: Backend | null,
    config: MultiplexerConfig,
  ) {
    this.config = normalizeConfig(config);
  }

  /**
   * SpawnAgentPane creates a new pane, sets up IPC, and returns a managed pane
   * for bidirectional communication with the child agent process.
   */
  async spawnAgentPane(agentName: string, task: string, signal?: AbortSignal): Promise<ManagedPane> {
    if (!this.backend) {
      throw newIOError("no multiplexer backend available");
    }

    this.paneCounter++;
    const paneID = `pane-${this.paneCounter}`;
    const sockPath = socketPath(this.config.socketDir, paneID);

    // 1. Create Unix socket listener.
    const server = await createListener(sockPath);
    try {
      // 2. Spawn pane running the pane-agent command.
      const command = `orange-code pane-agent --socket ${sockPath}`;
      const info = await this.backend.createPane(agentName, command);
      // Override the backend-generated ID with our tracked ID.
      info.id = paneID;

      // 3. Wait for the child process to connect.
      const conn = await waitForConnection(server, this.config.commandTimeoutMs);
      const transport = new SocketTransport(conn);

      // 4. Send the task payload.
      const taskPayload: TaskPayload = {
        task,
        agentId: agentName,
        sessionId: "",
        tools: [],
      };

      const msg: IPCMessage = {
        type: IPCMessageType.Task,
        id: paneID,
        payload: taskPayload,
      };

      await transport.send(msg);

      const abortController = new AbortController();
      if (signal) {
        signal.addEventListener("abort", () => abortController.abort());
      }

      const managed: ManagedPane = {
        info,
        transport,
        socketPath: sockPath,
        abortController,
      };

      this.panes.set(paneID, managed);

      // Auto-cleanup when aborted.
      abortController.signal.addEventListener("abort", () => {
        this.closePane(paneID).catch(() => {});
      });

      return managed;
    } finally {
      // Close the listening socket (we no longer need it).
      server.close();
    }
  }

  /**
   * ClosePane tears down a pane and cleans up its socket.
   */
  async closePane(paneID: string): Promise<void> {
    const managed = this.panes.get(paneID);
    if (!managed) return;
    this.panes.delete(paneID);

    managed.abortController.abort();
    await managed.transport.close();
    if (this.backend) {
      await this.backend.closePane(paneID).catch(() => {});
    }
    await cleanupSocket(managed.socketPath);
  }

  /**
   * CloseAll terminates all managed panes.
   */
  async closeAll(): Promise<void> {
    const ids = [...this.panes.keys()];
    for (const id of ids) {
      await this.closePane(id);
    }
  }

  /**
   * ActivePanes returns all currently tracked panes.
   */
  activePanes(): PaneInfo[] {
    return [...this.panes.values()].map((p) => p.info);
  }
}
