/**
 * @module wenxin
 *
 * Baidu Wenxin (ERNIE) provider implementation.
 *
 * Adapts the Wenxin API to the AiProvider interface.
 * Uses Baidu's authentication flow (access token via API key/secret).
 */
import type { ProviderConfig } from "./provider.js";
import { providerTimeout } from "./provider.js";
import type { ChatMessage, ToolDefinition, ChatOptions, AiResponse, StreamEvent, AiTokenUsage } from "./types.js";
import { newAiParseError, newAiNetworkError, newAiApiError, newAiAuthError } from "./error.js";
import { parseSSEStream } from "./stream.js";

// ---------------------------------------------------------------------------
// Wenxin / Baidu ERNIE provider
// ---------------------------------------------------------------------------
// Baidu's ERNIE (Wenxin) models expose an OpenAI-incompatible HTTP API.
// Two quirks this provider papers over:
//   1. Authentication is OAuth2 (client_credentials) rather than bearer API
//      keys, so we cache an access_token and refresh it ~5 min before expiry.
//   2. The chat completion field is `result` (not `content`), and the
//      endpoint path is per-model (completions_pro vs completions vs
//      ernie_speed/ernie_lite). modelToEndpoint maps friendly names.

const DEFAULT_WENXIN_BASE_URL = "https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop";

// ---------------------------------------------------------------------------
// Wenxin wire types
// ---------------------------------------------------------------------------
// Shape of the JSON returned by the Wenxin chat endpoints. Only the fields
// we read are typed; the API may return others which we ignore.

interface WenxinResponse {
  id: string;
  object: string;
  result: string; // Wenxin uses "result" instead of "content"
  model: string;
  usage: WenxinUsage;
}

interface WenxinUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

// ---------------------------------------------------------------------------
// WenxinProvider class
// ---------------------------------------------------------------------------

/**
 * Provider implementing AiProvider for Baidu Wenxin/ERNIE models. Owns the
 * OAuth2 access-token lifecycle (cached in-memory, auto-refreshed). Both the
 * non-streaming and streaming chat methods share the same request-body
 * construction; streaming only adds `stream: true` and parses the SSE deltas.
 */

export class WenxinProvider {
  private config: ProviderConfig;
  private baseURL: string;
  private timeoutMs: number;

  // OAuth token state. `tokenExpiry` is a wall-clock ms timestamp; we refresh
  // proactively 5 minutes before it lapses to avoid mid-request 401s.
  private accessToken = "";
  private tokenExpiry = 0;

  constructor(config: ProviderConfig) {
    this.config = config;
    this.baseURL = config.baseURL || DEFAULT_WENXIN_BASE_URL;
    this.timeoutMs = providerTimeout(config);
  }

  name(): string {
    return "wenxin";
  }

  // -------------------------------------------------------------------------
  // ChatCompletion (non-streaming)
  // -------------------------------------------------------------------------

  /**
   * Non-streaming chat completion. Builds the Wenxin request body (note the
   * `result`-style response), waits for the full payload, and normalizes it
   * into the shared {@link AiResponse}. Aborts on timeout.
   */
  async chatCompletion(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AiResponse> {
    const model = opts.model || this.config.defaultModel;

    const reqBody = {
      model,
      messages,
      tools: tools.length > 0 ? tools : undefined,
      temperature: opts.temperature,
      max_tokens: opts.max_tokens,
      top_p: opts.top_p,
      stop: opts.stop_sequences,
    };

    const body = safeMarshal(reqBody);

    const accessToken = await this.getAccessToken();

    const url = this.buildURL(accessToken, model);
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const resp = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
        signal: controller.signal,
      });

      if (!resp.ok) {
        const respBody = await resp.text();
        throw newAiApiError(
          `API returned status ${resp.status}: ${respBody}`,
          resp.status,
        );
      }

      const result = (await resp.json()) as WenxinResponse;

