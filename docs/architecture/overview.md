# 架构概览

> OrangeCoding 是一个用 Rust 编写的多 Agent AI 编程助手系统，采用模块化 workspace 架构，由 11 个独立 crate 组成。

## 目录

- [系统概述](#系统概述)
- [Crate 架构图](#crate-架构图)
- [各 Crate 职责](#各-crate-职责)
- [依赖关系](#依赖关系)
- [数据流](#数据流)
- [安全边界](#安全边界)
- [技术栈](#技术栈)

---

## 系统概述

OrangeCoding（Code Engineering AI Runtime）是一个基于 Rust 的多 Agent AI 编程助手运行时系统。其设计目标是：

- **多 Agent 协作**：11 种专业化 Agent，各司其职
- **多模型支持**：统一适配 OpenAI、Anthropic、DeepSeek、通义千问、文心一言
- **安全优先**：多层次权限控制、审计链、密钥检测
- **可扩展**：插件化工具系统、Hook 机制、MCP 协议支持
- **高性能**：基于 Tokio 异步运行时，支持并发工具执行

---

## Crate 架构图

```
                           ┌─────────────────────────┐
                           │       orangecoding-cli          │
                           │    (命令行入口点)          │
                           │  Commands: launch,       │
                           │  config, status          │
                           │  OAuth: PKCE 授权流程     │
                           │  Slash: 斜杠命令处理      │
                           └────────────┬─────────────┘
                                        │
            ┌───────────────────────────┬┴────────────────────────┐
            │                          │                          │
            ▼                          ▼                          ▼
   ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────────┐
   │    orangecoding-tui     │    │   orangecoding-agent    │    │   orangecoding-config      │
   │  (终端用户界面)   │    │  (Agent 核心循环) │    │  (配置管理)          │
   │                  │    │                  │    │                     │
   │ • Ratatui 渲染   │    │ • AgentLoop      │    │ • 多层配置合并       │
   │ • Markdown 渲染  │    │ • IntentGate     │    │ • CryptoStore       │
   │ • 主题系统       │    │ • HookRegistry   │    │ • 模型配置           │
   │ • 输入处理       │    │ • 11种 Agent      │    │ • JSONC 解析         │
   │ • 多 Tab 显示    │    │ • Category 路由   │    │ • 配置发现           │
   └────────┬────────┘    └────────┬─────────┘    └──────────┬──────────┘
            │                      │                          │
            └──────────────────────┼──────────────────────────┘
                                   │
            ┌──────────────────────┼──────────────────────────┐
            │                      │                          │
            ▼                      ▼                          ▼
   ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────────┐
   │   orangecoding-mesh    │    │   orangecoding-tools    │    │     orangecoding-ai        │
   │  (多Agent协调)   │    │  (工具系统)       │    │  (AI 提供商适配)     │
   │                  │    │                  │    │                     │
   │ • MessageBus    │    │ • 16+ 工具实现    │    │ • OpenAI 适配       │
   │ • ModelRouter   │    │ • ToolRegistry   │    │ • Anthropic 适配    │
   │ • SharedState   │    │ • SecurityPolicy │    │ • DeepSeek 适配     │
   │ • TaskOrchestrator│  │ • FileOpGuard    │    │ • 通义千问适配       │
   │ • AgentRegistry │    │ • Permissions    │    │ • 文心一言适配       │
   │ • Negotiation   │    │                  │    │ • 流式解析           │
   │ • TaskHandoff   │    │                  │    │ • Fallback 链       │
   └────────┬────────┘    └────────┬─────────┘    └──────────┬──────────┘
            │                      │                          │
            └──────────────────────┼──────────────────────────┘
                                   │
            ┌──────────────────────┼──────────────┐
            │                      │              │
            ▼                      ▼              ▼
   ┌─────────────────┐    ┌──────────────┐   ┌──────────────────┐
   │  orangecoding-session   │    │  orangecoding-core  │   │   orangecoding-audit    │
   │  (会话管理)      │    │  (基础类型)   │   │  (审计日志)       │
   │                  │    │              │   │                  │
   │ • JSONL 存储     │    │ • AgentId    │   │ • HashChain      │
   │ • 多分支会话     │    │ • SessionId  │   │ • AuditLogger    │
   │ • SessionTree   │    │ • AgentRole  │   │ • Sanitizer      │
   │ • 条目类型系统   │    │ • Message    │   │ • SecretDetector │
   │ • 雪花 ID       │    │ • Event      │   │                  │
   └─────────────────┘    │ • ToolCall   │   └──────────────────┘
                           │ • TokenUsage │
                           └──────────────┘

   ┌──────────────────┐
   │    orangecoding-mcp     │
   │  (MCP 协议实现)   │
   │                  │
   │ • JSON-RPC 2.0   │
   │ • McpServer      │
   │ • McpClient      │
   │ • StdioTransport │
   └──────────────────┘
```

---

## 各 Crate 职责

### orangecoding-core（基础类型）

系统的基石，定义所有共享的基础类型和抽象。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `types.rs` | `AgentId`, `SessionId`, `ToolName`, `AgentRole`, `AgentStatus`, `AgentCapability`, `TokenUsage` | 全局标识符和枚举 |
| `message.rs` | `Role`, `Message`, `ToolCall`, `Conversation` | AI 对话消息模型 |
| `event.rs` | `AgentEvent` | Agent 事件类型 |
| `error.rs` | `OrangeCodingError` | 统一错误类型 |

**设计原则**：零外部 AI/网络依赖，仅使用 serde、chrono、uuid 等基础库。

### orangecoding-agent（Agent 核心循环）

实现 Agent 的主事件循环、生命周期管理和专业化 Agent。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `agent_loop.rs` | `AgentLoop` | 主循环：接收消息 → AI 推理 → 工具执行 → 返回结果 |
| `context.rs` | `AgentContext` | 运行时上下文（会话、对话、环境变量） |
| `executor.rs` | `ToolExecutor` | 并发工具执行引擎 |
| `hooks.rs` | `HookRegistry`, `HookEvent`, `HookAction`, `HookPriority` | 生命周期 Hook 系统 |
| `intent_gate.rs` | `IntentGate`, `IntentKind`, `ClassifiedIntent` | 用户意图分类 |
| `category.rs` | `Category` | 8 种任务类别路由 |
| `agents/mod.rs` | `AgentKind` | 11 种专业化 Agent 定义 |
| `compaction.rs` | — | 上下文压缩策略 |
| `memory.rs` | — | 内存管理 |
| `pipeline.rs` | — | Agent 执行管线 |
| `skills.rs` | — | 技能定义 |
| `workflows/` | — | 预配置工作流（Atlas、Boulder、Prometheus 等） |

### orangecoding-tools（工具系统）

实现所有可供 Agent 使用的工具。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `lib.rs` | `Tool` trait, `ToolError`, `ToolResult` | 工具接口定义 |
| `registry.rs` | `ToolRegistry` | 线程安全的工具注册表 |
| `security.rs` | `SecurityPolicy`, `PathValidator`, `FileOperationGuard` | 安全策略和路径验证 |
| `permissions.rs` | `PermissionKind`, `PermissionLevel`, `PermissionPolicy` | 权限系统 |
| `*_tool.rs` | 16+ 具体工具 | 各功能工具实现 |

### orangecoding-ai（AI 提供商适配）

统一抽象多个 AI 服务提供商。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `provider.rs` | `AiProvider` trait, `ChatMessage`, `ToolDefinition`, `StreamEvent` | 统一 AI 接口 |
| `model_roles.rs` | `ModelRole`, `ThinkingLevel` | 模型角色和思考级别 |
| `fallback.rs` | — | 模型降级策略 |
| `stream.rs` | `SseParser`, `StreamAggregator` | SSE 流式解析 |
| `providers/openai.rs` | `OpenAiProvider` | OpenAI API 适配 |
| `providers/anthropic.rs` | `AnthropicProvider` | Anthropic API 适配 |
| `providers/deepseek.rs` | `DeepSeekProvider` | DeepSeek API 适配 |
| `providers/qianwen.rs` | `QianwenProvider` | 通义千问 API 适配 |
| `providers/wenxin.rs` | `WenxinProvider` | 文心一言 API 适配 |

### orangecoding-mesh（多 Agent 协调）

管理多 Agent 间的通信、协调和任务编排。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `message_bus.rs` | `MessageBus`, `BusMessage` | 发布/订阅消息路由 |
| `model_router.rs` | `ModelRouter`, `RoutingRule`, `RoutingCondition` | 动态模型选择 |
| `shared_state.rs` | `SharedState` | 线程安全的共享状态（含 TTL） |
| `task_orchestrator.rs` | `TaskOrchestrator`, `Task`, `TaskStatus` | DAG 任务调度 |
| `agent_registry.rs` | `AgentRegistry`, `AgentInfo` | Agent 注册发现 |
| `role_system.rs` | `RoleSystem`, `RoleDefinition` | 角色权限定义 |
| `agent_comm.rs` | `AgentComm` | 点对点 Agent 通信 |
| `negotiation.rs` | `NegotiationProtocol` | 请求-提议-接受协议 |
| `task_handoff.rs` | `TaskHandoff` | 任务重分配管理 |

### orangecoding-config（配置管理）

多层次配置管理，支持加密存储。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `config.rs` | `OrangeCodingConfig`, `AiConfig`, `AgentConfig`, `ToolsConfig`, `TuiConfig`, `LoggingConfig` | 配置数据结构 |
| `crypto.rs` | `CryptoStore` | AES-256 加密密钥存储 |
| `source.rs` | `ConfigSource`, `LayeredConfig` | 多层配置合并 |
| `discovery.rs` | `ConfigDiscovery` | 自动配置文件发现 |
| `models_config.rs` | `ModelsConfig` | AI 模型定义 |
| `jsonc.rs` | — | JSON with Comments 解析 |

### orangecoding-audit（审计日志）

不可篡改的审计日志系统。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `logger.rs` | `AuditLogger`, `AuditEntry`, `AuditLoggerConfig` | 审计日志记录 |
| `chain.rs` | `HashChain` | SHA-256 哈希链 |
| `sanitizer.rs` | `Sanitizer` | 敏感信息脱敏 |
| `secrets.rs` | `SecretSource`, `ObfuscationMode`, `SecretEntry` | 密钥检测 |

### orangecoding-session（会话管理）

基于 JSONL 的会话持久化和多分支支持。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `entry.rs` | `EntryId`, `EntryType`, `EntryData`, `MessageEntry` | 会话条目类型 |
| `storage.rs` | `SessionStorage`, `SessionHeader` | JSONL 文件存储 |
| `manager.rs` | `SessionManager`, `SessionInfo`, `Session` | 会话生命周期 |
| `tree.rs` | `SessionTree` | 多分支会话树 |

### orangecoding-mcp（MCP 协议）

Model Context Protocol 的 Rust 实现。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `protocol.rs` | `JsonRpcRequest`, `JsonRpcResponse`, `RequestId` | JSON-RPC 2.0 |
| `server.rs` | `McpServer`, `McpCapabilities`, `ToolHandler` trait | MCP 服务器 |
| `client.rs` | `McpClient`, `ClientConfig` | MCP 客户端 |
| `transport.rs` | `Transport` trait, `StdioTransport` | 传输层抽象 |

### orangecoding-tui（终端界面）

基于 Ratatui 的终端用户界面。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `app.rs` | `App`, `AppMode`, `AppAction` | 应用状态机 |
| `markdown.rs` | `MarkdownRenderer` | Markdown 渲染 |
| `theme.rs` | `Theme`, `ThemeVariant`, `ColorMode` | 主题系统 |
| `components/` | — | UI 组件（会话、输入、状态栏） |

### orangecoding-cli（命令行入口）

程序入口点和命令行接口。

| 模块 | 关键类型 | 描述 |
|------|----------|------|
| `main.rs` | `Cli`, `Commands` | Clap 命令定义 |
| `commands/` | — | launch, config, status, init |
| `oauth.rs` | — | OAuth PKCE 流程 |
| `slash.rs` | — | 斜杠命令处理 |
| `rpc.rs` | — | JSON-RPC 客户端 |

---

## 依赖关系

### Crate 依赖图

```
orangecoding-cli ──────┬──► orangecoding-tui
                ├──► orangecoding-agent ──┬──► orangecoding-tools ──► orangecoding-core
                ├──► orangecoding-config  │                      ▲
                └──► orangecoding-mcp     ├──► orangecoding-ai ─────────┘
                                   ├──► orangecoding-mesh ───────┘
                                   ├──► orangecoding-session ────┘
                                   └──► orangecoding-audit ──────┘
```

### 依赖规则

| 规则 | 描述 |
|------|------|
| `orangecoding-core` 无项目内依赖 | 基石层，被所有其他 crate 依赖 |
| 无循环依赖 | Cargo workspace 强制保证 |
| 上层可依赖下层 | cli → agent → tools → core |
| 平层可互相独立 | mesh、session、audit 互不依赖 |

### 外部依赖摘要

| 领域 | 主要依赖 | 用途 |
|------|----------|------|
| 异步运行时 | `tokio`, `futures`, `async-trait` | 异步编程基础 |
| 序列化 | `serde`, `serde_json`, `toml` | 数据序列化/反序列化 |
| HTTP | `reqwest` (rustls-tls) | HTTP 客户端 |
| CLI | `clap` (derive) | 命令行解析 |
| TUI | `ratatui`, `crossterm` | 终端界面 |
| 并发 | `dashmap`, `parking_lot` | 并发数据结构 |
| 加密 | `ring`, `base64` | 加密和编码 |
| 嵌入式数据库 | `sled` | 嵌入式键值数据库 |
| 日志 | `tracing`, `tracing-subscriber` | 结构化日志 |
| 时间 | `chrono` | 时间处理 |
| ID | `uuid` (v4) | 唯一标识符 |
| 搜索 | `regex`, `glob`, `walkdir` | 文件搜索 |

---

## 数据流

### 用户请求处理流程

```
                                    完整数据流
═══════════════════════════════════════════════════════════════

  用户输入                 "帮我修复 src/main.rs 中的编译错误"
     │
     ▼
┌─── orangecoding-cli ────────────────────────────────────────────────┐
│  1. 解析命令行参数                                             │
│  2. 加载配置（orangecoding-config）                                   │
│  3. 初始化日志（tracing）                                      │
│  4. 创建/恢复会话（orangecoding-session）                              │
└──────────────────────────┬───────────────────────────────────┘
                           │
                           ▼
┌─── orangecoding-agent ──────────────────────────────────────────────┐
│  5. IntentGate 分类                                           │
│     意图: Fix, 置信度: 0.92                                    │
│     推荐类别: "unspecified-low"                                │
│                                                               │
│  6. AgentKind 选择                                            │
│     选中: Junior Agent（类别决定模型）                          │
│                                                               │
│  7. HookRegistry 执行 PreMessage Hooks                        │
│     • 注入上下文信息                                            │
│     • 检查权限                                                 │
└──────────────────────────┬───────────────────────────────────┘
                           │
                           ▼
┌─── orangecoding-mesh ───────────────────────────────────────────────┐
│  8. ModelRouter 路由                                          │
│     任务类型: Coding, 复杂度: 30                               │
│     选中模型: claude-sonnet-4-6                               │
│                                                               │
│  9. SharedState 加载上下文                                     │
│     工作目录、历史对话、环境信息                                 │
└──────────────────────────┬───────────────────────────────────┘
                           │
                           ▼
┌─── orangecoding-ai ─────────────────────────────────────────────────┐
│  10. AnthropicProvider.chat_completion_stream()               │
│      发送: 系统提示 + 上下文 + 用户消息 + 工具定义              │
│      接收: 流式 AI 响应                                        │
│                                                               │
│  11. StreamAggregator 聚合                                    │
│      收集 ContentDelta、ToolCallStart 等事件                   │
└──────────────────────────┬───────────────────────────────────┘
                           │
                           ▼
┌─── orangecoding-tools ──────────────────────────────────────────────┐
│  12. AI 请求工具调用: bash { command: "cargo build" }         │
│                                                               │
│  13. HookRegistry 执行 PreToolCall Hooks                     │
│      • security_path_check ✅                                 │
│      • permission_bash_check ✅                               │
│                                                               │
│  14. FileOperationGuard 检查 ✅                               │
│                                                               │
│  15. BashTool.execute() → 执行 cargo build                   │
│                                                               │
│  16. HookRegistry 执行 PostToolCall Hooks                    │
│      • audit_tool_call → 记录到审计链                          │
│      • transform_sanitize_output → 脱敏输出                   │
└──────────────────────────┬───────────────────────────────────┘
                           │
                           ▼
┌─── orangecoding-audit ──────────────────────────────────────────────┐
│  17. AuditLogger 记录                                         │
│      • 工具调用详情                                            │
│      • Token 使用量                                           │
│      • 哈希链追加                                             │
│                                                               │
│  18. Sanitizer 脱敏                                           │
│      • 移除结果中的 API 密钥                                   │
│      • 匿名化路径中的用户名                                    │
└──────────────────────────┬───────────────────────────────────┘
                           │
                           ▼
┌─── orangecoding-session ────────────────────────────────────────────┐
│  19. Session 记录                                             │
│      • 用户消息条目                                            │
│      • AI 响应条目                                            │
│      • 工具调用条目                                            │
│      • 追加到 JSONL 文件                                      │
└──────────────────────────┬───────────────────────────────────┘
                           │
                           ▼
┌─── orangecoding-tui ────────────────────────────────────────────────┐
│  20. MarkdownRenderer 渲染 AI 响应                            │
│  21. 显示工具调用结果                                          │
│  22. 更新状态栏（Token 使用量、Agent 状态）                     │
└──────────────────────────────────────────────────────────────┘
```

---

## 安全边界

```
┌──────────────────────────────────────────────────────────────┐
│                       安全边界图                              │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──── 用户空间 ────────────────────────────────────────┐    │
│  │                                                      │    │
│  │  ┌──── 沙箱边界 ──────────────────────────────────┐  │    │
│  │  │                                                │  │    │
│  │  │  orangecoding-agent (Agent 循环)                      │  │    │
│  │  │       ↓                                        │  │    │
│  │  │  ╔═══ 安全层 1: Hook 系统 ═══════════════════╗ │  │    │
│  │  │  ║ PreToolCall Hooks (权限检查、安全扫描)      ║ │  │    │
│  │  │  ╚═══════════════════════════════════════════╝ │  │    │
│  │  │       ↓                                        │  │    │
│  │  │  ╔═══ 安全层 2: FileOperationGuard ══════════╗ │  │    │
│  │  │  ║ 路径验证、阻止列表、遍历检测                ║ │  │    │
│  │  │  ╚═══════════════════════════════════════════╝ │  │    │
│  │  │       ↓                                        │  │    │
│  │  │  ╔═══ 安全层 3: PermissionPolicy ════════════╗ │  │    │
│  │  │  ║ 权限级别: Allow / Ask / Deny              ║ │  │    │
│  │  │  ╚═══════════════════════════════════════════╝ │  │    │
│  │  │       ↓                                        │  │    │
│  │  │  orangecoding-tools (工具执行)                        │  │    │
│  │  │       ↓                                        │  │    │
│  │  │  ╔═══ 安全层 4: 审计链 ══════════════════════╗ │  │    │
│  │  │  ║ HashChain + Sanitizer + SecretDetector    ║ │  │    │
│  │  │  ╚═══════════════════════════════════════════╝ │  │    │
│  │  │                                                │  │    │
│  │  └── 沙箱边界 ────────────────────────────────────┘  │    │
│  │                                                      │    │
│  │  ┌──── 网络边界 ──────────────────────────────────┐  │    │
│  │  │  orangecoding-ai (AI API 请求，HTTPS 强制)            │  │    │
│  │  │  orangecoding-mcp (MCP 通信，OAuth 认证)              │  │    │
│  │  └────────────────────────────────────────────────┘  │    │
│  │                                                      │    │
│  │  ┌──── 存储边界 ──────────────────────────────────┐  │    │
│  │  │  orangecoding-config (加密密钥存储，AES-256)          │  │    │
│  │  │  orangecoding-session (会话文件，用户权限)             │  │    │
│  │  │  orangecoding-audit (审计日志，只追加)                 │  │    │
│  │  └────────────────────────────────────────────────┘  │    │
│  │                                                      │    │
│  └──── 用户空间 ────────────────────────────────────────┘    │
│                                                              │
│  ╔═══ 永久阻止区域 ════════════════════════════════════════╗ │
│  ║  /etc/shadow, ~/.ssh/, ~/.aws/, /sys/, /proc/           ║ │
│  ║  无论任何配置均不可访问                                   ║ │
│  ╚═════════════════════════════════════════════════════════╝ │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 安全层次说明

| 层次 | 组件 | 功能 | 可绕过 |
|------|------|------|--------|
| 第 1 层 | Hook 系统 | 可编程的安全检查 | ❌ Critical 优先级 Hook 不可跳过 |
| 第 2 层 | FileOperationGuard | 路径安全验证 | ❌ 永久阻止路径不可覆盖 |
| 第 3 层 | PermissionPolicy | 权限决策 | ⚠️ auto_approve 模式可跳过 Ask |
| 第 4 层 | 审计链 | 事后追溯 | ❌ 哈希链不可篡改 |

---

## 技术栈

| 类别 | 技术 | 版本 |
|------|------|------|
| 语言 | Rust | 2021 Edition, MSRV 1.75 |
| 异步运行时 | Tokio | 1.35+ |
| HTTP 客户端 | reqwest (rustls-tls) | 0.11 |
| CLI 框架 | clap (derive) | 4.4 |
| TUI 框架 | ratatui + crossterm | 0.24 / 0.27 |
| 序列化 | serde + serde_json + toml | 1.0 / 1.0 / 0.8 |
| 并发容器 | DashMap + parking_lot | 5.5 / 0.12 |
| 加密 | ring | 0.17 |
| 嵌入式数据库 | sled | 0.34 |
| 日志 | tracing + tracing-subscriber | 0.1 / 0.3 |
| 许可证 | Apache-2.0 | — |

---

## 相关文档

- [Agent 系统架构](./agent-system.md) - Agent 类型和协作机制
- [Mesh 架构](./mesh.md) - 多 Agent 协调详解
- [安全架构](./security.md) - 安全策略深入分析
- [工具参考](../reference/tools.md) - 工具使用指南
- [Hook 系统参考](../reference/hooks.md) - Hook 系统详解
