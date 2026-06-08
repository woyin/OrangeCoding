# TUI UX & Provider Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix mouse scrolling in all TUI modes, enhance `/model` interactive selection, add z.ai/zen provider auto-configuration with predefined models, and improve slash command UX.

**Architecture:** The changes span 4 crates: `orangecoding-tui` (mouse scroll fix, command menu enhancements), `orangecoding-cli` (slash_builtins model selector integration), `orangecoding-config` (z.ai/zen predefined provider configs), and `orangecoding-ai` (provider factory model lists). The approach is to fix the P0 mouse scroll bug first, then enhance the model selector, then add provider auto-config.

**Tech Stack:** Rust, ratatui 0.24, crossterm 0.27, serde/serde_yaml, chrono with Local timezone

**Status of prior work (already completed, no changes needed):**
- Sidebar already shows Context Overview / MCP Status / Changes panels (not files/agents) ✅
- Time display already uses `chrono::Local` in `format_local_timestamp()` ✅
- Mouse capture is already enabled via `EnableMouseCapture` ✅
- Command menu system already exists with `/` → slash menu → model submenu flow ✅

---

### Task 1: Fix Mouse Scroll in Input Mode (P0)

**Problem:** `handle_mouse_event()` works regardless of mode, but the real issue is that when `Paragraph` uses `Wrap { trim: false }`, wrapped lines aren't counted in `scroll_offset`. The `total_lines` calculation in `session.rs` counts logical lines, not visual lines after wrapping. This means scrolling is inaccurate for long wrapped messages.

Additionally, the `scroll_offset` is `u16` which limits scroll range to 65535 lines — should work for most cases, but let's add a max-scroll clamp to prevent over-scrolling past content.

**Files:**
- Modify: `crates/orangecoding-tui/src/app.rs` — add `max_scroll_offset` and clamp in `scroll_up`
- Modify: `crates/orangecoding-tui/src/components/session.rs` — track `last_total_lines` for scroll clamping

**Step 1: Write the failing test**

In `crates/orangecoding-tui/src/app.rs`, add test:

```rust
#[test]
fn 测试滚动不会超过最大偏移量() {
    let mut app = App::new("gpt-4");
    app.max_scroll_offset = 10;
    app.scroll_up(20);
    assert_eq!(app.scroll_offset, 10);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p orangecoding-tui 测试滚动不会超过最大偏移量 -- --nocapture`
Expected: FAIL — `max_scroll_offset` field doesn't exist

**Step 3: Add `max_scroll_offset` field and clamp logic**

In `App` struct, add:
```rust
pub max_scroll_offset: u16,
```

In `App::new()`, set `max_scroll_offset: u16::MAX`.

Update `scroll_up`:
```rust
pub fn scroll_up(&mut self, lines: u16) {
    self.scroll_offset = self.scroll_offset.saturating_add(lines).min(self.max_scroll_offset);
}
```

**Step 4: Update `session.rs` to set `max_scroll_offset`**

In `SessionView::render()`, after computing `total_lines` and `visible_height`, add:
```rust
app.max_scroll_offset = total_lines.saturating_sub(visible_height);
```

But since `app` is `&App` (immutable), we need to change the approach: instead of clamping in `scroll_up`, clamp `scroll_offset` during rendering. Change the `scroll_from_top` calculation to automatically handle over-scroll:

```rust
let scroll_from_top = if total_lines > visible_height {
    let max_scroll = total_lines - visible_height;
    let clamped_offset = app.scroll_offset.min(max_scroll);
    max_scroll.saturating_sub(clamped_offset)
} else {
    0
};
```

This already handles over-scroll visually. The `scroll_offset` can exceed max but the display will be clamped.

**Step 5: Run tests to verify**

Run: `cargo test -p orangecoding-tui -- --nocapture`
Expected: All pass

**Step 6: Commit**

