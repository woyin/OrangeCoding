# Harness Engineering 与 Coding Agents：2026 Q2 行业研究

> 本文件综合 OpenAI、Anthropic 及行业最新实践，为 OrangeCoding 持续演进提供参考。

---

## 1. OpenAI 最新实践

### 1.1 OpenAI Agents SDK（Python / JS）

**核心架构更新（2025-2026）：**

| 能力 | 描述 | OrangeCoding 对标 |
|------|------|-------------------|
| **Orchestrator Pattern** | Agents SDK 引入显式 orchestrator agent，负责拆分任务、委派给 specialist agents、汇总结果。不再是简单的 agent 链式调用，而是有中心协调者。 | `modules/mesh` 的 `negotiator.go` 有类似思路，但尚未完全接入 agent loop |
| **Handoffs** | 类型安全的 agent 间切换。一个 agent 可以在运行时将对话控制权移交给另一个 agent，连同上下文一起传递。 | 尚未实现。当前 sub-agent delegation 是 fire-and-forget brief 模式 |
| **Guardrails (Input/Output)** | SDK 提供两种 guardrail：input guardrail（在用户输入到达模型前检查）和 output guardrail（在模型输出返回用户前检查）。两者都可以是同步或异步。 | `harness_guardrail.go` 定义了 4 个 phase（pre_model, pre_tool, post_tool, final_output）但只有 `pre_tool` 被接线 |
| **Tracing** | 内置分布式追踪。每次 agent run 自动生成 span tree，可追踪：agent 调用、工具执行、LLM 请求、handoff 事件。支持导出到 OpenTelemetry。 | trace 事件已记录但无查询 API、无 span tree、无 OpenTelemetry 导出 |
| **Sessions** | 会话管理器自动处理对话历史截断和上下文窗口管理。支持自定义 serializer 持久化到任何后端。 | `HarnessContextBuilder` 做了类似的事，但没有 session manager 抽象 |
| **Max Turn Agent** | `max_turns` 参数限制 agent 循环次数，超出后自动停止。配合 `agent.max_turns` 和工具级别的 `tool.max_uses` 实现精细化预算。 | `LongTaskPolicy.MaxToolCalls` 实现了全局限制，但缺少 per-tool 级别的限制 |
| **Model Settings per Agent** | 每个 agent 实例可以有独立的 model settings（temperature, top_p, max_tokens, reasoning），不再是全局配置。 | 当前 `ReasoningPolicy` 是全局的，不支持 per-agent 定制 |
| **Structured Output** | Agents SDK 深度集成 structured output（JSON schema），agent 的 final output 可以是强类型对象。 | 未实现 |

**关键设计决策：**

1. **Agent as first-class primitive**：Agent 不再是简单的 prompt + tool 列表，而是一个完整的运行时对象，拥有自己的生命周期、状态和配置。
2. **Handoff > Chaining**：优先使用 handoff 模式而非链式调用，因为 handoff 保留了完整的对话上下文。
3. **Guardrails are pluggable**：guardrail 不是硬编码的安全检查，而是可插拔的策略模块，支持自定义 LLM-based guardrail（用另一个 LLM 来评估输出安全性）。
4. **Tracing is non-negotiable**：生产环境必须能追踪每一步决策。

### 1.2 OpenAI Codex CLI

**架构要点：**

- **Sandbox-first execution**：所有代码执行在沙箱中（Docker/chroot），工具结果通过受限通道返回。
- **Autonomous + approval modes**：支持全自动执行和逐步审批两种模式。
- **File diff as first-class output**：agent 的主要输出不是代码片段，而是结构化的 file diff。
- **Multi-turn with compaction**：长时间任务自动压缩历史，保留关键决策上下文。
- **Task-level isolation**：每个任务在独立环境中运行，不共享文件系统状态。

### 1.3 OpenAI 推理模型最新进展

- **o3/o4-mini 系列**：支持 `reasoning_effort` 参数（low/medium/high），控制模型推理深度。
- **Adaptive reasoning**：模型会根据任务复杂度自动调整推理深度，用户只需要设置 effort 上限。
- **Reasoning token visibility**：reasoning tokens 现在可以通过 API 返回，用于调试和审计（但默认不返回以节省输出 token）。

---

## 2. Anthropic 最新实践

### 2.1 Claude Code 架构

**核心设计理念（从公开信息推断）：**

