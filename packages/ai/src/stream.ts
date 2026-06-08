// ---------------------------------------------------------------------------
// SSE stream parser
// ---------------------------------------------------------------------------

/**
 * Parses Server-Sent Events from a text stream and returns data payloads.
 * Reads lines prefixed with "data: " and collects the payload after the prefix.
 * Empty lines, comment lines (starting with ":"), and "data: [DONE]" are skipped.
 */
export async function parseSSEStream(
  reader: ReadableStreamDefaultReader<Uint8Array>,
): Promise<string[]> {
  const payloads: string[] = [];
  const decoder = new TextDecoder();
  let buffer = "";

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      // Keep the last potentially incomplete line in the buffer
      buffer = lines.pop() ?? "";

      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed === "") continue;
        if (trimmed.startsWith(":")) continue;
        if (!trimmed.startsWith("data: ")) continue;

        const payload = trimmed.slice(6); // "data: ".length === 6
        if (payload.trim() === "[DONE]") continue;

        payloads.push(payload);
      }
    }

    // Process any remaining content in buffer
    const remaining = buffer.trim();
    if (remaining !== "" && remaining.startsWith("data: ")) {
      const payload = remaining.slice(6);
      if (payload.trim() !== "[DONE]") {
        payloads.push(payload);
      }
    }
  } catch (err) {
    throw new Error(`SSE stream read error: ${err instanceof Error ? err.message : String(err)}`);
  }

  return payloads;
}