      return {
        content: result.result ?? "",
        model: result.model ?? "",
        usage: {
          prompt_tokens: result.usage?.prompt_tokens ?? 0,
          completion_tokens: result.usage?.completion_tokens ?? 0,
          total_tokens: result.usage?.total_tokens ?? 0,
        },
        finish_reason: "stop",
        tool_calls: [],
      };
    } catch (err) {
      if (err instanceof Error && err.name === "AbortError") {
        throw newAiNetworkError(`request timed out after ${this.timeoutMs}ms`);
      }
      throw err;
    } finally {
      clearTimeout(timer);
    }
  }

  // -------------------------------------------------------------------------
  // ChatCompletionStream (streaming)
  // -------------------------------------------------------------------------

  /**
   * Streaming chat completion. Same request body as {@link chatCompletion}
   * plus `stream: true`; the response body is parsed as SSE where each chunk
   * carries an incremental `result` delta until `is_end` is true.
   */
  async chatCompletionStream(
    messages: ChatMessage[],
    tools: ToolDefinition[],
    opts: ChatOptions,
  ): Promise<AsyncIterable<StreamEvent>> {
    const model = opts.model || this.config.defaultModel;

    const reqBody = {
      model,
      messages,
      tools: tools.length > 0 ? tools : undefined,
      stream: true,
      temperature: opts.temperature,
      max_tokens: opts.max_tokens,
      top_p: opts.top_p,
      stop: opts.stop_sequences,
    };

    const body = safeMarshal(reqBody);

    const accessToken = await this.getAccessToken();

    const url = this.buildURL(accessToken, model);
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);

    let resp: Response;
    try {
      resp = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
        signal: controller.signal,
      });
    } catch (err) {
      clearTimeout(timer);
      if (err instanceof Error && err.name === "AbortError") {
        throw newAiNetworkError(`request timed out after ${this.timeoutMs}ms`);
      }
      throw err;
    }

    if (!resp.ok) {
      clearTimeout(timer);
      const respBody = await resp.text();
      throw newAiApiError(
        `API returned status ${resp.status}: ${respBody}`,
        resp.status,
      );
    }

    return this.readStream(resp, timer);
  }

  // -------------------------------------------------------------------------
  // Internal helpers
  // -------------------------------------------------------------------------

  /** Constructs the Wenxin API URL with the access token. */
  private buildURL(accessToken: string, model: string): string {
    const endpoint = this.modelToEndpoint(model);
    return `${this.baseURL}/${endpoint}?access_token=${accessToken}`;
  }

  /** Maps a model name to its Wenxin API endpoint path. */
  private modelToEndpoint(model: string): string {
    switch (model.toLowerCase()) {
      case "ernie-4.0":
      case "ernie-4.0-8k":
      case "completions_pro":
        return "completions_pro";
      case "ernie-3.5":
      case "ernie-3.5-8k":
      case "completions":
        return "completions";
      case "ernie-speed":
      case "ernie-speed-8k":
        return "ernie_speed";
      case "ernie-lite":
      case "ernie-lite-8k":
        return "ernie_lite";
      case "ernie-bot-4":
        return "completions_pro";
      default:
        return "completions_pro";
    }
  }

  /**
   * Obtain (or return cached) OAuth2 access token. Uses client_credentials
   * grant with the API key/secret. Caches the token and schedules a refresh
   * 5 minutes before the server-reported expiry to avoid edge-case 401s.
   */
  private async getAccessToken(): Promise<string> {
    if (this.accessToken && Date.now() < this.tokenExpiry) {
      return this.accessToken;
    }

    const tokenURL = `https://aip.baidubce.com/oauth/2.0/token?grant_type=client_credentials&client_id=${this.config.apiKey}&client_secret=${this.config.apiSecret}`;

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const resp = await fetch(tokenURL, {
        method: "POST",
        signal: controller.signal,
      });

      if (!resp.ok) {
        throw newAiAuthError(`token request failed with status ${resp.status}`);
      }

      const tokenResp = (await resp.json()) as {
        access_token?: string;
        expires_in?: number;
        error?: string;
      };

      if (tokenResp.error) {
        throw newAiAuthError(`token error: ${tokenResp.error}`);
      }

      if (!tokenResp.access_token) {
        throw newAiAuthError("empty access token received");
      }

      this.accessToken = tokenResp.access_token;
      const expirySeconds = tokenResp.expires_in && tokenResp.expires_in > 0
        ? tokenResp.expires_in
        : 2592000; // 30 days default
      this.tokenExpiry = Date.now() + expirySeconds * 1000 - 5 * 60 * 1000; // 5 min buffer

      return this.accessToken;
    } catch (err) {
      if (err instanceof Error && err.name === "AbortError") {
        throw newAiAuthError("token request timed out");
      }
      throw err;
    } finally {
      clearTimeout(timer);
    }
  }

  /**
   * Parse the SSE response body into StreamEvents. Wenxin deltas carry the
   * incremental text in `result`; the stream terminates when a chunk sets
   * `is_end`. The timeout timer is cleared in `finally` so it does not leak
   * if the consumer aborts iteration early.
   */
  private async *readStream(resp: Response, timer: ReturnType<typeof setTimeout>): AsyncGenerator<StreamEvent> {
    try {
      if (!resp.body) {
        yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
        return;
      }

      const reader = resp.body.getReader();
      const payloads = await parseSSEStream(reader);

      for (const payload of payloads) {
        // Wenxin streaming returns incremental results
        let chunk: { result?: string; is_end?: boolean };
        try {
          chunk = JSON.parse(payload) as { result?: string; is_end?: boolean };
        } catch {
          continue;
        }

        if (chunk.result) {
          yield {
            type: "content_delta",
            content: chunk.result,
            tool_call_id: "",
            tool_call_name: "",
            arguments: "",
            usage: null,
          };
        }

        if (chunk.is_end) {
          yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
          return;
        }
      }

      yield { type: "done", content: "", tool_call_id: "", tool_call_name: "", arguments: "", usage: null };
    } finally {
      clearTimeout(timer);
    }
  }
}

/** JSON.stringify wrapper that converts cyclic/unserializable inputs into an AiParseError. */
function safeMarshal(obj: unknown): string {
  try {
    return JSON.stringify(obj);
  } catch (err) {
    throw newAiParseError(`failed to marshal request: ${err instanceof Error ? err.message : String(err)}`);
  }
}

/** Creates a new Wenxin provider with the given config. */
/**
 * Creates a Baidu Wenxin (ERNIE) AI provider.
 *
 * Wenxin uses a token-based authentication flow:
 * 1. Exchange API key/secret for an access token
 * 2. Use the access token for subsequent API calls
 * 3. Handle token refresh when expired
 *
 * @param config - Provider configuration with API key and secret
 * @returns Configured AiProvider for Wenxin/ERNIE models
 */
export function newWenxinProvider(config: ProviderConfig): WenxinProvider {
  return new WenxinProvider(config);
}
