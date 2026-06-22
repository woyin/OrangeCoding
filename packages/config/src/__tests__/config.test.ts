/**
 * Tests for the config module — defaultConfig, ConfigManager,
 * and normalization functions.
 */

import { defaultConfig, ConfigManager } from "../config.js";
import type { OrangeConfig } from "../types.js";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";

// ---------------------------------------------------------------------------
// defaultConfig
// ---------------------------------------------------------------------------

describe("defaultConfig", () => {
  it("returns a valid configuration", () => {
    const cfg = defaultConfig();
    expect(cfg.log_level).toBe("info");
    expect(cfg.default_provider).toBe("openai");
    expect(cfg.default_model).toBe("");
    expect(cfg.control_port).toBe(3200);
  });

  it("has sensible harness defaults", () => {
    const cfg = defaultConfig();
    expect(cfg.harness.checkpoint_store).toBe("memory");
    expect(cfg.harness.reasoning_effort).toBe("high");
    expect(cfg.harness.reasoning_budget_tokens).toBe(4096);
  });

  it("has sensible multiplexer defaults", () => {
    const cfg = defaultConfig();
    expect(cfg.multiplexer.enabled).toBe(false);
    expect(cfg.multiplexer.preferred_backend).toBe("auto");
    expect(cfg.multiplexer.command_timeout_ms).toBe(30000);
  });

  it("has audit enabled by default", () => {
    const cfg = defaultConfig();
    expect(cfg.audit.enabled).toBe(true);
    expect(cfg.audit.dir).toBe("audit");
  });

  it("has empty providers and hooks", () => {
    const cfg = defaultConfig();
    expect(Object.keys(cfg.providers)).toHaveLength(0);
    expect(cfg.hooks.pre_tool_call).toBeUndefined();
    expect(cfg.hooks.post_tool_call).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// ConfigManager — load/save roundtrip
// ---------------------------------------------------------------------------

describe("ConfigManager", () => {
  let tmpDir: string;
  let configPath: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "oc-config-test-"));
    configPath = path.join(tmpDir, "config.json");
  });

  afterEach(() => {
    try { fs.rmSync(tmpDir, { recursive: true }); } catch { /* ignore */ }
  });

  it("saves and loads a configuration", () => {
    const mgr = new ConfigManager();
    const cfg = defaultConfig();
    cfg.default_model = "gpt-5.1";

    mgr.save(configPath, cfg);
    const loaded = mgr.load(configPath);

    expect(loaded.default_model).toBe("gpt-5.1");
    expect(loaded.default_provider).toBe("openai");
  });

  it("loads JSONC files with comments", () => {
    const jsonc = `{
      // This is a comment
      "log_level": "debug",
      "default_provider": "anthropic",
      "default_model": "claude-opus-4-7",
      "control_port": 4000,
      "providers": {},
      "hooks": {},
      "permissions": {},
      "harness": {
        "checkpoint_store": "memory",
        "checkpoint_dir": "cp",
        "reasoning_effort": "high",
        "reasoning_budget_tokens": 2048
      },
      "multiplexer": {
        "enabled": false,
        "preferred_backend": "auto",
        "socket_dir": "",
        "command_timeout_ms": 10000
      },
      "audit": {
        "enabled": true,
        "dir": "audit"
      }
    }`;
    fs.writeFileSync(configPath, jsonc, "utf-8");

    const mgr = new ConfigManager();
    const cfg = mgr.load(configPath);

    expect(cfg.log_level).toBe("debug");
    expect(cfg.default_provider).toBe("anthropic");
    expect(cfg.control_port).toBe(4000);
  });

  it("get retrieves a top-level field", () => {
    const mgr = new ConfigManager();
    mgr.save(configPath, defaultConfig());

    const value = mgr.get(configPath, "log_level");
    expect(value).toBe("info");
  });

  it("get retrieves a nested field via dot path", () => {
    const mgr = new ConfigManager();
    mgr.save(configPath, defaultConfig());

    const value = mgr.get(configPath, "harness.reasoning_effort");
    expect(value).toBe("high");
  });

  it("set updates a field and persists", () => {
    const mgr = new ConfigManager();
    mgr.save(configPath, defaultConfig());

    mgr.set(configPath, "default_model", "new-model");

    const loaded = mgr.load(configPath);
    expect(loaded.default_model).toBe("new-model");
  });

  it("throws on non-existent config file", () => {
    const mgr = new ConfigManager();
    expect(() => mgr.load("/nonexistent/path.json")).toThrow();
  });

  it("creates parent directories on save", () => {
    const mgr = new ConfigManager();
    const deepPath = path.join(tmpDir, "a", "b", "c", "config.json");
    mgr.save(deepPath, defaultConfig());

    expect(fs.existsSync(deepPath)).toBe(true);
  });
});