| 能力 | 描述 | OrangeCoding 对标 |
|------|------|-------------------|
| **Tool-based agent loop** | Claude Code 本质上是一个 agent loop：LLM → tool call → observe → LLM。工具包括文件读写、搜索、shell 执行、git 操作等。 | `loop.go` 实现了相同的模式 |
| **Extended Thinking** | Claude Code 深度利用 extended thinking（`budget_tokens`），在复杂任务中分配大量思考预算。thinking 输出作为上下文的一部分参与后续推理。 | `ReasoningPolicy` 支持 `budget_tokens` 但 thinking 输出未参与上下文 |
| **Context Window Management** | 动态上下文管理：优先保留最近消息 + 重要决策点 + 当前文件状态。使用 prompt caching 减少重复处理。 | `HarnessContextBuilder` 的优先级驱逐策略类似，但没有 prompt caching |
| **Permission Tiers** | 工具权限分三层：auto-allow（只读操作）、ask（写操作需要确认）、deny（危险操作永远拒绝）。 | `AutoApproveTools` 标志存在但未被检查 |
| **Multi-file awareness** | 通过 LSP/AST 理解项目结构，不依赖简单的文件搜索。知道哪些文件相关、哪些函数被调用。 | 未实现 |
| **Streaming with backpressure** | 流式输出支持背压控制，不会因为用户界面慢而阻塞 agent 循环。 | 未实现 |
| **Session persistence** | 会话可以跨 CLI 重启持久化，恢复时重新加载上下文。 | checkpoint 存在但无 resume CLI |

### 2.2 Anthropic Agent 最佳实践（官方文档）

Anthropic 在其 agent 构建指南中强调的关键原则：

1. **Start simple, add complexity incrementally**：不要一开始就构建复杂的 multi-agent 系统。先用单个 agent + 工具验证核心循环。
2. **Explicit tool schemas**：每个工具必须有完整的 JSON schema，包括描述、参数类型和约束。模糊的 schema 是大多数 agent 失败的根源。
3. **Structured error handling**：工具错误应该是结构化的，包含错误类型、可操作的建议和重试指引。不要返回原始错误字符串。
4. **Think in turns, not in one shot**：agent 的设计应该鼓励多轮交互，而不是试图在一个超大 prompt 中解决所有问题。
5. **Context is king**：agent 的表现直接取决于上下文质量。投资上下文管理比投资 prompt engineering 回报更高。
6. **Test with real tasks**：不要用 toy examples 测试 agent。用真实的、复杂的工程任务测试。

### 2.3 Anthropic Computer Use 与 Tool Use

- **Tool use as the primary interface**：Anthropic 将 tool use 定位为 agent 与世界交互的主要方式，而非文本生成。
- **Parallel tool execution**：Claude 原生支持并行工具调用（multiple tool_use blocks in one response），agent 框架应该利用这一点。
- **Tool result streaming**：长时间运行的工具应该流式返回结果，而不是等完成后一次性返回。
- **Computer use for UI testing**：screen → screenshot → click/type → observe 循环，适用于 GUI 自动化。

---

## 3. Harness Engineering 行业趋势

### 3.1 Nyosegawa 的 Harness Engineering 框架

**核心框架 — 5 级成熟度模型：**

| 级别 | 能力 | OrangeCoding 当前 |
|------|------|-------------------|
| L1: Minimal | 状态机 + 基础工具执行 + 超时 | ✅ 已实现 |
| L2: Observable | Trace + checkpoint + 日志 + 指标 | ⚠️ 部分实现（trace 记录但无查询，checkpoint 无 atomicity） |
| L3: Resumable | Resume from checkpoint + replay + 断点续跑 | ❌ 未实现 |
| L4: Intelligent | 语义记忆 + 自适应上下文 + 策略配置 | ⚠️ 部分实现（FACT 记忆但无语义检索，guardrail 硬编码） |
| L5: Self-evolving | 从运行历史学习 + 自动调优 + A/B 测试策略 | ⚠️ Rust 侧有 Phase 11 框架，但未与 Go harness 集成 |

**关键建议：**

1. **Don't build L5 before L2**：先做好可观测性，再谈智能化。
2. **Checkpoint must be atomic**：checkpoint 写入必须是原子操作（write-to-temp + rename），否则崩溃恢复不可靠。
3. **Trace schema should be stable**：trace 事件的 schema 应该向后兼容，否则升级后历史数据不可用。
4. **Guardrail policies should be external**：安全策略应该在代码外部（配置文件/数据库），而不是硬编码。
5. **Memory needs semantics**：关键词匹配的记忆系统在规模增长后会失效，需要 embedding-based 检索。

### 3.2 行业 Coding Agent 架构趋势（2025-2026）

**趋势 1：Agent Loop as Infrastructure**

Coding agent 的核心循环正在从应用逻辑变成基础设施：
- Agent loop 不再是简单的 while 循环，而是一个有状态的运行时
- 状态迁移、checkpoint、恢复是核心能力
- 工具执行、上下文管理、安全检查是可插拔的中间件

