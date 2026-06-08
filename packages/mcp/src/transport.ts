/** Transport abstracts the underlying communication channel for JSON-RPC messages. */
export interface Transport {
  /** Send writes a JSON-RPC message (as bytes) to the transport. */
  send(data: Uint8Array): Promise<void>;
  /** Receive reads the next JSON-RPC message from the transport. */
  receive(): Promise<Uint8Array>;
  /** Close releases any resources held by the transport. */
  close(): Promise<void>;
}

/**
 * StdioTransport implements Transport over stdin/stdout-style I/O.
 * Messages are line-delimited JSON: each message is terminated by a newline.
 */
export class StdioTransport implements Transport {
  private buffer = "";
  private closed = false;

  constructor(
    private readonly reader: NodeJS.ReadableStream,
    private readonly writer: NodeJS.WritableStream,
  ) {}

  async send(data: Uint8Array): Promise<void> {
    return new Promise((resolve, reject) => {
      this.writer.write(data);
      this.writer.write("\n", (err) => {
        if (err) {
          reject(err);
        } else {
          resolve();
        }
      });
    });
  }

  async receive(): Promise<Uint8Array> {
    return new Promise((resolve, reject) => {
      const onData = (chunk: Buffer | string) => {
        this.buffer += chunk.toString();
        const idx = this.buffer.indexOf("\n");
        if (idx !== -1) {
          const line = this.buffer.substring(0, idx);
          this.buffer = this.buffer.substring(idx + 1);
          this.reader.removeListener("data", onData);
          this.reader.removeListener("error", onError);
          this.reader.removeListener("end", onEnd);
          resolve(new TextEncoder().encode(line));
        }
      };

      const onError = (err: Error) => {
        this.reader.removeListener("data", onData);
        this.reader.removeListener("end", onEnd);
        reject(err);
      };

      const onEnd = () => {
        this.reader.removeListener("data", onData);
        this.reader.removeListener("error", onError);
        reject(new Error("transport: stream ended"));
      };

      this.reader.on("data", onData);
      this.reader.on("error", onError);
      this.reader.on("end", onEnd);
    });
  }

  async close(): Promise<void> {
    if (!this.closed) {
      this.closed = true;
      const writer = this.writer as NodeJS.WritableStream & { end(cb?: () => void): void };
      if (typeof writer.end === "function") {
        return new Promise((resolve) => {
          writer.end(() => resolve());
        });
      }
    }
  }
}
