# oh-my-openagent 完整复刻 + 扩展功能实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在已有的 orangecoding-rewrite-rs (975 tests, 11 crates, 98 files) 基础上，完整复刻 oh-my-openagent 的所有功能，并增加 Codex OAuth、Sub-agent 通信、Zellij 集成等扩展功能。

**Architecture:** 分 7 个阶段递进实现。每个阶段独立可编译、可测试、可提交。使用 Clean-room 方法——仅根据公开文档和功能描述重新实现，不参考原始代码。

**Tech Stack:** Rust 1.75+, tokio, serde, serde_json, clap, ratatui, reqwest, ring, dashmap

---

## Phase 4: 专业 Agent 系统 + Category 路由

### Task 4.1: Agent 定义与 Agent Registry 重构

**Files:**
- Create: `crates/orangecoding-agent/src/agents/mod.rs`
- Create: `crates/orangecoding-agent/src/agents/sisyphus.rs`
- Create: `crates/orangecoding-agent/src/agents/hephaestus.rs`
- Create: `crates/orangecoding-agent/src/agents/prometheus.rs`
- Create: `crates/orangecoding-agent/src/agents/atlas.rs`
- Create: `crates/orangecoding-agent/src/agents/oracle.rs`
- Create: `crates/orangecoding-agent/src/agents/librarian.rs`
- Create: `crates/orangecoding-agent/src/agents/explore.rs`
- Create: `crates/orangecoding-agent/src/agents/metis.rs`
- Create: `crates/orangecoding-agent/src/agents/momus.rs`
- Create: `crates/orangecoding-agent/src/agents/junior.rs`
- Create: `crates/orangecoding-agent/src/agents/multimodal.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`
- Test: `crates/orangecoding-agent/src/agents/mod.rs` (内联测试)

每个 Agent 需要：
- `AgentDefinition` trait: name, default_model, fallback_chain, tool_restrictions, system_prompt, order
- 11 个具名 Agent 实现（Sisyphus, Hephaestus, Prometheus, Atlas, Oracle, Librarian, Explore, Metis, Momus, Sisyphus-Junior, Multimodal-Looker）
- 工具限制映射：Oracle/Librarian/Explore 只读；Atlas 不可 re-delegate；Junior 不可 delegate
- Tab 循环确定性排序：Sisyphus(1) > Hephaestus(2) > Prometheus(3) > Atlas(4) > 其他

**Step 1: 写 AgentDefinition trait 和 AgentKind 枚举的测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_kind_from_str() {
        assert_eq!(AgentKind::from_str("sisyphus").unwrap(), AgentKind::Sisyphus);
        assert_eq!(AgentKind::from_str("hephaestus").unwrap(), AgentKind::Hephaestus);
        assert!(AgentKind::from_str("unknown").is_err());
    }

    #[test]
    fn test_agent_tab_order() {
        assert_eq!(AgentKind::Sisyphus.tab_order(), 1);
        assert_eq!(AgentKind::Hephaestus.tab_order(), 2);
        assert_eq!(AgentKind::Prometheus.tab_order(), 3);
        assert_eq!(AgentKind::Atlas.tab_order(), 4);
    }

    #[test]
    fn test_sisyphus_tool_restrictions() {
        let agent = SisyphusAgent::new();
        assert!(agent.blocked_tools().is_empty()); // Sisyphus 无限制
    }

    #[test]
    fn test_oracle_is_readonly() {
        let agent = OracleAgent::new();
        let blocked = agent.blocked_tools();
        assert!(blocked.contains(&"write".to_string()));
        assert!(blocked.contains(&"edit".to_string()));
        assert!(blocked.contains(&"task".to_string()));
    }
}
```

**Step 2: 实现 AgentKind 枚举和 AgentDefinition trait**
**Step 3: 实现 11 个 Agent 结构体**
**Step 4: 运行测试验证**
**Step 5: 提交**

---

### Task 4.2: Category 路由系统

**Files:**
- Create: `crates/orangecoding-agent/src/category.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`
- Test: `crates/orangecoding-agent/src/category.rs` (内联测试)

Category 系统——基于意图的模型路由：
- 8 内置类别：visual-engineering, ultrabrain, deep, artistry, quick, unspecified-low, unspecified-high, writing
- 每个类别有默认模型、variant、temperature、top_p、maxTokens
- 可自定义覆盖
- `is_unstable_agent` 标记

**Step 1: 写 Category 测试**

```rust
#[test]
fn test_builtin_categories() {
    let registry = CategoryRegistry::default();
    assert_eq!(registry.len(), 8);
    let quick = registry.get("quick").unwrap();
    assert!(quick.default_model.contains("mini"));
}

