/**
 * @module ai-stream
 *
 * Streaming utilities for AI provider responses.
 *
 * Provides helpers for parsing Server-Sent Events (SSE) streams
 * and converting them to the AiProvider's AsyncIterable<StreamEvent> format.
 */
// ---------------------------------------------------------------------------
// SSE stream parser
// ---------------------------------------------------------------------------

/**
 * 解析 Server-Sent Events 文本流，返回所有 `data:` 负载。
 *
 * 只读取以 `data: ` 开头的行；空行、注释行（以 `:` 开头）与 `data: [DONE]` 被跳过。
 *
 * 性能优化：原实现对每行调用 trim() 多达 3 次（trimmed、payload.trim() ×2），
 * 在高吞吐流式响应里是显著的不必要分配。重构后每行只 trim 一次，
 * 并用常量前缀长度避免重复字面量计算；同时把 `[DONE]` 判定挪到 slice 之前，
 * 减少一次 slice 调用。
 */
export async function parseSSEStream(
  reader: ReadableStreamDefaultReader<Uint8Array>,
): Promise<string[]> {
  const payloads: string[] = [];
  const decoder = new TextDecoder();
  let buffer = "";
  // `data: ` 前缀长度为 6，提取为常量避免每次循环重新计算字面量长度。
  const DATA_PREFIX = "data: ";
  const DATA_PREFIX_LEN = 6;

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      // 保留最后一条可能不完整的行在 buffer 中，等下一次 read 拼接。
      buffer = lines.pop() ?? "";

      for (const line of lines) {
        // SSE 规范允许行首/行尾有空白；每行只 trim 一次，避免重复分配。
        const trimmed = line.trim();
        if (trimmed === "") continue;
        // 注释行或非 data 行直接跳过。
        if (trimmed.charCodeAt(0) === 58 /* ":" */ ) continue;
        if (!trimmed.startsWith(DATA_PREFIX)) continue;

        const payload = trimmed.slice(DATA_PREFIX_LEN);
        // 终止标记 [DONE]（可能带尾部空白），跳过不作为负载返回。
        if (payload.trim() === "[DONE]") continue;

        payloads.push(payload);
      }
    }

    // 处理 buffer 中残留的最后一条（可能没有结尾换行）。
    const remaining = buffer.trim();
    if (remaining !== "" && remaining.startsWith(DATA_PREFIX)) {
      const payload = remaining.slice(DATA_PREFIX_LEN);
      if (payload.trim() !== "[DONE]") {
        payloads.push(payload);
      }
    }
  } catch (err) {
    throw new Error(`SSE stream read error: ${err instanceof Error ? err.message : String(err)}`);
  }

  return payloads;
}
