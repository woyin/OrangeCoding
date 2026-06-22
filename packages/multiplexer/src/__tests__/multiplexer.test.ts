/**
 * Tests for the multiplexer package — config, transport, pane types,
 * and backend detection.
 */

import {
  defaultMultiplexerConfig,
  normalizeConfig,
} from "../config.js";
import type { MultiplexerConfig } from "../config.js";
import {
  IPCMessageType,
  socketPath,
} from "../transport.js";
import { PaneState } from "../pane.js";
import type { PaneInfo } from "../pane.js";
import { detectBackend } from "../backend.js";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

describe("MultiplexerConfig", () => {
  it("defaultMultiplexerConfig returns sensible defaults", () => {
    const cfg = defaultMultiplexerConfig();
    expect(cfg.enabled).toBe(false);
    expect(cfg.preferredBackend).toBe("auto");
    expect(cfg.commandTimeoutMs).toBe(30000);
    expect(cfg.socketDir).toBeTruthy();
  });

  it("normalizeConfig fills in missing preferredBackend", () => {
    const cfg: MultiplexerConfig = {
      ...defaultMultiplexerConfig(),
      preferredBackend: "",
    };
    const normalized = normalizeConfig(cfg);
    expect(normalized.preferredBackend).toBe("auto");
  });

  it("normalizeConfig fills in missing socketDir", () => {
    const cfg: MultiplexerConfig = {
      ...defaultMultiplexerConfig(),
      socketDir: "",
    };
    const normalized = normalizeConfig(cfg);
    expect(normalized.socketDir).toBeTruthy();
  });

  it("normalizeConfig fills in missing commandTimeoutMs", () => {
    const cfg: MultiplexerConfig = {
      ...defaultMultiplexerConfig(),
      commandTimeoutMs: 0,
    };
    const normalized = normalizeConfig(cfg);
    expect(normalized.commandTimeoutMs).toBe(30000);
  });

  it("normalizeConfig preserves explicit values", () => {
    const cfg: MultiplexerConfig = {
      enabled: true,
      preferredBackend: "tmux",
      socketDir: "/custom/path",
      commandTimeoutMs: 5000,
    };
    const normalized = normalizeConfig(cfg);
    expect(normalized.preferredBackend).toBe("tmux");
    expect(normalized.socketDir).toBe("/custom/path");
    expect(normalized.commandTimeoutMs).toBe(5000);
  });
});

// ---------------------------------------------------------------------------
// IPCMessageType constants
// ---------------------------------------------------------------------------

describe("IPCMessageType", () => {
  it("has all expected message type values", () => {
    expect(IPCMessageType.Task).toBe("task");
    expect(IPCMessageType.Result).toBe("result");
    expect(IPCMessageType.Event).toBe("event");
    expect(IPCMessageType.Keepalive).toBe("keepalive");
  });
});

// ---------------------------------------------------------------------------
// socketPath
// ---------------------------------------------------------------------------

