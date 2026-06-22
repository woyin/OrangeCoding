/**
 * SSETransport SSE 帧解析测试。
 *
 * 保护目标：extractSSEData 从 indexOf 重构（原 split("\n")）后，
 * 仍能正确处理：单行 data、多行事件（含 event:/id: 等非 data 行）、
 * [DONE] 终止标记、以及多 chunk 拼接。
 */

import { SSETransport } from "../transport.js";

/**
 * 用伪造的 fetch 触发 SSE 流程：返回一个 ReadableStream，
 * 内容是给定的 SSE 事件字节序列。
 */
function withMockFetch(sseBytes: Uint8Array, fn: () => Promise<void>): Promise<void> {
  const original = globalThis.fetch;
  globalThis.fetch = (() =>
    Promise.resolve(
      new Response(new ReadableStream({
        start(controller) {
          controller.enqueue(sseBytes);
          controller.close();
        },
      }), {
        status: 200,
        headers: { "content-type": "text/event-stream" },
      }),
    )) as typeof fetch;
  return fn().finally(() => {
    globalThis.fetch = original;
  });
}

function enc(s: string): Uint8Array {
  return new TextEncoder().encode(s);
}

describe("SSETransport SSE 帧解析", () => {
  it("单行 data 事件被正确提取", async () => {
    const t = new SSETransport("http://x");
    await withMockFetch(enc('data: {"jsonrpc":"2.0","id":1,"result":42}\n\n'), async () => {
      await t.send(enc('{"jsonrpc":"2.0","id":1,"method":"ping"}'));
      const msg = await t.receive();
      expect(new TextDecoder().decode(msg)).toBe('{"jsonrpc":"2.0","id":1,"result":42}');
    });
    await t.close();
  });

  it("多行事件（含非 data 行）只取 data 字段", async () => {
    const t = new SSETransport("http://x");
    await withMockFetch(
      enc('event: message\nid: 7\ndata: {"ok":true}\n\n'),
      async () => {
        await t.send(enc('{"jsonrpc":"2.0","id":1}'));
        const msg = await t.receive();
        expect(new TextDecoder().decode(msg)).toBe('{"ok":true}');
      },
    );
    await t.close();
  });

  it("[DONE] 标记被跳过（不作为负载返回）", async () => {
    const t = new SSETransport("http://x");
    await withMockFetch(
      enc('data: {"a":1}\n\ndata: [DONE]\n\n'),
      async () => {
        await t.send(enc('{"jsonrpc":"2.0","id":1}'));
        const msg = await t.receive();
        expect(new TextDecoder().decode(msg)).toBe('{"a":1}');
      },
    );
    await t.close();
  });

  it("多 chunk 拼接：事件跨 chunk 边界仍能完整解析", async () => {
    const t = new SSETransport("http://x");
    const original = globalThis.fetch;
    globalThis.fetch = (() =>
      Promise.resolve(
        new Response(
          new ReadableStream({
            start(controller) {
              controller.enqueue(enc('data: {"par'));
              controller.enqueue(enc('tial":true}\n\n'));
              controller.close();
            },
          }),
          { status: 200, headers: { "content-type": "text/event-stream" } },
        ),
      )) as typeof fetch;
    try {
      await t.send(enc('{"jsonrpc":"2.0","id":1}'));
      const msg = await t.receive();
      expect(new TextDecoder().decode(msg)).toBe('{"partial":true}');
    } finally {
      globalThis.fetch = original;
      await t.close();
    }
  });
});
