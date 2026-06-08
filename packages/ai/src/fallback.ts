import type { AiProvider } from "./provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent } from "./types.js";

// ---------------------------------------------------------------------------
// FallbackChain
// ---------------------------------------------------------------------------

interface ProviderEntry {
  provider: AiProvider;
  coolUntil: number; // timestamp in ms
  lastErr: Error | null;
}

/**
 * Tries multiple providers in order, falling back on failure.
 * Providers that recently failed are put on cooldown and skipped.
 */
export class FallbackChain {
  private entries: ProviderEntry[];
  private cooldownMs: number;

  /**
   * Creates a new FallbackChain with the given providers and
   * cooldown duration (in milliseconds). Providers are tried in array order.
   */
  constructor(providers: AiProvider[], cooldownMs: number) {
    this.entries = providers.map((p) => ({
      provider: p,
      coolUntil: 0,
      lastErr: null,
    }));
    this.cooldownMs = cooldownMs;
  }

  /**
   * Tries providers in order until one succeeds.
   * Providers on cooldown are skipped. On failure, the provider is put on cooldown.
   */
  async chatCompletion(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AiResponse> {
    let lastErr: Error | null = null;

    for (const entry of this.entries) {
      const now = Date.now();
      if (now < entry.coolUntil) {
        if (entry.lastErr) {
          lastErr = entry.lastErr;
        }
        continue;
      }

      try {
        return await entry.provider.chatCompletion(messages, tools, opts);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        entry.coolUntil = Date.now() + this.cooldownMs;
        entry.lastErr = error;
        lastErr = error;
      }
    }

    throw lastErr ?? new Error("all providers failed");
  }

  /**
   * Tries providers in order until one succeeds.
   * Providers on cooldown are skipped. On failure, the provider is put on cooldown.
   */
  async chatCompletionStream(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AsyncIterable<StreamEvent>> {
    let lastErr: Error | null = null;

    for (const entry of this.entries) {
      const now = Date.now();
      if (now < entry.coolUntil) {
        if (entry.lastErr) {
          lastErr = entry.lastErr;
        }
        continue;
      }

      try {
        return await entry.provider.chatCompletionStream(messages, tools, opts);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        entry.coolUntil = Date.now() + this.cooldownMs;
        entry.lastErr = error;
        lastErr = error;
      }
    }

    throw lastErr ?? new Error("all providers failed");
  }

  /** Returns a copy of the provider list (for testing/inspection). */
  providers(): AiProvider[] {
    return this.entries.map((e) => e.provider);
  }

  /** Returns true if the provider at the given index is on cooldown. */
  isCoolingDown(index: number): boolean {
    if (index < 0 || index >= this.entries.length) {
      return false;
    }
    return Date.now() < this.entries[index]!.coolUntil;
  }
}
