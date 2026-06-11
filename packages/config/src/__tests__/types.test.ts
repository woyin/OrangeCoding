import { validateConfig } from "../types.js";
import type { OrangeConfig } from "../types.js";

function makeConfig(overrides: Partial<OrangeConfig> = {}): OrangeConfig {
  return {
    log_level: "info",
    default_provider: "openai",
    default_model: "",
    control_port: 3200,
    providers: {},
    hooks: {},
    permissions: {},
    harness: {
      checkpoint_store: "memory",
      checkpoint_dir: "checkpoints",
      reasoning_effort: "high",
      reasoning_budget_tokens: 4096,
    },
    multiplexer: {
      enabled: false,
      preferred_backend: "auto",
      socket_dir: "",
      command_timeout_ms: 30000,
    },
    audit: {
      enabled: true,
      dir: "audit",
    },
    ...overrides,
  };
}

describe("validateConfig", () => {
  it("accepts a valid configuration", () => {
    expect(() => validateConfig(makeConfig())).not.toThrow();
  });

  it("rejects control_port out of range", () => {
    expect(() => validateConfig(makeConfig({ control_port: -1 }))).toThrow("control_port");
    expect(() => validateConfig(makeConfig({ control_port: 70000 }))).toThrow("control_port");
  });

  it("accepts control_port 0", () => {
    expect(() => validateConfig(makeConfig({ control_port: 0 }))).not.toThrow();
  });

  it("accepts control_port 65535", () => {
    expect(() => validateConfig(makeConfig({ control_port: 65535 }))).not.toThrow();
  });

  it("rejects invalid checkpoint_store", () => {
    expect(() =>
      validateConfig(
        makeConfig({ harness: { ...makeConfig().harness, checkpoint_store: "redis" } }),
      ),
    ).toThrow("checkpoint_store");
  });

  it("accepts empty checkpoint_store", () => {
    expect(() =>
      validateConfig(
        makeConfig({ harness: { ...makeConfig().harness, checkpoint_store: "" } }),
      ),
    ).not.toThrow();
  });

  it("accepts valid checkpoint_store values", () => {
    for (const store of ["memory", "file"]) {
      expect(() =>
        validateConfig(
          makeConfig({ harness: { ...makeConfig().harness, checkpoint_store: store } }),
        ),
      ).not.toThrow();
    }
  });

  it("rejects invalid preferred_backend", () => {
    expect(() =>
      validateConfig(
        makeConfig({ multiplexer: { ...makeConfig().multiplexer, preferred_backend: "screen" } }),
      ),
    ).toThrow("preferred_backend");
  });

  it("accepts valid preferred_backend values", () => {
    for (const backend of ["", "auto", "zellij", "tmux"]) {
      expect(() =>
        validateConfig(
          makeConfig({ multiplexer: { ...makeConfig().multiplexer, preferred_backend: backend } }),
        ),
      ).not.toThrow();
    }
  });
});