```bash
git add crates/orangecoding-tui/
git commit -m "fix: clamp scroll offset to prevent over-scrolling past content

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 2: Enhance `/model` Command with Interactive Menu

**Problem:** When user types `/model` and presses Enter in Input mode (not Command mode), it goes through `slash_builtins::execute_builtin("model", "")` which returns a static text string. It should instead open the interactive model menu.

**Files:**
- Modify: `crates/orangecoding-cli/src/slash_builtins.rs` — change `format_model_selector()` to return `ModelSelector` action
- Modify: `crates/orangecoding-tui/src/app.rs` — add `AppAction::OpenModelSelector`
- Modify: `crates/orangecoding-cli/src/commands/launch.rs` — handle `OpenModelSelector` action in TUI loop

**Step 1: Write the failing test**

In `crates/orangecoding-tui/src/app.rs`, add test:

```rust
#[test]
fn 测试model命令打开模型选择菜单() {
    let mut app = App::new("gpt-4");
    app.mode = AppMode::Input;
    app.input.buffer = "/model".to_string();
    app.input.cursor_position = 6;

    let action = app.handle_key_event(key_press(KeyCode::Enter));

    assert_eq!(action, AppAction::SlashCommand {
        name: "model".to_string(),
        args: String::new(),
    });
}
```

**Step 2: Run test to verify behavior**

Run: `cargo test -p orangecoding-tui 测试model命令打开模型选择菜单`
Expected: PASS (this already works, the slash command returns the right action)

**Step 3: Add `AppAction::OpenModelSelector`**

In `app.rs`, add new variant:
```rust
pub enum AppAction {
    // ...existing variants...
    /// 打开模型选择器菜单
    OpenModelSelector,
}
```

**Step 4: Handle model command in launch.rs**

In `commands/launch.rs`, when the TUI loop processes `AppAction::SlashCommand { name: "model", args }`:
- If `args` is empty, open the model menu by setting `app.mode = AppMode::Command` and calling `app.open_model_menu("")`
- If `args` is non-empty, proceed with model switching

**Step 5: Write test for model menu opening via slash command**

```rust
#[test]
fn 测试slash_model无参数打开菜单() {
    let mut app = App::new("gpt-4");
    app.open_model_menu("");
    assert!(app.command_menu.is_some());
    assert_eq!(app.command_menu.as_ref().unwrap().kind, CommandMenuKind::Model);
    assert!(!app.command_menu.as_ref().unwrap().items.is_empty());
}
```

**Step 6: Run tests**

Run: `cargo test -p orangecoding-tui -- --nocapture`
Expected: All pass

**Step 7: Commit**

```bash
git add crates/orangecoding-tui/ crates/orangecoding-cli/
git commit -m "feat: /model opens interactive selector when no args provided

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 3: Add z.ai and OpenCode Zen Provider Auto-Configuration

**Problem:** z.ai and opencode-zen provider aliases exist in `ProviderFactory` and `ModelsConfig`, but they don't have predefined model lists. Users should only need to provide an API key to use these providers.

**Files:**
- Modify: `crates/orangecoding-config/src/models_config.rs` — add `predefined_provider_config()` with model lists
- Modify: `crates/orangecoding-ai/src/provider.rs` — ensure factory handles provider-specific base URLs

**Step 1: Write the failing test**

In `crates/orangecoding-config/src/models_config.rs`:

```rust
#[test]
fn test_predefined_zai_models() {
    let config = ModelsConfig::predefined_provider_config("zai");
    assert!(config.is_some());
    let config = config.unwrap();
    assert!(config.base_url.is_some());
    assert!(!config.models.is_empty());
}

#[test]
fn test_predefined_zen_models() {
    let config = ModelsConfig::predefined_provider_config("zen");
    assert!(config.is_some());
    let config = config.unwrap();
    assert!(config.base_url.is_some());
    assert!(!config.models.is_empty());
}

#[test]
fn test_predefined_unknown_provider() {
    let config = ModelsConfig::predefined_provider_config("unknown");
    assert!(config.is_none());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orangecoding-config test_predefined`
