# OrangeCoding — Copilot Instructions

## Build, Test, Lint

```bash
# Full workspace
cargo check --workspace        # Compile check
cargo test --workspace         # Run all ~1960 tests
cargo fmt --all -- --check     # Format check
cargo clippy --workspace --all-targets  # Lint

# Single crate
cargo test -p orangecoding-tui
cargo test -p orangecoding-agent

# Single test by name
cargo test -p orangecoding-tui -- 测试选择mode进入模式子菜单
cargo test -p orangecoding-tui -- test_name_here

# Integration tests (invariant tests at workspace root)
cargo test --test session_invariants
cargo test --test auth_invariants
```

CI runs: `fmt → clippy → test → check`, then builds for 4 targets (x86_64/aarch64 × linux/macos).

## Architecture

15-crate Rust workspace for a terminal AI coding agent. The dependency flow is:

```
orangecoding-cli (binary entry point)
├── orangecoding-tui          (ratatui terminal UI)
├── orangecoding-ai           (provider abstraction: OpenAI, Anthropic, DeepSeek, Qianwen, Wenxin)
├── orangecoding-agent        (agent loop, workflows, context governance)
│   ├── workflows/autopilot   (Plan → Execute → Verify → Replan cycle)
│   ├── workflows/boulder     (session continuity across interruptions)
│   └── auto_compact          (context compaction with circuit breaker)
├── orangecoding-config       (layered config + models.yml provider catalog)
├── orangecoding-session      (JSONL tree storage with branches and blob store)
├── orangecoding-tools        (22+ tools with permission system)
├── orangecoding-mcp          (Model Context Protocol: JSON-RPC over stdio/SSE)
├── orangecoding-control-server (HTTP + WebSocket API, axum 0.7)
├── orangecoding-worker       (agent lifecycle management)
├── orangecoding-control-protocol (shared message types)
├── orangecoding-mesh         (multi-agent communication)
├── orangecoding-audit        (security logging, data masking)
├── orangecoding-invariant    (property-based test helpers)
└── orangecoding-core         (OrangeError, Result<T>, event types, TokenUsage)
```

**Key integration points:**
- `orangecoding-cli/src/commands/launch.rs` — TUI event loop, provider init, slash command dispatch
- `orangecoding-ai/src/provider.rs` — `AiProvider` async trait all providers implement
- `orangecoding-config/src/models_config.rs` — `ModelsConfig` with predefined z.ai/zen providers and `merge_with_predefined()`
- `orangecoding-tui/src/app.rs` — Core state: `AppMode` (Normal/Input/Command/Help), `InteractionMode` (Normal/Plan/Autopilot/UltraWork), `CommandMenuState` for interactive menus

## Conventions

### Error handling
Use `OrangeError` (defined via `thiserror`) with the workspace `Result<T>` alias from `orangecoding-core`. Ten variants: Config, Io, Network, Ai, Agent, Tool, Protocol, Serialization, Auth, Internal. Construct via helpers like `OrangeError::config("msg")`.

### Config paths
Config lives at `~/.config/orangecoding/` (resolved via `dirs::home_dir().join(".config/orangecoding")`). Do **not** use `dirs::config_dir()` — that gives `~/Library/Application Support` on macOS.

### Test naming
Unit tests use **Chinese function names** (e.g., `fn 测试应用初始状态()`). Integration/invariant tests in `tests/invariants/` use English with prefix `inv_` (e.g., `inv_session_01_state_persists_across_updates`).

### Float sorting
Always use `partial_cmp().unwrap_or(std::cmp::Ordering::Equal)` to avoid NaN panics. See `memory.rs` and `tool_summary.rs` for examples.

### Axum routing
Axum 0.7 with **`:id`** path parameter syntax (not `{id}` which is 0.8+). Example: `.route("/sessions/:id", get(handler))`.

### TUI command menus
`CommandMenuKind` has four variants: `Slash`, `Model`, `Mode`, `Think`. When a slash command has sub-options (like `/mode`, `/think`), open a sub-menu via `app.open_mode_menu()` / `app.open_think_menu()` instead of printing a text message.

### Provider auto-config
Known providers (z.ai, opencode zen) have predefined configs in `ModelsConfig::predefined_provider_config()`. Users only need to supply an `api_key` in `~/.config/orangecoding/models.yml` — base_url, api type, auth, and model list are auto-filled via `merge_with_predefined()`.

### Comments and docs
Code comments and doc-strings are written in **Chinese**. Public API docs use `///` with Chinese descriptions.
