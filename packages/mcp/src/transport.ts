/**
 * 包内共享的 TextEncoder 单例。TextEncoder 无状态，可安全跨调用复用，
 * 避免每次收发消息都 new 一个（transport/client/server 共用）。
 */
const sharedEncoder = new TextEncoder();

/**
 * Transport：对 JSON-RPC 消息底层通道的抽象。
 *
 * - send：把一条 JSON-RPC 消息（字节）写入通道
 * - receive：读取下一条 JSON-RPC 消息（字节）
 * - close：释放通道持有的资源
 */
export interface Transport {
  /** Send writes a JSON-RPC message (as bytes) to the transport. */
  send(data: Uint8Array): Promise<void>;
  /** Receive reads the next JSON-RPC message from the transport. */
  receive(): Promise<Uint8Array>;
  /** Close releases any resources held by the transport. */
  close(): Promise<void>;
}

/**
 * StdioTransport：基于 stdin/stdout 风格 I/O 的 Transport 实现。
 * 消息以换行分帧（每条 JSON 消息以 \n 结尾）。
 */
export class StdioTransport implements Transport {
  private buffer = "";
  private closed = false;
  /** 复用的 UTF-8 解码器；多字节序列跨 chunk 时靠其内部状态拼接。 */
  private readonly _decoder = new TextDecoder();

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
          resolve(sharedEncoder.encode(line));
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
 * SSETransport：基于 HTTP POST（发送）+ SSE（接收）的 Transport 实现。
 *
 * - 发送：把 JSON-RPC 消息以 JSON 形式 POST 到服务端端点
 * - 接收：从首次发送建立的 SSE 流中读取响应
 *
 * 对应 MCP HTTP 传输规范：客户端 POST 请求，服务端用 SSE 事件回响应。
 */
export class SSETransport implements Transport {
  private closed = false;
  private readonly _decoder = new TextDecoder();
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
      body: this._decoder.decode(data),
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
        const responseData = sharedEncoder.encode(text);
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

        this._sseBuffer += this._decoder.decode(value);

        // Parse SSE events from buffer
        let eventEnd: number;
        while ((eventEnd = this._sseBuffer.indexOf("\n\n")) !== -1) {
          const eventText = this._sseBuffer.substring(0, eventEnd);
          this._sseBuffer = this._sseBuffer.substring(eventEnd + 2);

          const data = this.extractSSEData(eventText);
          if (data) {
            const encoded = sharedEncoder.encode(data);
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

  /**
   * 从一条 SSE 事件文本里提取 `data:` 字段。
   *
   * 性能优化：原实现用 split("\n") 对事件文本切片构造数组，
   * 高频事件下分配显著。改为 indexOf 逐行扫描、直接 substring 取负载，
   * 不再分配中间数组。
   */
  private extractSSEData(eventText: string): string | null {
    let pos = 0;
    while (pos < eventText.length) {
      const nl = eventText.indexOf("\n", pos);
      const lineEnd = nl === -1 ? eventText.length : nl;
      // 行首是否为 "data: "（长度 6）
      if (
        lineEnd >= pos + 6 &&
        eventText.charCodeAt(pos) === 100 /* d */ &&
        eventText.charCodeAt(pos + 1) === 97 /* a */ &&
        eventText.charCodeAt(pos + 2) === 116 /* t */ &&
        eventText.charCodeAt(pos + 3) === 97 /* a */ &&
        eventText.charCodeAt(pos + 4) === 58 /* : */ &&
        eventText.charCodeAt(pos + 5) === 32 /* space */
      ) {
        const data = eventText.substring(pos + 6, lineEnd);
        if (data === "[DONE]") return null;
        return data;
      }
      if (nl === -1) break;
      pos = nl + 1;
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
 * StreamableHTTPTransport：纯 HTTP POST 的 Transport 实现。
 *
 * 每次 send() POST 一个请求，receive() 从响应体读取。对应 MCP 规范中
 * “Streamable HTTP”——每个请求/响应是一次独立的 HTTP 交换，严格单飞。
 */
export class StreamableHTTPTransport implements Transport {
  private closed = false;
  private readonly _decoder = new TextDecoder();
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

    const body = this._decoder.decode(data);

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
      return sharedEncoder.encode(text);
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