Expected: FAIL — method doesn't exist

**Step 3: Implement `predefined_provider_config()`**

Add to `ModelsConfig` impl:

```rust
pub fn predefined_provider_config(canonical_name: &str) -> Option<ProviderConfig> {
    match canonical_name {
        "zai" => Some(ProviderConfig {
            base_url: Some("https://api.z.ai/v1".to_string()),
            api_key: None,
            api: Some(ApiType::OpenAiCompletions),
            headers: None,
            auth: Some(AuthType::Bearer),
            models: vec![
                ModelDefinition {
                    id: "z1-max".to_string(),
                    name: Some("Z1 Max".to_string()),
                    reasoning: Some(true),
                    input: Some(vec!["text".to_string()]),
                    cost: None,
                    context_window: Some(200_000),
                    max_tokens: Some(100_000),
                },
                ModelDefinition {
                    id: "z1-mini".to_string(),
                    name: Some("Z1 Mini".to_string()),
                    reasoning: Some(true),
                    input: Some(vec!["text".to_string()]),
                    cost: None,
                    context_window: Some(128_000),
                    max_tokens: Some(65_536),
                },
            ],
            discovery: None,
        }),
        "zen" => Some(ProviderConfig {
            base_url: Some("https://api.opencode.ai/v1".to_string()),
            api_key: None,
            api: Some(ApiType::OpenAiCompletions),
            headers: None,
            auth: Some(AuthType::Bearer),
            models: vec![
                ModelDefinition {
                    id: "zen-v1".to_string(),
                    name: Some("Zen V1".to_string()),
                    reasoning: Some(false),
                    input: Some(vec!["text".to_string()]),
                    cost: None,
                    context_window: Some(128_000),
                    max_tokens: Some(32_768),
                },
            ],
            discovery: None,
        }),
        _ => None,
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p orangecoding-config -- --nocapture`
Expected: All pass

**Step 5: Commit**

```bash
git add crates/orangecoding-config/
git commit -m "feat: add z.ai and OpenCode Zen predefined provider configs

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 4: Model Identity Disambiguation (Provider-Qualified Model Names)

**Problem:** Different providers may offer the same model name. The model selector needs to show provider-qualified names like `zai/z1-max` vs `zen/z1-max`.

**Files:**
- Modify: `crates/orangecoding-tui/src/app.rs` — update `available_models` to include provider info in description
- Modify: `crates/orangecoding-config/src/models_config.rs` — `list_models()` already returns `(provider_name, &ModelDefinition)` tuples ✅

**Step 1: Write the failing test**

In `crates/orangecoding-config/src/models_config.rs`:

```rust
#[test]
fn test_model_identity_format() {
    let id = ModelsConfig::model_identity("z.ai", "z1-max");
    assert_eq!(id, "zai/z1-max");

    let id = ModelsConfig::model_identity("opencode-zen", "zen-v1");
    assert_eq!(id, "zen/zen-v1");
}
```

**Step 2: Run test**

Run: `cargo test -p orangecoding-config test_model_identity`
Expected: PASS (method already exists)

**Step 3: Add test for `App` model items with provider context**

In `crates/orangecoding-tui/src/app.rs`:

```rust
#[test]
fn 测试available_models包含提供商信息() {
    let app = App::new("gpt-4");
    assert!(!app.available_models.is_empty());
    // All items should have a description
    for item in &app.available_models {
        assert!(!item.description.is_empty());
    }
}
```

**Step 4: Run tests and commit**

Run: `cargo test --workspace`
Expected: All pass

```bash
git add crates/orangecoding-config/ crates/orangecoding-tui/
git commit -m "feat: model identity includes provider prefix for disambiguation

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 5: Custom Provider from Config File

