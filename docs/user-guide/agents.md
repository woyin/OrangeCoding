# Agent 系统文档

> CEAIR 内置 11 个专业 Agent，每个 Agent 拥有特定的角色、模型、工具权限和使用场景。

## 目录

- [概述](#概述)
- [Agent 一览表](#agent-一览表)
- [Agent 详细说明](#agent-详细说明)
  - [Sisyphus — 主编排器](#sisyphus--主编排器)
  - [Hephaestus — 深度工作者](#hephaestus--深度工作者)
  - [Prometheus — 战略规划器](#prometheus--战略规划器)
  - [Atlas — 任务执行编排器](#atlas--任务执行编排器)
  - [Oracle — 架构顾问](#oracle--架构顾问)
  - [Librarian — 文档搜索](#librarian--文档搜索)
  - [Explore — 代码搜索](#explore--代码搜索)
  - [Metis — 计划顾问](#metis--计划顾问)
  - [Momus — 计划审核](#momus--计划审核)
  - [Junior — 任务执行器](#junior--任务执行器)
  - [Multimodal — 视觉分析](#multimodal--视觉分析)
- [Agent 协作模式](#agent-协作模式)
- [自定义 Agent 配置](#自定义-agent-配置)

---

## 概述

CEAIR 的 Agent 系统采用 **专业化分工 + 协作编排** 的架构设计。每个 Agent 都有明确的职责边界和工具权限，通过 `ceair-mesh` 消息总线实现 Agent 间的通信和任务协商。

### 核心概念

- **模型绑定**：每个 Agent 绑定默认 AI 模型，可通过配置覆盖
- **工具权限**：通过 `blocked_tools` 限制 Agent 可使用的工具
- **委托能力**：部分 Agent 可将子任务委托给其他 Agent
- **Tab 顺序**：Agent 在 TUI 中的切换顺序
- **Category 路由**：基于意图分类自动选择最优模型

---

## Agent 一览表

| Tab | Agent | 角色 | 默认模型 | 可写入 | 可委托 | 可执行命令 |
|-----|-------|------|---------|--------|--------|-----------|
| 1 | **Sisyphus** | 主编排器 | `claude-opus-4-6` | ✅ | ✅ | ✅ |
| 2 | **Hephaestus** | 深度工作者 | `gpt-5.4` | ✅ | ✅ | ✅ |
| 3 | **Prometheus** | 战略规划器 | `claude-opus-4-6` | ⚠️ 仅规划文件 | ✅ | ❌ |
| 4 | **Atlas** | 任务执行编排器 | `claude-sonnet-4-6` | ✅ | ❌ | ✅ |
| 5 | **Oracle** | 架构顾问 | `claude-opus-4-6` | ❌ | ❌ | ❌ |
| 6 | **Librarian** | 文档搜索 | `minimax-m2.7` | ❌ | ❌ | ❌ |
| 7 | **Explore** | 代码搜索 | `grok-code-fast-1` | ❌ | ❌ | ❌ |
| 8 | **Metis** | 计划顾问 | `claude-opus-4-6` | ❌ | ❌ | ❌ |
| 9 | **Momus** | 计划审核 | `gpt-5.4` | ❌ | ❌ | ❌ |
| 10 | **Junior** | 任务执行器 | 按类别分配 | ✅ | ❌ | ✅ |
| 11 | **Multimodal** | 视觉分析 | `gpt-5.4` | ❌ | ❌ | ❌ |

---

## Agent 详细说明

### Sisyphus — 主编排器

**Tab 顺序：** 1（默认 Agent）

**默认模型：** `claude-opus-4-6`

**角色定位：**
Sisyphus 是 CEAIR 的核心编排 Agent，以希腊神话中永不放弃的西西弗斯命名。它拥有全部工具权限，负责理解用户意图、分解任务并协调其他 Agent 完成工作。

**工具权限：** 全部工具（无限制）

**关键能力：**
- 理解和分解复杂的开发需求
- 直接执行编码、测试、调试任务
- 委托子任务给专业 Agent（通过 `task` 工具）
- 管理 Boulder 会话状态，维持开发连续性
- 触发 UltraWork 全自动开发模式
- 调用 Prometheus 进行战略规划

**典型使用场景：**
- 日常编程交互（默认入口）
- 复杂项目的端到端开发
- 需要多 Agent 协作的大型任务
- Bug 调查和修复

**配置参数：**

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `temperature` | 0.7 | 生成多样性 |
| `max_tokens` | 4096 | 最大输出 Token 数 |
| `thinking_level` | `Medium` | 推理深度 |

---

### Hephaestus — 深度工作者

**Tab 顺序：** 2

**默认模型：** `gpt-5.4`

**角色定位：**
以希腊锻造之神赫菲斯托斯命名，Hephaestus 是专注于深度工程任务的 Agent。它使用 GPT-5.4 的强大推理能力，适合处理需要深度思考的复杂问题。

**工具权限：** 全部工具（无限制）

**关键能力：**
- 深度代码分析和重构
- 复杂算法实现
- 性能优化和调优
- 多文件协同修改
- 可委托子任务给其他 Agent

**典型使用场景：**
- 大规模代码重构
- 复杂功能实现
- 系统性能分析和优化
- 需要深度推理的技术决策

**与 Sisyphus 的区别：**
- Sisyphus 擅长编排和协调，Hephaestus 专注于深度工程
- Hephaestus 使用 GPT-5.4 模型，在长链推理方面更强
- 适合独立完成大块工作而非分解委托

---

### Prometheus — 战略规划器

**Tab 顺序：** 3

**默认模型：** `claude-opus-4-6`

**角色定位：**
以希腊先知普罗米修斯命名，负责项目的战略规划和方案设计。Prometheus 运行一个结构化的规划状态机，通过多阶段对话生成可执行的开发计划。

**工具权限：** 只读工具 + 规划文件写入

- ✅ `read`、`grep`、`find`、`lsp` — 代码分析
- ✅ 写入 `.sisyphus/plans/` 目录 — 规划文档
- ❌ `bash`、`write`、`edit`、`delete` — 不可修改代码

**规划状态机流程：**

```
Interview → ClearanceCheck → PlanGeneration → MetisConsult → MomusReview → Done
```

1. **Interview（访谈阶段）**：与用户确认目标、范围、歧义、方案和测试策略。需要 5 项确认才能进入下一阶段。
2. **ClearanceCheck（许可检查）**：验证规划范围是否在 Agent 权限内。
3. **PlanGeneration（计划生成）**：生成结构化计划，包含任务列表、依赖关系和验收标准。
4. **MetisConsult（Metis 咨询）**：调用 Metis Agent 进行差距分析。
5. **MomusReview（Momus 审核）**：调用 Momus Agent 进行批评审核。
6. **Done（完成）**：输出最终计划文档。

**计划输出格式：**

```
PlanTask {
    id: String,
    title: String,
    description: String,
    file_references: Vec<String>,
    acceptance_criteria: Vec<String>,
    depends_on: Vec<String>,
}
```

**典型使用场景：**
- 新功能的架构设计和规划
- 重构方案的制定
- 项目里程碑规划
- 技术方案评审

---

### Atlas — 任务执行编排器

**Tab 顺序：** 4

**默认模型：** `claude-sonnet-4-6`

**角色定位：**
以希腊擎天之神阿特拉斯命名，Atlas 是任务执行的编排器。它接受 Prometheus 生成的计划，按照依赖关系逐一执行任务，并积累经验智慧（Wisdom）。

**工具权限：** 全部工具（但**不可委托**子任务）

**关键能力：**
- 按依赖顺序执行计划中的任务
- 积累和注入执行经验（Wisdom 系统）
- 维护执行笔记本（Notepad）
- 任务验证和回滚

**Wisdom 系统：**

Atlas 在执行过程中积累五类经验：

| 类型 | 说明 |
|------|------|
| `conventions` | 项目约定和编码规范 |
| `successes` | 成功的经验和模式 |
| `failures` | 失败教训和需要避免的坑 |
| `gotchas` | 意外发现和注意事项 |
| `commands` | 有效的命令和操作方式 |

这些经验会被注入后续任务的上下文，使 Atlas 越来越高效。

**Notepad 系统：**

| 字段 | 说明 |
|------|------|
| `learnings` | 执行过程中的学习记录 |
| `decisions` | 做出的技术决策 |
| `issues` | 发现的问题 |
| `verification` | 验证步骤和结果 |
| `problems` | 未解决的问题 |

**典型使用场景：**
- 执行 Prometheus 生成的开发计划
- 多步骤的实现任务
- 需要顺序执行的复杂修改

---

### Oracle — 架构顾问

**Tab 顺序：** 5

**默认模型：** `claude-opus-4-6`

**角色定位：**
以希腊神谕之地命名，Oracle 是只读的架构分析 Agent。它不能修改任何文件，专注于提供代码结构分析、架构建议和技术评审。

**工具权限：** 仅只读工具

- ✅ `read`、`grep`、`find`、`lsp` — 代码阅读和搜索
- ❌ `bash`、`write`、`edit`、`delete`、`task` — 全部禁止

**关键能力：**
- 深度代码架构分析
- 设计模式识别和建议
- 依赖关系梳理
- 技术债务评估
- 安全审查建议

**典型使用场景：**
- "帮我分析这个模块的架构"
- "这个设计有什么潜在问题？"
- "如何改善代码的可维护性？"
- 代码审查（只读分析，不做修改）

---

### Librarian — 文档搜索

**Tab 顺序：** 6

**默认模型：** `minimax-m2.7`

**角色定位：**
Librarian 是专注于文档和知识检索的 Agent。使用轻量级模型，快速搜索项目文档、README、注释和配置文件中的信息。

**工具权限：** 仅只读工具

- ✅ `read`、`grep`、`find` — 文档和文件搜索
- ❌ 所有写入和执行工具

**关键能力：**
- 项目文档搜索和引用
- API 文档查询
- 配置文件解读
- 注释和文档字符串检索

**典型使用场景：**
- "这个函数的文档在哪里？"
- "项目的 README 里说了什么关于部署的？"
- "搜索所有关于认证的文档"

**为什么使用 minimax-m2.7？**
文档搜索任务相对简单，不需要复杂推理。使用轻量级模型可以显著降低成本并提高响应速度。

---

### Explore — 代码搜索

**Tab 顺序：** 7

**默认模型：** `grok-code-fast-1`

**角色定位：**
Explore 是专注于代码搜索和导航的 Agent。使用专门优化过的代码模型，快速定位符号定义、使用方式和代码模式。

**工具权限：** 仅只读工具

- ✅ `read`、`grep`、`find`、`lsp`、`ast_grep` — 代码搜索和分析
- ❌ 所有写入和执行工具

**关键能力：**
- 符号定义和引用搜索
- 正则表达式代码搜索
- AST 级别的代码模式匹配
- 跨文件的调用链追踪
- LSP 集成（跳转定义、查找引用）

**典型使用场景：**
- "找到所有使用 `UserService` 的地方"
- "这个函数是在哪里定义的？"
- "搜索所有 TODO 和 FIXME 注释"
- "展示这个接口的实现类"

**为什么使用 grok-code-fast-1？**
Grok 的代码模型针对代码理解和搜索进行了优化，响应速度快，对代码结构的理解能力强。

---

### Metis — 计划顾问

**Tab 顺序：** 8

**默认模型：** `claude-opus-4-6`

**角色定位：**
以希腊智慧女神墨提斯命名，Metis 是 Prometheus 规划工作流中的差距分析顾问。它审查计划草案，识别遗漏和不完善之处。

**工具权限：** 仅只读工具

**关键能力：**
- 计划完整性分析
- 差距识别和补充建议
- 技术可行性评估
- 风险点标注

**在规划工作流中的位置：**

```
Prometheus(PlanGeneration) → Metis(差距分析) → Momus(批评审核) → 最终计划
```

Metis 的反馈会被注入 Prometheus 的上下文，用于改进计划。

**典型使用场景：**
- 自动参与 Prometheus 规划流程（无需手动调用）
- 也可独立使用：分析任意计划文档的完整性

---

### Momus — 计划审核

**Tab 顺序：** 9

**默认模型：** `gpt-5.4`

**角色定位：**
以希腊批评之神摩墨斯命名，Momus 是计划审核 Agent。它对 Prometheus 生成的计划进行严格审查，提出批评意见和改进建议。

**工具权限：** 仅只读工具

**关键能力：**
- 计划质量审核
- 逻辑一致性检查
- 可执行性评估
- 风险和成本分析
- 建设性批评和改进建议

**与 Metis 的区别：**

| 维度 | Metis | Momus |
|------|-------|-------|
| 侧重 | 差距分析（缺什么） | 批评审核（有什么问题） |
| 模型 | claude-opus-4-6 | gpt-5.4 |
| 风格 | 建设性补充 | 批判性审查 |

使用不同模型确保审核视角的多样性，避免单一模型的偏见。

---

### Junior — 任务执行器

**Tab 顺序：** 10

**默认模型：** 按 Category 路由动态分配

**角色定位：**
Junior 是一个通用的任务执行 Agent，其模型由 Category 路由系统根据任务类型动态决定。它**不能委托**子任务，只能自己完成分配的工作。

**工具权限：** 全部工具（但**不可委托**）

- ✅ `read`、`write`、`edit`、`bash`、`grep` 等
- ❌ `task` — 不可委托子任务

**Category 路由规则：**

| 任务类型 | 分配的 Category | 模型 |
|---------|----------------|------|
| 视觉/UI 工程 | `visual-engineering` | `gemini-3.1-pro` |
| 深度推理 | `ultrabrain` | `gpt-5.4` (xhigh) |
| 自主解决 | `deep` | `gpt-5.4` (medium) |
| 创意工作 | `artistry` | `gemini-3.1-pro` (high) |
| 快速任务 | `quick` | `gpt-5.4-mini` |
| 低难度 | `unspecified-low` | `claude-sonnet-4-6` |
| 高难度 | `unspecified-high` | `claude-opus-4-6` (max) |
| 文档写作 | `writing` | `gemini-3-flash` |

**典型使用场景：**
- 被 Sisyphus 或 Hephaestus 委托的子任务
- Atlas 编排中的独立执行单元
- 简单、独立的开发任务

---

### Multimodal — 视觉分析

**Tab 顺序：** 11

**默认模型：** `gpt-5.4`

**角色定位：**
Multimodal 是处理视觉输入的 Agent，能够分析图片、截图、设计稿和图表。使用白名单模式限制工具访问。

**工具权限：** 白名单模式（仅允许特定工具）

- ✅ `read`、`browser`（截图分析）
- ❌ 大部分工具被限制

**关键能力：**
- 截图和图片分析
- UI/UX 设计审查
- 图表和架构图理解
- 视觉内容描述和提取

**典型使用场景：**
- "分析这个截图中的错误信息"
- "这个 UI 设计有什么可以改进的？"
- "解读这个架构图"
- "比较两个版本的界面差异"

---

## Agent 协作模式

### 委托模式

具有委托能力的 Agent（Sisyphus、Hephaestus、Prometheus）可以通过 `task` 工具将子任务分配给其他 Agent：

```
Sisyphus → task("实现 UserService") → Junior
Sisyphus → task("分析架构") → Oracle
Prometheus → task("审核计划") → Momus
```

### 规划-执行模式

```
用户需求 → Prometheus(规划) → Metis(差距分析) → Momus(审核) → Atlas(执行)
```

### 任务交接

使用 `/handoff` 命令将当前任务从一个 Agent 转交给另一个：

```
/handoff hephaestus    # 将任务交给 Hephaestus
```

### Agent 间通信

通过 `ceair-mesh` 消息总线实现：

- **直接消息**：Agent 间的点对点通信
- **任务协商**：多 Agent 讨论任务分配
- **任务重分配**：运行时动态调整任务归属
- **共享状态**：Agent 间共享工作上下文

---

## 自定义 Agent 配置

在 `.opencode/ceair.jsonc` 中覆盖 Agent 默认配置：

```jsonc
{
  "agents": {
    "sisyphus": {
      "model": "claude-sonnet-4-6",      // 使用更快的模型
      "temperature": 0.5,                 // 降低创造性
      "max_tokens": 8192,                 // 增加输出长度
      "thinking_level": "High"            // 提高推理深度
    },
    "junior": {
      "default_category": "deep",         // 默认使用 deep 类别
      "model": "gpt-5.4"                  // 固定模型
    },
    "hephaestus": {
      "timeout_secs": 600,                // 增加超时时间
      "max_iterations": 100               // 增加最大迭代次数
    }
  }
}
```

### Thinking Level 说明

| 级别 | 说明 | 适用场景 |
|------|------|---------|
| `Off` | 关闭推理 | 简单任务 |
| `Minimal` | 最小推理 | 快速响应 |
| `Low` | 低推理 | 常规编码 |
| `Medium` | 中等推理（默认） | 大部分任务 |
| `High` | 深度推理 | 复杂问题 |
| `XHigh` | 极限推理 | 最难问题 |

更多配置选项请参阅 [配置参考](configuration.md)。