**趋势 2：Multi-Agent Orchestration**

从单 agent 走向多 agent 编排：
- Orchestrator agent 负责任务分解和委派
- Specialist agents 负责特定领域（搜索、编辑、测试、审查）
- Agent 间通过结构化消息通信，而非自然语言
- 关键挑战：上下文同步和冲突解决

**趋势 3：Sandbox-First Security**

安全不再是事后的限制层，而是架构基础：
- 所有文件操作在沙箱中执行
- 网络访问按白名单控制
- 工具权限是声明式的，不是代码中的 if-else
- 安全策略可按项目/环境定制

**趋势 4：Streaming-First Architecture**

从请求-响应模式转向流式架构：
- LLM 输出流式处理
- 工具结果流式返回
- 用户界面实时更新
- 背压控制防止资源耗尽

**趋势 5：Context Engineering over Prompt Engineering**

上下文工程正在取代提示工程成为主要优化方向：
- 精确控制进入上下文窗口的内容
- 分优先级管理不同类型的上下文（系统、任务、历史、记忆）
- 动态裁剪策略而非固定截断
- Prompt caching 减少重复处理成本

---

## 4. 差距分析与改进建议

### 4.1 高优先级（架构级改进）

#### P0: Guardrail 全面接线

**现状**：4 个 guardrail phase 中只有 `pre_tool` 被使用。
**目标**：所有 4 个 phase 都被正确接线。

```
改进点：
- loop.go 中在模型调用前执行 GuardrailPhasePreModel
- loop.go 中在工具返回后执行 GuardrailPhasePostTool  
- loop.go 中在最终输出前执行 GuardrailPhaseFinalOutput
- GuardrailResult.Warn 应触发日志记录和可能的用户确认
- 增加 LLM-based guardrail 支持（用轻量模型评估输出安全性）
```

**参考**：OpenAI Agents SDK 的 input/output guardrail 模式。

#### P1: Handoff 模式

**现状**：sub-agent 是 fire-and-forget delegation brief。
**目标**：支持类型安全的 agent handoff，包含完整上下文传递。

```
改进点：
- 定义 HandoffRequest/HandoffResult 类型
- Agent 可以在运行时将控制权移交给另一个 Agent
- 移交时传递：对话历史、当前任务状态、已用工具列表
- 接收方 Agent 可以选择移交回来或继续
- 支持嵌套 handoff（A→B→C）
```

**参考**：OpenAI Agents SDK 的 handoff 机制。

#### P2: Per-Tool 调用预算

**现状**：只有全局 `MaxToolCalls`。
**目标**：每个工具可以独立设置调用次数上限。

```
改进点：
- ToolMetadata 增加 MaxUses 字段
- AgentLoop 维护 per-tool 调用计数器
- 超出限制时返回结构化错误而非静默跳过
- 支持 "soft limit"（警告）和 "hard limit"（拒绝）
```

**参考**：OpenAI Agents SDK 的 `tool.max_uses`。

#### P3: Checkpoint 原子性与查询

**现状**：文件写入非原子，无 List/Delete 操作，无查询能力。
**目标**：生产级 checkpoint 管理。

```
改进点：
- FileCheckpointStore 使用 write-to-temp + rename 实现原子写入
- 增加 List(runIDPrefix) []CheckpointSummary
- 增加 Delete(runID) error
- 增加 TTL-based 自动清理
- 考虑 SQLite 后端用于结构化查询
```

**参考**：Nyosegawa L2→L3 过渡要求。

### 4.2 中优先级（能力增强）

#### P4: Trace 查询与导出

**现状**：trace 事件被记录到 checkpoint，但无独立查询接口。
**目标**：结构化 trace 存储，支持查询和 OpenTelemetry 导出。

```
改进点：
- 独立的 TraceStore 接口（不嵌入 Checkpoint）
- 按 run_id / session_id / time_range / state 查询
- 导出为 OpenTelemetry spans
- CLI 命令：orangecoding trace list / trace show <run_id>
```

#### P5: 语义记忆

**现状**：关键词匹配的 FACT 提取。
**目标**：embedding-based 语义检索。

```
改进点：
- 记忆存储增加 embedding 字段
- 支持多种 embedding provider（OpenAI text-embedding-3-small, 本地模型）
- Recall 改为向量相似度搜索 + 关键词混合
- 记忆去重（相似度阈值）
- 记忆过期和容量限制
```

#### P6: Guardrail 策略配置化

**现状**：危险命令和重复调用规则硬编码。
**目标**：外部配置文件定义 guardrail 规则。