#[test]
fn test_category_override() {
    let mut registry = CategoryRegistry::default();
    registry.override_category("quick", CategoryConfig {
        model: Some("openai/gpt-5.4".into()),
        ..Default::default()
    });
    let quick = registry.get("quick").unwrap();
    assert!(quick.effective_model().contains("gpt-5.4"));
}
```

**Step 2: 实现 CategoryConfig、CategoryRegistry**
**Step 3: 运行测试**
**Step 4: 提交**

---

### Task 4.3: Intent Gate (意图分类)

**Files:**
- Create: `crates/orangecoding-agent/src/intent_gate.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`
- Test: `crates/orangecoding-agent/src/intent_gate.rs`

意图分类器——在执行前分析用户真实意图：
- 意图类型：Research, Implementation, Investigation, Fix, Refactor, Planning, QuickFix
- 基于关键词和上下文的分类逻辑
- 将意图映射到推荐 Category

**Step 1: 写意图分类测试**

```rust
#[test]
fn test_classify_implementation_intent() {
    let gate = IntentGate::new();
    let intent = gate.classify("Build a REST API with JWT auth");
    assert_eq!(intent.kind, IntentKind::Implementation);
}

#[test]
fn test_classify_fix_intent() {
    let gate = IntentGate::new();
    let intent = gate.classify("Fix the login button not responding");
    assert_eq!(intent.kind, IntentKind::Fix);
}
```

**Step 2: 实现 IntentGate、IntentKind、ClassifiedIntent**
**Step 3: 运行测试**
**Step 4: 提交**

---

### Task 4.4: 模型 Fallback 链

**Files:**
- Create: `crates/orangecoding-ai/src/fallback.rs`
- Modify: `crates/orangecoding-ai/src/lib.rs`
- Test: `crates/orangecoding-ai/src/fallback.rs`

模型回退机制——当主模型不可用时自动切换：
- `FallbackChain` 结构：有序模型列表
- 每个条目可带 variant、thinking config
- 基于错误码（429, 503, 529）触发回退
- 冷却期管理

**Step 1-5: TDD 循环实现**

---

## Phase 5: 编排工作流 + Boulder 系统

### Task 5.1: Prometheus 规划工作流

**Files:**
- Create: `crates/orangecoding-agent/src/workflows/mod.rs`
- Create: `crates/orangecoding-agent/src/workflows/prometheus.rs`
- Modify: `crates/orangecoding-agent/src/lib.rs`

Prometheus 工作流：
- 访谈模式——向用户提问以澄清需求
- ClearanceCheck——核心目标明确？范围边界确立？无关键歧义？
- 生成计划到 `.sisyphus/plans/*.md`
- 可选 Metis 咨询（缺口分析）
- 可选 Momus 审核（严格验证）
- Prometheus 为只读——只能创建/修改 `.sisyphus/` 目录下的 markdown 文件

---

### Task 5.2: Atlas 执行编排

**Files:**
- Create: `crates/orangecoding-agent/src/workflows/atlas.rs`
- Test: 内联测试

Atlas 编排引擎：
- 读取 Prometheus 计划
- 分析任务依赖
- 基于 Category+Skill 委派任务给 Sisyphus-Junior
- 智慧积累——从每个任务中提取经验
- 独立验证——不信任子 agent 声明
- Notepad 系统（learnings.md, decisions.md, issues.md, verification.md, problems.md）

---

### Task 5.3: Boulder 系统（会话连续性）

**Files:**
- Create: `crates/orangecoding-agent/src/boulder.rs`
- Test: 内联测试

Boulder 系统——追踪活跃工作状态：
- `boulder.json` 结构：active_plan, session_ids, started_at, plan_name
- 工作恢复——读取 boulder.json 计算进度，注入延续提示
- 中断恢复——断电/崩溃后精确恢复
- `/start-work` 命令集成

---

### Task 5.4: UltraWork (ulw) 模式

**Files:**
- Create: `crates/orangecoding-agent/src/workflows/ultrawork.rs`
- Test: 内联测试

ultrawork 模式——全自动化：
- 关键词触发：`ultrawork` 或 `ulw`
- 自动规划、深度研究、并行 agent、自我修正循环
- 无干预执行
- 集成 Intent Gate + Category 路由

---

## Phase 6: 扩展 Hook 系统 + 技能注入 + 命令

### Task 6.1: 完整 Hook 生命周期引擎

**Files:**
- Modify: `crates/orangecoding-agent/src/hooks.rs` (大幅扩展)
- Test: 内联测试

扩展现有 Hook 系统以支持 40+ 生命周期钩子：

Hook 事件类型：
- PreToolUse — 工具执行前（可阻止/修改输入/注入上下文）
- PostToolUse — 工具执行后（添加警告/修改输出/注入消息）
- Message — 消息处理期间（转换内容/检测关键词/激活模式）
- Event — 会话生命周期变化（恢复/回退/通知）
- Transform — 上下文转换期间（注入上下文/验证块）
- Params — 设置 API 参数时（调整模型设置/effort级别）

关键 Hook 实现：
1. keyword-detector — 检测 ultrawork/ulw/search/find/analyze 关键词
2. think-mode — 自动检测 "think deeply"/"ultrathink"
3. comment-checker — 减少过多注释
4. edit-error-recovery — 编辑工具失败恢复
5. write-existing-file-guard — 防止覆写未读文件
6. session-recovery — 会话错误恢复
7. todo-continuation-enforcer — 强制完成 todo
8. compaction-todo-preserver — 压缩时保留 todo 状态
9. background-notification — 后台任务完成通知
10. tool-output-truncator — 动态截断输出
11. ralph-loop — 自引用循环管理
12. start-work — /start-work 命令执行
13. stop-continuation-guard — 停止延续机制
14. prometheus-md-only — Prometheus 只 markdown
15. hashline-read-enhancer — 增强哈希行读取
16. hashline-edit-diff-enhancer — 增强编辑差异
17. directory-agents-injector — 自动注入 AGENTS.md
18. rules-injector — 条件规则注入
19. compaction-context-injector — 压缩时保留关键上下文
20. auto-update-checker — 版本检查
21. runtime-fallback — 运行时模型回退
22. model-fallback — 模型回退链
23. anthropic-effort — Anthropic effort 调整
24. agent-usage-reminder — Agent 使用提醒
25. delegate-task-retry — 委派任务重试
26. unstable-agent-babysitter — 不稳定 agent 看护

---

### Task 6.2: 技能注入系统扩展

**Files:**
- Modify: `crates/orangecoding-agent/src/skills.rs` (扩展)
- Test: 内联测试

扩展技能系统：
- 内置技能：git-master, playwright, playwright-cli, agent-browser, dev-browser, frontend-ui-ux
- 技能加载路径优先级：.opencode/skills > ~/.config/opencode/skills > .claude/skills > .agents/skills > ~/.agents/skills
- 技能可嵌入 MCP 服务器
- disabled_skills 配置
- SKILL.md 解析（YAML front-matter + markdown 内容）
- Category + Skill 组合策略

---

### Task 6.3: Slash 命令扩展

**Files:**
- Modify: `crates/orangecoding-cli/src/slash_builtins.rs` (扩展)
- Test: 内联测试

新增命令：
- `/init-deep` — 生成层级 AGENTS.md 知识库
- `/ralph-loop` — 自引用开发循环
- `/ulw-loop` — ultrawork 循环
- `/cancel-ralph` — 取消 Ralph 循环
- `/refactor` — 智能重构（LSP + AST-grep）
- `/start-work` — 从 Prometheus 计划开始工作
- `/stop-continuation` — 停止所有延续机制
- `/handoff` — 创建详细上下文摘要

---

### Task 6.4: 权限系统

**Files:**
- Create: `crates/orangecoding-tools/src/permissions.rs`
- Modify: `crates/orangecoding-tools/src/lib.rs`
- Test: 内联测试

工具权限系统：
- 权限类型：edit, bash, webfetch, doom_loop, external_directory
- 权限级别：ask, allow, deny
- 运行时权限检查
- 配置文件覆盖

---

## Phase 7: Sub-Agent 通信 + Zellij 集成

### Task 7.1: Agent 间通信协议

**Files:**
- Create: `crates/orangecoding-mesh/src/agent_comm.rs`
- Create: `crates/orangecoding-mesh/src/negotiation.rs`
- Create: `crates/orangecoding-mesh/src/task_handoff.rs`
- Modify: `crates/orangecoding-mesh/src/lib.rs`
- Test: 内联测试

Sub-agent 间通信：
- 直接消息传递（agent-to-agent messaging）
- 协商协议（negotiation protocol）：请求-提议-接受/拒绝
- 任务重分配（task redistribution）：能力评估 + 负载均衡
- 智慧共享（wisdom sharing）：经验在 agent 间传递
- 消息类型：TaskRequest, TaskOffer, TaskAccept, TaskReject, WisdomShare, StatusUpdate, HelpRequest

**Step 1: 写通信协议测试**

```rust
#[tokio::test]
async fn test_agent_direct_message() {
    let bus = AgentCommBus::new();
    let rx_a = bus.subscribe("agent_a");
    bus.send("agent_b", "agent_a", AgentMessage::StatusUpdate { progress: 50 }).await;
    let msg = rx_a.recv().await.unwrap();
    assert!(matches!(msg, AgentMessage::StatusUpdate { progress: 50 }));
}

#[tokio::test]
async fn test_task_negotiation() {
    let protocol = NegotiationProtocol::new();
    let request = TaskRequest { task_id: "t1".into(), requirements: vec!["rust".into()] };
    let offer = protocol.negotiate("agent_a", request).await;
    assert!(offer.is_some()); // 至少一个 agent 应答
}
```

**Step 2-5: TDD 实现**

---

### Task 7.2: Zellij 集成

**Files:**
- Create: `crates/orangecoding-cli/src/zellij.rs`
- Modify: `crates/orangecoding-cli/src/lib.rs`
- Test: `crates/orangecoding-cli/src/zellij.rs`

Zellij 终端复用器集成：
- 检测 Zellij 环境（ZELLIJ_SESSION_NAME 环境变量）
- 子 agent 在 Zellij pane 中生成
- 布局管理——主面板 + 子 agent 面板
- Pane 操作：new-pane, write-chars, close-pane, focus-pane
- 配置选项：enabled, layout, main_pane_size
- 兼容 tmux 回退

```rust
#[test]
fn test_detect_zellij_env() {
    std::env::set_var("ZELLIJ_SESSION_NAME", "test");
    assert!(ZellijIntegration::is_available());
    std::env::remove_var("ZELLIJ_SESSION_NAME");
}

#[test]
fn test_zellij_pane_command() {
    let zellij = ZellijIntegration::new();
        let cmd = zellij.build_new_pane_command("OrangeCoding agent run", Some("Agent-1"));

    assert!(cmd.contains("zellij action new-pane"));
}
```

---

## Phase 8: Codex OAuth + JSONC 配置 + 高级工具

### Task 8.1: Codex OAuth 认证

**Files:**
- Create: `crates/orangecoding-cli/src/oauth.rs`
- Create: `crates/orangecoding-cli/src/commands/mcp_oauth.rs`
- Modify: `crates/orangecoding-cli/src/commands/mod.rs`
- Test: 内联测试

OAuth 2.1 认证流程：
- RFC 9728 (Protected Resource Metadata) 自动发现
- RFC 8414 (Authorization Server Metadata) 回退发现
- RFC 7591 (Dynamic Client Registration) 支持
- PKCE 强制
- RFC 8707 (Resource Indicators)
- Token 存储在 `~/.config/opencode/mcp-oauth.json`（0600 权限）
- 自动刷新（401 时 refresh）；403 时 step-up authorization
- CLI 命令：`mcp oauth login`, `mcp oauth logout`, `mcp oauth status`

---

### Task 8.2: JSONC 配置解析

**Files:**
- Create: `crates/orangecoding-config/src/jsonc.rs`
- Modify: `crates/orangecoding-config/src/config.rs`
- Test: 内联测试

JSONC 配置支持：
- 支持 `//` 和 `/* */` 注释
- 支持尾逗号
- 配置文件位置：`.opencode/oh-my-openagent.json[c]`（项目级）、`~/.config/opencode/oh-my-openagent.json[c]`（用户级）
- 用户配置为基础，项目配置合并覆盖
- `.jsonc` 优先于 `.json`
- 完整的 Agent、Category、Skill、Hook、MCP、Permission 配置支持

---

### Task 8.3: 扩展工具集

**Files:**
- Create: `crates/orangecoding-tools/src/call_omo_agent.rs` — 子 agent 调用工具
- Create: `crates/orangecoding-tools/src/background_tool.rs` — 后台任务管理（background_output, background_cancel）
- Create: `crates/orangecoding-tools/src/look_at_tool.rs` — 多模态分析工具
- Create: `crates/orangecoding-tools/src/session_tools.rs` — 会话工具（session_list, session_read, session_search, session_info）
- Create: `crates/orangecoding-tools/src/task_management.rs` — 任务管理工具（task_create, task_get, task_list, task_update）
- Create: `crates/orangecoding-tools/src/skill_tool.rs` — 技能工具（skill, skill_mcp）
- Create: `crates/orangecoding-tools/src/interactive_bash.rs` — 交互式终端工具
- Modify: `crates/orangecoding-tools/src/registry.rs`
- Test: 各工具内联测试

---

### Task 8.4: Hashline 编辑工具增强

**Files:**
- Modify: `crates/orangecoding-agent/src/hashline.rs`
- Test: 内联测试

LINE#ID 内容哈希锚定编辑：
- 每行生成哈希标记
- 编辑前验证内容哈希
- 零陈旧行错误
- hashline-read-enhancer 钩子集成
- hashline-edit-diff-enhancer 钩子集成

---

## Phase 9: 文档 + 中文注释 + 推送

### Task 9.1: 全部代码添加中文注释

**Files:** 所有 `.rs` 文件

要求：
- 每个 `pub` 项（struct、enum、fn、trait、const）必须有 `///` 中文文档注释
- 每个模块文件顶部必须有 `//!` 模块级注释
- 复杂逻辑处添加行内 `//` 中文注释
- 不注释显而易见的代码

