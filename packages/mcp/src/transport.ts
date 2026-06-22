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

  /**
   * Write a message to the underlying stream as a line of UTF-8. The
   * newline framing is what makes messages self-delimiting on the wire.
   */
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

  /**
   * Block until a complete newline-terminated line arrives, then return it
   * as encoded bytes. Attaches one-shot data/error/end listeners per call
   * and removes them on resolution so concurrent receives do not interfere.
   */
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

  /** Idempotently close the writer (and thus the underlying stream). */
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

// ---------------------------------------------------------------------------
// SSETransport — Client-side transport using HTTP + Server-Sent Events
// ---------------------------------------------------------------------------
// Client-side transport matching servers that respond with text/event-stream.
// POST carries requests; SSE carries responses. Inbound messages are either
// resolved immediately (if a receive() call is already pending) or queued
// for the next receive().

/**
 * SSETransport implements Transport over HTTP POST (send) and SSE (receive).
 *
 * - Send: POSTs JSON-RPC messages to the server endpoint as line-delimited JSON.
 * - Receive: Reads responses from an SSE stream established on first send.
 *
 * This matches the MCP HTTP transport specification where the client POSTs
 * requests and the server responds via SSE events.
 */
export class SSETransport implements Transport {
  private closed = false;
  private _endpoint: string;
  private _headers: Record<string, string>;
  private _messageQueue: Uint8Array[] = [];
  private _waitingResolver: ((data: Uint8Array) => void) | null = null;
  private _sseReader: ReadableStreamDefaultReader<Uint8Array> | null = null;
  private _sseBuffer = "";
  private _sseConnected = false;

  constructor(
    endpoint: string,
    headers?: Record<string, string>,
  ) {
    this._endpoint = endpoint;
    this._headers = headers ?? {};
  }

  async send(data: Uint8Array): Promise<void> {
    if (this.closed) {
      throw new Error("SSETransport: closed");
    }

    // POST the JSON-RPC message to the server
    const response = await fetch(this._endpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...this._headers,
      },
      body: new TextDecoder().decode(data),
    });

    if (!response.ok) {
      throw new Error(`SSETransport: POST failed with status ${response.status}`);
    }

    // If the response is SSE, start reading the stream
    const contentType = response.headers.get("content-type") ?? "";
    if (contentType.includes("text/event-stream") && response.body) {
      this.processSSEStream(response.body).catch(() => {
        // SSE stream error — non-fatal, next receive will handle it
      });
    } else {
      // Direct JSON response (not SSE)
      const text = await response.text();
      if (text.trim()) {
        const responseData = new TextEncoder().encode(text);
        if (this._waitingResolver) {
          const resolve = this._waitingResolver;
          this._waitingResolver = null;
          resolve(responseData);
        } else {
          this._messageQueue.push(responseData);
        }
      }
    }
  }

  /**
   * Block until a complete newline-terminated line arrives, then return it
   * as encoded bytes. Attaches one-shot data/error/end listeners per call
   * and removes them on resolution so concurrent receives do not interfere.
   */
  async receive(): Promise<Uint8Array> {
    if (this.closed) {
      throw new Error("SSETransport: closed");
    }

    // Return queued message if available
    if (this._messageQueue.length > 0) {
      return this._messageQueue.shift()!;
    }

    // Wait for next message
    return new Promise<Uint8Array>((resolve) => {
      this._waitingResolver = resolve;
    });
  }

  /** Idempotently close the writer (and thus the underlying stream). */
  async close(): Promise<void> {
    if (!this.closed) {
      this.closed = true;
      if (this._sseReader) {
        await this._sseReader.cancel().catch(() => {});
        this._sseReader = null;
      }
      if (this._waitingResolver) {
        this._waitingResolver = null;
      }
      this._messageQueue = [];
    }
  }

  private async processSSEStream(body: ReadableStream<Uint8Array>): Promise<void> {
    this._sseReader = body.getReader();

    try {
      for (;;) {
        const { done, value } = await this._sseReader.read();
        if (done) break;

        this._sseBuffer += new TextDecoder().decode(value);

        // Parse SSE events from buffer
        let eventEnd: number;
        while ((eventEnd = this._sseBuffer.indexOf("\n\n")) !== -1) {
          const eventText = this._sseBuffer.substring(0, eventEnd);
          this._sseBuffer = this._sseBuffer.substring(eventEnd + 2);

          const data = this.extractSSEData(eventText);
          if (data) {
            const encoded = new TextEncoder().encode(data);
            if (this._waitingResolver) {
              const resolve = this._waitingResolver;
              this._waitingResolver = null;
              resolve(encoded);
            } else {
              this._messageQueue.push(encoded);
            }
          }
        }
      }
    } catch {
      // Stream ended or error
    } finally {
      this._sseReader = null;
    }
  }

  /** Extract the data field from an SSE event text. */
  private extractSSEData(eventText: string): string | null {
    for (const line of eventText.split("\n")) {
      if (line.startsWith("data: ")) {
        const data = line.substring(6);
        if (data === "[DONE]") return null;
        return data;
      }
    }
    return null;
  }
}

// ---------------------------------------------------------------------------
// StreamableHTTPTransport — Modern MCP HTTP transport (POST-based, no SSE)
// ---------------------------------------------------------------------------
// Each send() POSTs a request and stores the response promise; receive()
// awaits and clears it. Strictly one-in-flight at a time, matching the MCP
// "Streamable HTTP" spec where each request/response is a separate exchange.

/**
 * StreamableHTTPTransport implements Transport over pure HTTP POST.
 *
 * Each send() POSTs the request, and receive() reads from the response body.
 * This is the "Streamable HTTP" transport from the MCP spec where each
 * request-response pair is a separate HTTP exchange.
 */
export class StreamableHTTPTransport implements Transport {
  private closed = false;
  private _endpoint: string;
  private _headers: Record<string, string>;
  private _pendingResponse: Promise<Uint8Array> | null = null;

  constructor(
    endpoint: string,
    headers?: Record<string, string>,
  ) {
    this._endpoint = endpoint;
    this._headers = headers ?? {};
  }

  async send(data: Uint8Array): Promise<void> {
    if (this.closed) {
      throw new Error("StreamableHTTPTransport: closed");
    }

    const body = new TextDecoder().decode(data);

    this._pendingResponse = (async () => {
      const response = await fetch(this._endpoint, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Accept": "application/json",
          ...this._headers,
        },
        body,
      });

      if (!response.ok) {
        throw new Error(`StreamableHTTPTransport: POST failed with status ${response.status}`);
      }

      const text = await response.text();
      return new TextEncoder().encode(text);
    })();
  }

  /**
   * Block until a complete newline-terminated line arrives, then return it
   * as encoded bytes. Attaches one-shot data/error/end listeners per call
   * and removes them on resolution so concurrent receives do not interfere.
   */
  async receive(): Promise<Uint8Array> {
    if (this.closed) {
      throw new Error("StreamableHTTPTransport: closed");
    }

    if (!this._pendingResponse) {
      throw new Error("StreamableHTTPTransport: no pending request to receive response for");
    }

    const data = await this._pendingResponse;
    this._pendingResponse = null;
    return data;
  }

  /** Idempotently close the writer (and thus the underlying stream). */
  async close(): Promise<void> {
    this.closed = true;
    this._pendingResponse = null;
  }
}