```
改进点：
- 定义 guardrail policy YAML/JSON schema
- 支持 per-project (.orangecoding/guardrails.yaml) 和 per-user 配置
- 内置策略：dangerous-commands, repeated-calls, token-budget, output-length
- 支持 custom guardrail（注册自定义检查函数或 LLM-based guardrail）
- 热重载（文件变更时自动更新策略）
```

#### P7: Agent 级别 Model Settings

**现状**：ReasoningPolicy 是全局配置。
**目标**：每个 agent 实例可以有独立的模型设置。

```
改进点：
- AgentConfig 增加 ModelSettings 字段
- 支持 per-agent: model, temperature, top_p, max_tokens, reasoning
- orchestrator 可以为不同 sub-agent 使用不同模型
- 例如：规划用强模型，执行用快模型，审查用强模型
```

### 4.3 低优先级（长期方向）

#### P8: Session Resume CLI

```
orangecoding resume <run_id>   # 从 checkpoint 恢复运行
orangecoding replay <run_id>   # 重放 trace 用于调试
orangecoding runs list         # 列出所有 checkpoint
```

#### P9: Streaming Backpressure

- 工具结果流式返回（特别是长时间运行的 shell 命令）
- 背压控制（UI 消费慢时不阻塞 agent 循环）
- 渐进式结果展示

#### P10: Sandbox Execution

- 文件操作在沙箱目录中执行
- 网络 API 调用按白名单控制
- 工具权限声明式配置
- 参考 OpenAI Codex CLI 的 sandbox 模式

#### P11: Multi-Agent 编排增强

- Orchestrator agent 自动任务分解
- Agent 间结构化消息通信
- 共享工作空间（文件、变量）的冲突解决
- Agent 池和负载均衡

#### P12: Prompt Caching

- 利用 Anthropic/OpenAI 的 prompt caching API
- 保持 system prompt 前缀稳定
- 减少长会话的重复 token 处理成本

---

## 5. 行动路线图

### Phase 13: Guardrail 全面激活

**目标**：将所有 guardrail phase 接入 agent loop。

**交付物**：
- `loop.go` 中增加 pre_model / post_tool / final_output 检查点
- GuardrailResult.Warn 处理逻辑
- 基础 LLM-based guardrail 接口
- 测试：4 个 phase 各有 happy path / violation / boundary 测试

### Phase 14: Checkpoint 生产化

**目标**：checkpoint 存储达到生产质量。

**交付物**：
- 原子文件写入（write-to-temp + rename）
- List / Delete / TTL 清理
- SQLite 后端（可选）
- CLI 命令：`runs list` / `runs show` / `runs delete`

### Phase 15: Trace 与可观测性

**目标**：结构化 trace 存储，支持查询和导出。

**交付物**：
- 独立 TraceStore 接口
- 按 run_id / session_id / time_range 查询
- CLI 命令：`trace list` / `trace show`
- OpenTelemetry 导出适配器

### Phase 16: Agent Handoff 与编排

**目标**：类型安全的 agent handoff 和基础编排。

**交付物**：
- HandoffRequest / HandoffResult 类型
- Agent 间上下文传递
- Per-tool 调用预算
- Per-agent model settings
- Orchestrator agent 基础骨架

### Phase 17: 智能记忆

**目标**：从关键词记忆升级到语义记忆。

**交付物**：
- Embedding provider 接口
- 向量相似度 + 关键词混合检索
- 记忆去重和过期
- 跨会话知识迁移

---

## 6. 参考资源

### OpenAI

- OpenAI Agents SDK (Python): https://github.com/openai/openai-agents-python
- OpenAI Agents SDK (JS): https://github.com/openai/openai-agents-js
- Agents SDK Handoffs: https://openai.github.io/openai-agents-python/handoffs/
- Agents SDK Guardrails: https://openai.github.io/openai-agents-python/guardrails/
- Agents SDK Tracing: https://openai.github.io/openai-agents-python/tracing/
- Reasoning Models: https://platform.openai.com/docs/guides/reasoning
- Codex CLI: https://github.com/openai/codex

### Anthropic

- Building Agents: https://docs.anthropic.com/en/docs/build-with-claude/agent-patterns
- Tool Use: https://docs.anthropic.com/en/docs/build-with-claude/tool-use
- Extended Thinking: https://docs.anthropic.com/en/docs/build-with-claude/extended-thinking
- Computer Use: https://docs.anthropic.com/en/docs/build-with-claude/computer-use
- Prompt Caching: https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching
- Claude Code: https://docs.anthropic.com/en/docs/claude-code

### 行业

- Nyosegawa Harness Engineering: https://nyosegawa.com/en/posts/harness-engineering-best-practices-2026/
- Model Context Protocol (MCP): https://modelcontextprotocol.io/
- OpenTelemetry: https://opentelemetry.io/

---

*最后更新：2026-05-17*
