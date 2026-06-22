/**
 * Tests for the SSE stream parser.
 */

import { parseSSEStream } from "../stream.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Creates a ReadableStream from string chunks. */
function makeStream(chunks: string[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  return new ReadableStream({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(encoder.encode(chunk));
      }
      controller.close();
    },
  });
}

// ---------------------------------------------------------------------------
// parseSSEStream
// ---------------------------------------------------------------------------

describe("parseSSEStream", () => {
  it("parses data: lines", async () => {
    const stream = makeStream([
      "data: hello\n",
      "data: world\n",
    ]);
    const reader = stream.getReader();
    const payloads = await parseSSEStream(reader);
    expect(payloads).toEqual(["hello", "world"]);
  });

  it("ignores empty lines", async () => {
    const stream = makeStream([
      "data: first\n",
      "\n",
      "data: second\n",
    ]);
    const reader = stream.getReader();
    const payloads = await parseSSEStream(reader);
    expect(payloads).toEqual(["first", "second"]);
  });

  it("ignores comment lines (starting with :)", async () => {
    const stream = makeStream([
      ": this is a comment\n",
      "data: real data\n",
    ]);
    const reader = stream.getReader();
    const payloads = await parseSSEStream(reader);
    expect(payloads).toEqual(["real data"]);
  });

  it("ignores [DONE] sentinel", async () => {
    const stream = makeStream([
      "data: content\n",
      "data: [DONE]\n",
    ]);
    const reader = stream.getReader();
    const payloads = await parseSSEStream(reader);
    expect(payloads).toEqual(["content"]);
  });

  it("handles JSON data payloads", async () => {
    const json = '{"choices":[{"delta":{"content":"Hi"}}]}';
    const stream = makeStream([`data: ${json}\n`]);
    const reader = stream.getReader();
    const payloads = await parseSSEStream(reader);
    expect(payloads).toHaveLength(1);
    expect(JSON.parse(payloads[0]!)).toEqual(JSON.parse(json));
  });

  it("handles data split across chunks", async () => {
    const stream = makeStream([
      "data: hel",
      "lo world\n",
      "data: second\n",
    ]);
    const reader = stream.getReader();
    const payloads = await parseSSEStream(reader);
    expect(payloads).toEqual(["hello world", "second"]);
  });

  it("returns empty array for no data lines", async () => {
    const stream = makeStream([
      ": comment only\n",
      "\n",
    ]);
    const reader = stream.getReader();
    const payloads = await parseSSEStream(reader);
    expect(payloads).toEqual([]);
  });

  it("handles remaining buffer after stream ends", async () => {
    const stream = makeStream([
      "data: trailing",  // no newline at end
    ]);
    const reader = stream.getReader();
    const payloads = await parseSSEStream(reader);
    expect(payloads).toEqual(["trailing"]);
  });
});
