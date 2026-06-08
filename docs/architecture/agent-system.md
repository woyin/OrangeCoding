# Agent 系统架构

> OrangeCoding 实现了一个多 Agent 协作系统，包含 11 种专业化 Agent，通过意图分类、类别路由和任务协商实现智能任务分配。

## 目录

- [概述](#概述)
- [Agent 类型系统](#agent-类型系统)
  - [AgentKind（11 种 Agent）](#agentkind11-种-agent)
  - [AgentDefinition](#agentdefinition)
- [Category 路由](#category-路由)
- [Intent Gate 意图分类](#intent-gate-意图分类)
- [模型 Fallback 链](#模型-fallback-链)
- [Agent 间通信 (AgentCommBus)](#agent-间通信-agentcommbus)
- [任务协商 (NegotiationProtocol)](#任务协商-negotiationprotocol)
- [任务重分配 (HandoffManager)](#任务重分配-handoffmanager)
- [Agent 生命周期](#agent-生命周期)
- [工作流系统](#工作流系统)

---

## 概述

OrangeCoding 的 Agent 系统位于 `orangecoding-agent` crate 中，是整个系统的核心。它实现了：

```
┌───────────────────────────────────────────────────────────────┐
│                    Agent 系统架构总览                           │
│                                                               │
│  ┌─────────────┐    ┌──────────────┐    ┌──────────────────┐ │
│  │ IntentGate   │───►│ Category路由  │───►│ AgentKind 选择   │ │
│  │ 意图分类      │    │ 8种类别       │    │ 11种专业Agent    │ │
│  └─────────────┘    └──────────────┘    └────────┬─────────┘ │
│                                                    │          │
│  ┌─────────────┐    ┌──────────────┐              │          │
│  │ HookRegistry│◄───│  AgentLoop   │◄─────────────┘          │
│  │ Hook 系统    │    │  主事件循环   │                         │
│  └─────────────┘    └──────┬───────┘                         │
│                             │                                 │
│            ┌────────────────┼────────────────┐               │
│            │                │                │               │
│            ▼                ▼                ▼               │
│    ┌──────────────┐ ┌────────────┐  ┌──────────────┐       │
│    │ ToolExecutor │ │  AI Provider│  │ AgentCommBus │       │
│    │ 工具执行引擎  │ │  模型调用   │  │ Agent间通信   │       │
│    └──────────────┘ └────────────┘  └──────────────┘       │
│                                                               │
│    ┌──────────────────────────────────────────────────┐      │
│    │           NegotiationProtocol                     │      │
│    │           + HandoffManager                       │      │
│    │           任务协商和重分配                          │      │
│    └──────────────────────────────────────────────────┘      │
└───────────────────────────────────────────────────────────────┘
```

---

## Agent 类型系统

### AgentKind（11 种 Agent）

OrangeCoding 定义了 11 种专业化 Agent，每种有独特的定位和默认模型：

```rust
pub enum AgentKind {
    Sisyphus,     // 主编排器
    Hephaestus,   // 深度工作探索者
    Prometheus,   // 战略规划师
    Atlas,        // 任务编排器
    Oracle,       // 架构顾问
    Librarian,    // 文档搜索者
    Explore,      // 代码搜索者
    Metis,        // 计划顾问
    Momus,        // 计划审查者
    Junior,       // 任务执行者
    Multimodal,   // 视觉分析器
}
```

#### 详细 Agent 说明

| # | Agent | 角色 | 默认模型 | Tab | 核心 | 描述 |
|---|-------|------|----------|-----|------|------|
| 1 | **Sisyphus** | 主编排器 | claude-opus-4-6 | 1 | ✅ | 复杂任务分解、分配和监控 |
| 2 | **Hephaestus** | 深度探索者 | gpt-5.4 | 2 | ✅ | 需要大量上下文的深度分析 |
| 3 | **Prometheus** | 战略规划师 | claude-opus-4-6 | 3 | ✅ | 技术方案设计和架构规划 |
| 4 | **Atlas** | 任务编排器 | claude-sonnet-4-6 | 4 | ✅ | DAG 任务调度和管理 |
| 5 | **Oracle** | 架构顾问 | gpt-5.4 | 5 | ❌ | 架构评审和技术选型建议 |
| 6 | **Librarian** | 文档搜索 | minimax-m2.7 | 6 | ❌ | 项目文档和知识检索 |
| 7 | **Explore** | 代码搜索 | grok-code-fast-1 | 7 | ❌ | 代码遍历和符号定位 |
| 8 | **Metis** | 计划顾问 | claude-opus-4-6 | 8 | ❌ | 计划审查和完善 |
| 9 | **Momus** | 批判审查 | gpt-5.4 | 9 | ❌ | Code Review 和风险识别 |
| 10 | **Junior** | 任务执行 | (Category决定) | 10 | ❌ | 具体编码和修改任务 |
| 11 | **Multimodal** | 视觉分析 | gpt-5.4 | 11 | ❌ | 图像和截图分析 |

### AgentDefinition

```rust
pub struct AgentDefinition {
    pub kind: AgentKind,
    pub display_name: String,
    pub description: String,
    pub default_model: String,
    pub system_prompt: String,
    pub allowed_tools: Vec<String>,
    pub role: AgentRole,
    pub tab_order: u32,
    pub is_core: bool,
}
```

---

## Category 路由

8 种内置类别将用户请求路由到合适的模型配置：

| 类别 | 默认模型 | 适用场景 |
|------|----------|----------|
| `Deep` | claude-opus-4-6 | 复杂架构设计、深度分析 |
| `Ultrabrain` | gpt-5.4 | 最高难度推理任务 |
| `UnspecifiedHigh` | claude-sonnet-4-6 | 一般编码实现 |
| `UnspecifiedLow` | 快速模型 | 简单修复、格式调整 |
| `CodeSearch` | grok-code-fast-1 | 代码搜索和导航 |
| `DocSearch` | minimax-m2.7 | 文档搜索和摘要 |
| `Multimodal` | gpt-5.4 | 图像分析、UI 审查 |
| `Quick` | 快速模型 | 简单问答 |

---

## Intent Gate 意图分类

### IntentKind（7 种意图）

```rust
pub enum IntentKind {
    Research,       // 信息收集
    Implementation, // 功能开发
    Fix,           // Bug 修复
    Investigation, // 深度调查
    Refactor,      // 代码重构
    Planning,      // 架构规划
    QuickFix,      // 快速修复
}
```

### 意图→类别→模型映射

```
Research         →  Deep            →  claude-opus-4-6
Implementation   →  UnspecifiedHigh →  claude-sonnet-4-6
Fix              →  UnspecifiedLow  →  快速模型
Investigation    →  Ultrabrain      →  gpt-5.4
Refactor         →  UnspecifiedHigh →  claude-sonnet-4-6
Planning         →  Deep            →  claude-opus-4-6
QuickFix         →  Quick           →  快速模型
```

### ClassifiedIntent

```rust
pub struct ClassifiedIntent {
    pub kind: IntentKind,
    pub confidence: f32,           // 0.0-1.0
    pub recommended_category: String,
    pub keyword_triggered: bool,
    pub trigger_keyword: Option<String>,
}
```

---

## 模型 Fallback 链

当首选模型不可用时，自动尝试备选模型：

```
尝试 1: claude-opus-4-6 (Anthropic)
  └── 失败（限流） →
尝试 2: gpt-5.4 (OpenAI)
  └── 失败（超时） →
尝试 3: deepseek-v3 (DeepSeek)
  └── 成功 → 使用此模型
```

触发条件：`RateLimit`、`Timeout`、`ServerError`、`Auth`、`UnsupportedProvider`

---

## Agent 间通信 (AgentCommBus)

基于 `MessageBus` 的 Agent 通信，支持直接消息和广播：

```
┌─────────┐    ┌──────────────┐    ┌─────────┐
│ Sisyphus │───►│  MessageBus  │◄───│  Atlas  │
└─────────┘    │  (broadcast)  │    └─────────┘
               │              │
┌─────────┐    │              │    ┌─────────┐
│ Junior   │◄──│              │───►│ Oracle  │
└─────────┘    └──────────────┘    └─────────┘
```

### 消息类型

| 类型 | 描述 |
|------|------|
| `TaskRequest` | 任务分配请求 |
| `TaskResult` | 任务完成结果 |
| `StatusUpdate` | Agent 状态变更 |
| `Query` | 信息查询 |
| `Response` | 查询响应 |
| `Negotiation` | 协商消息 |

---

## 任务协商 (NegotiationProtocol)

请求-提议-接受 (RPA) 协议：

```
Sisyphus              Agent 池
   │── 1. Request ─────►│  "需要数据库优化Agent"
   │◄── 2. Propose ────│  Hephaestus: 置信度 0.9
   │◄── 2. Propose ────│  Junior: 置信度 0.6
   │── 3. Accept ──────►│  选择 Hephaestus
   │── 4. Assign ──────►│  分配任务
   │◄── 5. Complete ───│  返回结果
```

选择权重：置信度 40% | 历史表现 25% | 当前负载 20% | 预计耗时 15%

---

## 任务重分配 (HandoffManager)

触发条件：`Timeout` | `CannotComplete` | `ComplexityExceeded` | `DoomLoop` | `VoluntaryHandoff` | `ModelUnavailable`

流程：保存上下文 → 记录原因 → 选择更强Agent → 传递上下文 → 从断点继续

---

## Agent 生命周期

```
Created → register() → Idle → assign_task() → Running
                         ↑                       │
                         │                  ┌────┼────┐
                         │                  ↓    ↓    ↓
                         └── next task ── Completed  Failed
                                            │
                                         Waiting → resume()
```

---

## 工作流系统

| 工作流 | 描述 |
|--------|------|
| **Atlas** | 任务编排：分解 → 分配 → 执行 → 汇总 |
| **Boulder** | 持续推进：循环执行直到完成 |
| **Prometheus** | 规划优先：先规划再执行 |
| **Ultrawork** | 超级工作流：多Agent并行协作 |

---

## 相关文档

- [架构概览](./overview.md) - 系统整体架构
- [Mesh 架构](./mesh.md) - 多 Agent 协调基础设施
- [Hook 系统参考](../reference/hooks.md) - Agent 生命周期 Hook
- [工具参考](../reference/tools.md) - Agent 可使用的工具
