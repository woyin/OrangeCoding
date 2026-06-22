/**
 * Tests for the ai package core modules — error types, model router,
 * provider factory, and provider normalization.
 */

import { AiError, AiErrorKind, newAiNetworkError, newAiApiError, newAiAuthError, newAiParseError, newAiStreamError, newAiConfigError, newAiUnsupportedProviderError, newAiRateLimitError, newAiTimeoutError } from "../error.js";
import { ModelRouter, ModelCategory, createOmORouter } from "../router.js";
import type { RoutingRule } from "../router.js";
import {
  ProviderFactory,
  providerTimeout,
  defaultModelForProvider,
  defaultBaseURLForProvider,
  normalizeProviderConfig,
} from "../provider.js";
import type { ProviderConfig } from "../provider.js";

// ---------------------------------------------------------------------------
// AiError
// ---------------------------------------------------------------------------

describe("AiError", () => {
  it("creates an error with kind and message", () => {
    const err = new AiError(AiErrorKind.Network, "connection refused");
    expect(err).toBeInstanceOf(Error);
    expect(err.name).toBe("AiError");
    expect(err.kind).toBe(AiErrorKind.Network);
    expect(err.message).toContain("connection refused");
    expect(err.statusCode).toBe(0);
    expect(err.retryAfter).toBe(0);
  });

  it("supports statusCode and retryAfter", () => {
    const err = new AiError(AiErrorKind.RateLimit, "too many requests", 429, 30);
    expect(err.statusCode).toBe(429);
    expect(err.retryAfter).toBe(30);
  });

  it("isRetryable returns true for network, rate-limit, and timeout errors", () => {
    expect(new AiError(AiErrorKind.Network, "").isRetryable()).toBe(true);
    expect(new AiError(AiErrorKind.RateLimit, "").isRetryable()).toBe(true);
    expect(new AiError(AiErrorKind.Timeout, "").isRetryable()).toBe(true);
  });

  it("isRetryable returns false for auth, parse, and config errors", () => {
    expect(new AiError(AiErrorKind.Auth, "").isRetryable()).toBe(false);
    expect(new AiError(AiErrorKind.Parse, "").isRetryable()).toBe(false);
    expect(new AiError(AiErrorKind.Config, "").isRetryable()).toBe(false);
    expect(new AiError(AiErrorKind.Api, "").isRetryable()).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

describe("AiError convenience constructors", () => {
  it("newAiNetworkError creates a Network error", () => {
    const err = newAiNetworkError("timeout");
    expect(err.kind).toBe(AiErrorKind.Network);
    expect(err.isRetryable()).toBe(true);
  });

  it("newAiApiError creates an API error with status code", () => {
    const err = newAiApiError("bad request", 400);
    expect(err.kind).toBe(AiErrorKind.Api);
    expect(err.statusCode).toBe(400);
  });

  it("newAiAuthError creates an Auth error", () => {
    expect(newAiAuthError("invalid key").kind).toBe(AiErrorKind.Auth);
  });

  it("newAiParseError creates a Parse error", () => {
    expect(newAiParseError("bad json").kind).toBe(AiErrorKind.Parse);
  });

  it("newAiStreamError creates a Stream error", () => {
    expect(newAiStreamError("broken pipe").kind).toBe(AiErrorKind.Stream);
  });

  it("newAiConfigError creates a Config error", () => {
    expect(newAiConfigError("missing key").kind).toBe(AiErrorKind.Config);
  });

  it("newAiUnsupportedProviderError creates an UnsupportedProvider error", () => {
    expect(newAiUnsupportedProviderError("xyz").kind).toBe(AiErrorKind.UnsupportedProvider);
  });

  it("newAiRateLimitError creates a RateLimit error with retryAfter", () => {
    const err = newAiRateLimitError("slow down", 60);
    expect(err.kind).toBe(AiErrorKind.RateLimit);
    expect(err.retryAfter).toBe(60);
    expect(err.isRetryable()).toBe(true);
  });

  it("newAiTimeoutError creates a Timeout error", () => {
    const err = newAiTimeoutError("timed out");
    expect(err.kind).toBe(AiErrorKind.Timeout);
    expect(err.isRetryable()).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// AiErrorKind enum
// ---------------------------------------------------------------------------

describe("AiErrorKind", () => {
  it("has all expected values", () => {
    expect(AiErrorKind.Network).toBe("network");
    expect(AiErrorKind.Api).toBe("api");
    expect(AiErrorKind.Auth).toBe("auth");
    expect(AiErrorKind.Parse).toBe("parse");
    expect(AiErrorKind.Stream).toBe("stream");
    expect(AiErrorKind.Config).toBe("config");
    expect(AiErrorKind.UnsupportedProvider).toBe("unsupported-provider");
    expect(AiErrorKind.RateLimit).toBe("rate-limit");
    expect(AiErrorKind.Timeout).toBe("timeout");
  });
});

// ---------------------------------------------------------------------------
// ModelRouter
// ---------------------------------------------------------------------------

describe("ModelRouter", () => {
  it("routes to the matching rule", () => {
    const rules: RoutingRule[] = [
      { category: ModelCategory.Coding, provider: "anthropic", model: "claude-opus-4-7" },
      { category: ModelCategory.Review, provider: "openai", model: "gpt-5.1" },
    ];
    const router = new ModelRouter(rules);

    const coding = router.route(ModelCategory.Coding);
    expect(coding.provider).toBe("anthropic");
    expect(coding.model).toBe("claude-opus-4-7");

    const review = router.route(ModelCategory.Review);
    expect(review.provider).toBe("openai");
    expect(review.model).toBe("gpt-5.1");
  });

  it("falls back to the first rule when no match found", () => {
    const rules: RoutingRule[] = [
      { category: ModelCategory.General, provider: "openai", model: "gpt-4" },
    ];
    const router = new ModelRouter(rules);

    const result = router.route(ModelCategory.Coding);
    expect(result.provider).toBe("openai");
    expect(result.model).toBe("gpt-4");
  });

  it("uses built-in default when no rules provided", () => {
    const router = new ModelRouter([]);
    const result = router.route(ModelCategory.Coding);
    expect(result.provider).toBe("openai");
    expect(result.model).toBe("gpt-4");
  });
});

// ---------------------------------------------------------------------------
// ModelCategory enum
// ---------------------------------------------------------------------------

describe("ModelCategory", () => {
  it("has standard categories", () => {
    expect(ModelCategory.Coding).toBe("coding");
    expect(ModelCategory.Planning).toBe("planning");
    expect(ModelCategory.Review).toBe("review");
    expect(ModelCategory.General).toBe("general");
  });

  it("has OmO-style intent categories", () => {
    expect(ModelCategory.Quick).toBe("quick");
    expect(ModelCategory.Deep).toBe("deep");
    expect(ModelCategory.Visual).toBe("visual");
    expect(ModelCategory.Ultrabrain).toBe("ultrabrain");
  });
});

// ---------------------------------------------------------------------------
// createOmORouter
// ---------------------------------------------------------------------------

describe("createOmORouter", () => {
  it("creates a router with default OmO mappings", () => {
    const router = createOmORouter();
    const quick = router.route(ModelCategory.Quick);
    expect(quick.provider).toBeTruthy();
    expect(quick.model).toBeTruthy();
  });

  it("applies overrides to default mappings", () => {
    const router = createOmORouter({
      [ModelCategory.Quick]: { provider: "custom", model: "custom-model" },
    });
    const quick = router.route(ModelCategory.Quick);
    expect(quick.provider).toBe("custom");
    expect(quick.model).toBe("custom-model");
  });
});

// ---------------------------------------------------------------------------
// Provider factory and normalization
// ---------------------------------------------------------------------------

describe("ProviderFactory", () => {
  const factory = new ProviderFactory();

  it("creates an OpenAI-compatible provider for 'openai'", () => {
    const provider = factory.createProvider("openai", {
      apiKey: "test",
      apiSecret: "",
      baseURL: "",
      defaultModel: "",
      timeoutSecs: 0,
      extra: {},
    });
    expect(provider.name()).toBeTruthy();
  });

  it("creates an Anthropic provider for 'anthropic'", () => {
    const provider = factory.createProvider("anthropic", {
      apiKey: "test",
      apiSecret: "",
      baseURL: "",
      defaultModel: "",
      timeoutSecs: 0,
      extra: {},
    });
    expect(provider.name()).toBeTruthy();
  });

  it("throws for unknown provider", () => {
    expect(() =>
      factory.createProvider("unknown-provider", {
        apiKey: "",
        apiSecret: "",
        baseURL: "",
        defaultModel: "",
        timeoutSecs: 0,
        extra: {},
      }),
    ).toThrow();
  });

  it("normalizes provider name case-insensitively", () => {
    const provider = factory.createProvider("OpenAI", {
      apiKey: "test",
      apiSecret: "",
      baseURL: "",
      defaultModel: "",
      timeoutSecs: 0,
      extra: {},
    });
    expect(provider).toBeDefined();
  });
});

describe("providerTimeout", () => {
  it("returns configured timeout in milliseconds", () => {
    const cfg: ProviderConfig = {
      apiKey: "", apiSecret: "", baseURL: "", defaultModel: "",
      timeoutSecs: 30, extra: {},
    };
    expect(providerTimeout(cfg)).toBe(30_000);
  });

  it("returns default 120s when timeoutSecs is 0", () => {
    const cfg: ProviderConfig = {
      apiKey: "", apiSecret: "", baseURL: "", defaultModel: "",
      timeoutSecs: 0, extra: {},
    };
    expect(providerTimeout(cfg)).toBe(120_000);
  });
});

describe("defaultModelForProvider", () => {
  it("returns GPT model for openai", () => {
    expect(defaultModelForProvider("openai")).toContain("gpt");
  });

  it("returns Claude model for anthropic", () => {
    expect(defaultModelForProvider("anthropic")).toContain("claude");
  });

  it("returns empty string for unknown provider", () => {
    expect(defaultModelForProvider("unknown")).toBe("");
  });
});

describe("defaultBaseURLForProvider", () => {
  it("returns moonshot URL for kimi", () => {
    expect(defaultBaseURLForProvider("kimi")).toContain("moonshot");
  });

  it("returns bigmodel URL for glm", () => {
    expect(defaultBaseURLForProvider("glm")).toContain("z.ai");
  });

  it("returns empty string for openai (no default URL needed)", () => {
    expect(defaultBaseURLForProvider("openai")).toBe("");
  });
});

describe("normalizeProviderConfig", () => {
  it("fills in default model when empty", () => {
    const cfg: ProviderConfig = {
      apiKey: "key", apiSecret: "", baseURL: "", defaultModel: "",
      timeoutSecs: 0, extra: {},
    };
    const result = normalizeProviderConfig("openai", cfg);
    expect(result.defaultModel).toContain("gpt");
  });

  it("fills in default base URL when empty", () => {
    const cfg: ProviderConfig = {
      apiKey: "key", apiSecret: "", baseURL: "", defaultModel: "",
      timeoutSecs: 0, extra: {},
    };
    const result = normalizeProviderConfig("kimi", cfg);
    expect(result.baseURL).toContain("moonshot");
  });

  it("adds reasoning_format for thinking-compatible providers", () => {
    const cfg: ProviderConfig = {
      apiKey: "key", apiSecret: "", baseURL: "", defaultModel: "",
      timeoutSecs: 0, extra: {},
    };
    const result = normalizeProviderConfig("kimi", cfg);
    expect(result.extra["reasoning_format"]).toBe("thinking");
  });

  it("preserves explicit model and baseURL", () => {
    const cfg: ProviderConfig = {
      apiKey: "key", apiSecret: "", baseURL: "https://custom.api.com",
      defaultModel: "custom-model", timeoutSecs: 0, extra: {},
    };
    const result = normalizeProviderConfig("openai", cfg);
    expect(result.defaultModel).toBe("custom-model");
    expect(result.baseURL).toBe("https://custom.api.com");
  });
});