---

### Task 9.2: 用户手册

**Files:**
- Create: `docs/guide/overview.md` — 项目概览
- Create: `docs/guide/installation.md` — 安装指南
- Create: `docs/guide/quick-start.md` — 快速开始
- Create: `docs/guide/orchestration.md` — 编排系统指南
- Create: `docs/guide/agent-model-matching.md` — Agent-模型匹配指南

---

### Task 9.3: 参考手册

**Files:**
- Create: `docs/reference/configuration.md` — 配置参考
- Create: `docs/reference/features.md` — 功能参考
- Create: `docs/reference/cli.md` — CLI 参考
- Create: `docs/reference/agents.md` — Agent 参考
- Create: `docs/reference/hooks.md` — Hook 参考
- Create: `docs/reference/tools.md` — 工具参考

---

### Task 9.4: 架构文档

**Files:**
- Create: `docs/architecture/overview.md` — 架构概览
- Create: `docs/architecture/security.md` — 安全架构
- Create: `docs/architecture/multi-agent.md` — 多 Agent 架构
- Create: `docs/architecture/zellij-integration.md` — Zellij 集成架构

---

### Task 9.5: 最终验证与推送

**Steps:**
1. `cargo test --workspace` — 全量测试通过
2. `cargo clippy --workspace` — 无警告
3. `cargo doc --workspace --no-deps` — 文档生成正常
4. 更新 README.md — 反映所有新功能
5. `git add -A && git commit -m "feat: 完整复刻 oh-my-openagent 所有功能"`
6. `git push origin master --force`

---

## 实施优先级

| 顺序 | 阶段 | 预估任务数 | 关键路径 |
|------|------|-----------|---------|
| 1 | Phase 4: Agent + Category | 4 tasks | 后续所有功能的基础 |
| 2 | Phase 5: 编排 + Boulder | 4 tasks | 依赖 Phase 4 的 Agent 定义 |
| 3 | Phase 6: Hook + Skill + 命令 | 4 tasks | 依赖 Phase 4-5 的 Agent 和工作流 |
| 4 | Phase 7: 通信 + Zellij | 2 tasks | 独立于 Phase 5-6，可并行 |
| 5 | Phase 8: OAuth + JSONC + 工具 | 4 tasks | 部分依赖 Phase 4 的 Agent |
| 6 | Phase 9: 文档 + 注释 + 推送 | 5 tasks | 最后阶段 |

**总计: 23 个任务，约 50+ 个子步骤**