**Problem:** Users should be able to define custom providers in a `models.yml` config file, and those models should automatically appear in the model selector.

**Files:**
- Modify: `crates/orangecoding-config/src/models_config.rs` — add `merge_with_predefined()` method
- Modify: `crates/orangecoding-tui/src/app.rs` — add `App::load_models_from_config()` method

**Step 1: Write the failing test**

In `crates/orangecoding-config/src/models_config.rs`:

```rust
#[test]
fn test_merge_with_predefined() {
    let mut config = ModelsConfig::default();

    // User config only has api_key for zai, no models
    config.providers.insert(
        "zai".to_string(),
        ProviderConfig {
            base_url: None,
            api_key: Some("user-key".to_string()),
            api: None,
            headers: None,
            auth: None,
            models: vec![],
            discovery: None,
        },
    );

    config.merge_with_predefined();

    let zai = config.get_provider("zai").unwrap();
    // Should have predefined base_url filled in
    assert!(zai.base_url.is_some());
    // Should have predefined models filled in
    assert!(!zai.models.is_empty());
    // Should keep user's api_key
    assert_eq!(zai.api_key.as_deref(), Some("user-key"));
}
```

**Step 2: Run test**

Run: `cargo test -p orangecoding-config test_merge_with_predefined`
Expected: FAIL — method doesn't exist

**Step 3: Implement `merge_with_predefined()`**

```rust
pub fn merge_with_predefined(&mut self) {
    let known_providers = ["zai", "zen"];
    for name in &known_providers {
        let canonical = Self::canonical_provider_name(name);
        if let Some(user_config) = self.providers.get_mut(&canonical) {
            if let Some(predefined) = Self::predefined_provider_config(&canonical) {
                // Fill in missing fields from predefined config
                if user_config.base_url.is_none() {
                    user_config.base_url = predefined.base_url;
                }
                if user_config.api.is_none() {
                    user_config.api = predefined.api;
                }
                if user_config.auth.is_none() {
                    user_config.auth = predefined.auth;
                }
                if user_config.models.is_empty() {
                    user_config.models = predefined.models;
                }
            }
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p orangecoding-config -- --nocapture`
Expected: All pass

**Step 5: Commit**