describe("socketPath", () => {
  it("constructs a path from directory and pane ID", () => {
    const path = socketPath("/tmp/sockets", "pane-1");
    expect(path).toContain("/tmp/sockets");
    expect(path).toContain("pane-1.sock");
  });

  it("appends .sock extension", () => {
    const path = socketPath("/var/run", "abc");
    expect(path.endsWith(".sock")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// PaneState enum
// ---------------------------------------------------------------------------

describe("PaneState", () => {
  it("has all expected states", () => {
    expect(PaneState.Created).toBe("created");
    expect(PaneState.Running).toBe("running");
    expect(PaneState.Exited).toBe("exited");
    expect(PaneState.Error).toBe("error");
  });
});

// ---------------------------------------------------------------------------
// PaneInfo type shape
// ---------------------------------------------------------------------------

describe("PaneInfo", () => {
  it("can be constructed with required fields", () => {
    const info: PaneInfo = {
      id: "pane-1",
      name: "agent-1",
      state: PaneState.Running,
      createdAt: new Date(),
      backend: "tmux",
    };

    expect(info.id).toBe("pane-1");
    expect(info.name).toBe("agent-1");
    expect(info.state).toBe(PaneState.Running);
    expect(info.backend).toBe("tmux");
  });

  it("allows optional pid field", () => {
    const info: PaneInfo = {
      id: "pane-2",
      name: "agent-2",
      state: PaneState.Created,
      createdAt: new Date(),
      backend: "zellij",
      pid: 12345,
    };

    expect(info.pid).toBe(12345);
  });
});

// ---------------------------------------------------------------------------
// detectBackend
// ---------------------------------------------------------------------------

describe("detectBackend", () => {
  it("returns null when no multiplexer is available", () => {
    // In test environment, neither tmux nor zellij should be running
    // (unless the test happens to run inside one)
    const original_tmux = process.env.TMUX;
    const original_zellij = process.env.ZELLIJ_SESSION_NAME;

    delete process.env.TMUX;
    delete process.env.ZELLIJ_SESSION_NAME;

    try {
      const backend = detectBackend();
      expect(backend).toBeNull();
    } finally {
      // Restore env
      if (original_tmux !== undefined) process.env.TMUX = original_tmux;
      if (original_zellij !== undefined) process.env.ZELLIJ_SESSION_NAME = original_zellij;
    }
  });
});

// ---------------------------------------------------------------------------
// TmuxBackend — availability and basic methods
// ---------------------------------------------------------------------------

import { TmuxBackend } from "../tmux.js";
import { ZellijBackend } from "../zellij.js";
import { PaneManager } from "../manager.js";

describe("TmuxBackend", () => {
  const originalTmux = process.env.TMUX;

  afterEach(() => {
    if (originalTmux !== undefined) {
      process.env.TMUX = originalTmux;
    } else {
      delete process.env.TMUX;
    }
  });

  it("name returns 'tmux'", () => {
    const backend = new TmuxBackend();
    expect(backend.name()).toBe("tmux");
  });

  it("isAvailable returns true when TMUX env is set", () => {
    process.env.TMUX = "/tmp/tmux-1000/default,12345,0";
    const backend = new TmuxBackend();
    expect(backend.isAvailable()).toBe(true);
  });

  it("isAvailable returns false when TMUX env is not set", () => {
    delete process.env.TMUX;
    const backend = new TmuxBackend();
    expect(backend.isAvailable()).toBe(false);
  });

  it("listPanes returns empty initially", async () => {
    const backend = new TmuxBackend();
    const panes = await backend.listPanes();
    expect(panes).toHaveLength(0);
  });
});

describe("ZellijBackend", () => {
  const originalZellij = process.env.ZELLIJ_SESSION_NAME;

  afterEach(() => {
    if (originalZellij !== undefined) {
      process.env.ZELLIJ_SESSION_NAME = originalZellij;
    } else {
      delete process.env.ZELLIJ_SESSION_NAME;
    }
  });

  it("name returns 'zellij'", () => {
    const backend = new ZellijBackend();
    expect(backend.name()).toBe("zellij");
  });

  it("isAvailable returns true when ZELLIJ_SESSION_NAME is set", () => {
    process.env.ZELLIJ_SESSION_NAME = "session-123";
    const backend = new ZellijBackend();
    expect(backend.isAvailable()).toBe(true);
  });

  it("isAvailable returns false when ZELLIJ_SESSION_NAME is not set", () => {
    delete process.env.ZELLIJ_SESSION_NAME;
    const backend = new ZellijBackend();
    expect(backend.isAvailable()).toBe(false);
  });

  it("listPanes returns empty initially", async () => {
    const backend = new ZellijBackend();
    const panes = await backend.listPanes();
    expect(panes).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// PaneManager — basic operations
// ---------------------------------------------------------------------------

describe("PaneManager", () => {
  it("activePanes returns empty initially", () => {
    const mgr = new PaneManager(null, defaultMultiplexerConfig());
    expect(mgr.activePanes()).toHaveLength(0);
  });

  it("spawnAgentPane throws when no backend", async () => {
    const mgr = new PaneManager(null, defaultMultiplexerConfig());
    await expect(mgr.spawnAgentPane("agent-1", "task")).rejects.toThrow("no multiplexer backend");
  });

  it("closePane is a no-op for non-existent pane", async () => {
    const mgr = new PaneManager(null, defaultMultiplexerConfig());
    await expect(mgr.closePane("nonexistent")).resolves.toBeUndefined();
  });

  it("closeAll is a no-op when no panes", async () => {
    const mgr = new PaneManager(null, defaultMultiplexerConfig());
    await expect(mgr.closeAll()).resolves.toBeUndefined();
  });
});
