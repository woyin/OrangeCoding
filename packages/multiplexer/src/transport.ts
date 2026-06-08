import { createConnection, createServer, type Socket } from "node:net";
import { mkdir, rm } from "node:fs/promises";
import { join, dirname } from "node:path";
import { newIOError } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// IPC message types
// ---------------------------------------------------------------------------

/** IPC message type constants. */
export const IPCMessageType = {
  Task: "task",
  Result: "result",
  Event: "event",
  Keepalive: "keepalive",
} as const;
export type IPCMessageType = (typeof IPCMessageType)[keyof typeof IPCMessageType];

/** IPCMessage is the envelope for all inter-pane communication. */
export interface IPCMessage {
  type: IPCMessageType;
  id: string;
  payload: unknown;
}

/** TaskPayload is sent parent -> child to assign work. */
export interface TaskPayload {
  task: string;
  agentId: string;
  sessionId: string;
  tools: string[];
  env?: Record<string, string>;
}

/** ResultPayload is sent child -> parent when the task finishes. */
export interface ResultPayload {
  success: boolean;
  content: string;
  error?: string;
}

/** EventPayload is sent child -> parent for streaming updates. */
export interface EventPayload {
  eventType: string;
  data: string;
}

// ---------------------------------------------------------------------------
// SocketTransport
// ---------------------------------------------------------------------------

/**
 * SocketTransport implements bidirectional IPC over a Unix domain socket
 * using a line-delimited JSON protocol.
 */
export class SocketTransport {
  private buffer = "";

  constructor(private conn: Socket) {}

  /**
   * Send writes a JSON-encoded message as a single newline-terminated line.
   */
  async send(msg: IPCMessage): Promise<void> {
    const data = JSON.stringify(msg) + "\n";
    return new Promise((resolve, reject) => {
      this.conn.write(data, (err) => {
        if (err) reject(err);
        else resolve();
      });
    });
  }

  /**
   * Receive reads the next newline-terminated JSON message.
   */
  async receive(): Promise<IPCMessage> {
    return new Promise((resolve, reject) => {
      const onData = (chunk: Buffer | string) => {
        this.buffer += chunk.toString();
        const idx = this.buffer.indexOf("\n");
        if (idx !== -1) {
          const line = this.buffer.substring(0, idx);
          this.buffer = this.buffer.substring(idx + 1);
          this.conn.removeListener("data", onData);
          this.conn.removeListener("error", onError);
          this.conn.removeListener("end", onEnd);
          try {
            resolve(JSON.parse(line));
          } catch (err) {
            reject(newIOError(`unmarshal IPC message: ${(err as Error).message}`));
          }
        }
      };

      const onError = (err: Error) => {
        this.conn.removeListener("data", onData);
        this.conn.removeListener("end", onEnd);
        reject(err);
      };

      const onEnd = () => {
        this.conn.removeListener("data", onData);
        this.conn.removeListener("error", onError);
        reject(new Error("socket closed"));
      };

      this.conn.on("data", onData);
      this.conn.on("error", onError);
      this.conn.on("end", onEnd);
    });
  }

  /** Close closes the underlying connection. */
  async close(): Promise<void> {
    this.conn.destroy();
  }
}

// ---------------------------------------------------------------------------
// Socket helpers
// ---------------------------------------------------------------------------

/**
 * CreateListener creates a Unix domain socket listener at the given path.
 * Parent side calls this to wait for the child process to connect.
 */
export async function createListener(socketPath: string) {
  try {
    await mkdir(dirname(socketPath), { recursive: true });
  } catch (err) {
    throw newIOError(`create socket dir: ${(err as Error).message}`);
  }

  // Remove stale socket if present.
  try { await rm(socketPath); } catch { /* ignore */ }

  const server = createServer();
  return new Promise<typeof server>((resolve, reject) => {
    server.listen(socketPath, () => resolve(server));
    server.on("error", reject);
  });
}

/**
 * WaitForConnection accepts a single connection with a timeout.
 */
export async function waitForConnection(
  server: import("node:net").Server,
  timeoutMs: number,
): Promise<Socket> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`timeout waiting for pane connection after ${timeoutMs}ms`));
    }, timeoutMs);

    server.once("connection", (socket: Socket) => {
      clearTimeout(timer);
      resolve(socket);
    });
  });
}

/**
 * ConnectSocket connects to a Unix domain socket at the given path.
 * Child side calls this to establish IPC with the parent.
 */
export async function connectSocket(socketPath: string, timeoutMs: number): Promise<Socket> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      return await new Promise<Socket>((resolve, reject) => {
        const sock = createConnection(socketPath, () => resolve(sock));
        sock.on("error", reject);
      });
    } catch {
      await new Promise((r) => setTimeout(r, 50));
    }
  }
  throw new Error(`timeout connecting to socket ${socketPath} after ${timeoutMs}ms`);
}

/**
 * SocketPath returns the standard socket path for a given pane ID.
 */
export function socketPath(socketDir: string, paneID: string): string {
  return join(socketDir, `${paneID}.sock`);
}

/**
 * CleanupSocket removes the socket file and its directory if empty.
 */
export async function cleanupSocket(path: string): Promise<void> {
  try { await rm(path); } catch { /* ignore */ }
  try { await rm(dirname(path)); } catch { /* ignore if not empty */ }
}