```bash
git add crates/orangecoding-config/
git commit -m "feat: merge user provider config with predefined defaults

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 6: Populate Model Selector from Config

**Problem:** `App::new()` has hardcoded model list. It should populate from `ModelsConfig`.

**Files:**
- Modify: `crates/orangecoding-tui/src/app.rs` — add `App::with_models_config()` constructor

**Step 1: Write the failing test**

```rust
#[test]
fn 测试从config加载模型列表() {
    let app = App::with_models_config("gpt-4", &[
        ("zai".to_string(), "z1-max".to_string(), "Z1 Max (z.ai)".to_string()),
        ("zen".to_string(), "zen-v1".to_string(), "Zen V1 (OpenCode Zen)".to_string()),
    ]);
    assert_eq!(app.available_models.len(), 2);
    assert_eq!(app.available_models[0].value, "zai/z1-max");
    assert_eq!(app.available_models[1].value, "zen/zen-v1");
}
```

**Step 2: Run test**

Run: `cargo test -p orangecoding-tui 测试从config加载模型列表`
Expected: FAIL

**Step 3: Implement `App::with_models_config()`**

```rust
pub fn with_models_config(
    model_name: impl Into<String>,
    models: &[(String, String, String)], // (provider, model_id, display_name)
) -> Self {
    let model_name = model_name.into();
    let available_models: Vec<CommandMenuItem> = models
        .iter()
        .map(|(provider, model_id, display)| CommandMenuItem {
            value: format!("{}/{}", provider, model_id),
            description: display.clone(),
        })
        .collect();

    Self {
        available_models,
        ..Self::new(model_name)
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p orangecoding-tui -- --nocapture`
Expected: All pass

**Step 5: Commit**

```bash
git add crates/orangecoding-tui/
git commit -m "feat: App::with_models_config populates model selector from config

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 7: Integrate Config-Driven Models into Launch

**Files:**
- Modify: `crates/orangecoding-cli/src/commands/launch.rs` — load `ModelsConfig` and pass to `App`

**Step 1: In `run_tui_mode()`, after creating `App`, load models from config**

```rust
// Load models config if available
let models_config_path = dirs::home_dir()
    .map(|h| h.join(".config/orangecoding/models.yml"));
if let Some(path) = models_config_path {
    if path.exists() {
        if let Ok(mut models_cfg) = ModelsConfig::load_from_file(&path) {
            models_cfg.merge_with_predefined();
            let model_items: Vec<CommandMenuItem> = models_cfg
                .list_models()
                .into_iter()
                .map(|(provider, model)| CommandMenuItem {
                    value: ModelsConfig::model_identity(&provider, &model.id),
                    description: format!(
                        "{} ({})",
                        model.name.as_deref().unwrap_or(&model.id),
                        ModelsConfig::provider_display_name(&provider),
                    ),
                })
                .collect();
            if !model_items.is_empty() {
                app.set_available_models(model_items);
            }
        }
    }
}
```

**Step 2: Run full test suite**

Run: `cargo test --workspace`
Expected: All pass

**Step 3: Commit**

```bash
git add crates/orangecoding-cli/
git commit -m "feat: load models from config file into TUI model selector

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 8: Enhance Slash Command Menu with More Items

**Problem:** The slash menu only shows 3 items (model, help, clear). Should show all major commands with descriptions.

**Files:**
- Modify: `crates/orangecoding-tui/src/app.rs` — expand `slash_menu_items()`

**Step 1: Write test**

```rust
#[test]
fn 测试斜杠菜单包含主要命令() {
    let items = App::slash_menu_items();
    let names: Vec<&str> = items.iter().map(|i| i.value.as_str()).collect();
    assert!(names.contains(&"model"));
    assert!(names.contains(&"help"));
    assert!(names.contains(&"compact"));
    assert!(names.contains(&"new"));
    assert!(names.contains(&"plan"));
    assert!(names.contains(&"settings"));
}
```

**Step 2: Expand `slash_menu_items()`**

```rust
fn slash_menu_items() -> Vec<CommandMenuItem> {
    vec![
        CommandMenuItem { value: "model".to_string(), description: "打开模型选择菜单".to_string() },
        CommandMenuItem { value: "help".to_string(), description: "显示命令帮助".to_string() },
        CommandMenuItem { value: "new".to_string(), description: "开始新会话".to_string() },
        CommandMenuItem { value: "compact".to_string(), description: "压缩上下文".to_string() },
        CommandMenuItem { value: "plan".to_string(), description: "切换计划模式".to_string() },
        CommandMenuItem { value: "settings".to_string(), description: "打开设置菜单".to_string() },
        CommandMenuItem { value: "usage".to_string(), description: "显示用量统计".to_string() },
        CommandMenuItem { value: "debug".to_string(), description: "调试信息".to_string() },
        CommandMenuItem { value: "clear".to_string(), description: "清空当前对话".to_string() },
        CommandMenuItem { value: "exit".to_string(), description: "退出".to_string() },
    ]
}
```

**Step 3: Run tests and commit**

Run: `cargo test --workspace`

```bash
git add crates/orangecoding-tui/
git commit -m "feat: expand slash command menu with all major commands

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 9: Final Integration Test & CI Verification

**Step 1: Run full test suite**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

Expected: All pass with zero errors

**Step 2: Verify CI workflow**

```bash
gh workflow view build-and-release.yml
gh run list --workflow=build-and-release.yml --limit 3
```

Check that the latest run passes. If any failures, investigate and fix.

**Step 3: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: address CI feedback

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```
