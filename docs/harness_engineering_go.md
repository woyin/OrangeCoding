# Go Harness Engineering Notes

本文件记录 `master` 分支 Go 版本的 agent harness 强化方向。

## 依据

这次调整参考了公开的 agent harness 工程实践：

- OpenAI Agents SDK 的 sessions/context 管理强调会话历史、恢复中断运行、限制历史检索和自动压缩。
- OpenAI Agents SDK 的 tracing/guardrails 强调运行过程可观测、可审计和可控。
- OpenAI reasoning 文档提供 `reasoning_effort` 一类 provider 级推理预算入口。
- Anthropic extended thinking 使用 `thinking.budget_tokens` 为复杂任务分配显式思考预算。
- Anthropic prompt caching/context 文档强调长上下文应稳定复用、必要时裁剪，而不是无限增长。
- Nyosegawa 的 Harness Engineering 文章强调先交付最小可用 harness，再逐步补齐 replay、trace 查询、语义记忆和策略配置等成熟能力。

参考链接：

- <https://openai.github.io/openai-agents-js/guides/sessions/>
- <https://openai.github.io/openai-agents-python/tracing/>
- <https://platform.openai.com/docs/guides/reasoning>
- <https://docs.anthropic.com/en/docs/build-with-claude/prompt-engineering/extended-thinking-tips>
- <https://nyosegawa.com/en/posts/harness-engineering-best-practices-2026/>

## 已落地能力

### Harness runtime

`modules/agent` 现在包含真正的 harness runtime 骨架：

- `harness_state.go` 定义显式状态机、trace event、checkpoint 和 checkpoint store 接口。
- `harness_engine.go` 提供 `HarnessEngine`，负责合法状态迁移、trace 记录和 checkpoint 保存。
- `harness_context.go` 提供 `HarnessContextBuilder`，把模型上下文拆成 system、task、memory、conversation 等可预算 block。
- `harness_memory.go` 提供 `HarnessMemoryManager`，负责 recall 和从 `FACT:` 观察中学习稳定事实。
- `harness_guardrail.go` 提供 guardrail pipeline，并内置危险 shell 命令和重复工具调用检查。
- `harness_checkpoint_file.go` 提供 JSON 文件持久化 checkpoint store；`MemoryCheckpointStore` 提供内存版。

`AgentLoop.Run` 已接入这些能力：每次运行都会创建 harness run ID、启动 checkpoint、构建上下文 block、执行 pre-tool guardrail、学习 FACT 记忆，并在状态迁移时记录 trace。

这是第一版可用 harness runtime，不是最终成熟系统。当前有状态机、context block、memory、guardrail、checkpoint 接口和 JSON 文件持久化；暂不包含 replay/resume CLI、trace 查询 API、memory 语义检索或 guardrail 策略配置文件。

工具调用准确性现在由统一的 `agent.BuildToolDefinitions` 提供，CLI、sub-agent 和 workflows 都使用真实工具 JSON schema，而不是空参数对象。长任务系统提示还会要求先选择最窄工具、按 schema 填参，并在适合并行探索、评审、验证或文档整理时生成 sub-agent delegation brief。

### 长任务

`modules/agent/harness_profile.go` 新增 `LongTaskPolicy`：

- 默认启用长任务模式。
- 默认 `MaxIterations` 从 20 调整为 60。
- 增加 `MaxToolCalls`，防止 agent 在工具循环中无界运行。
- 增加 `ProgressSnapshot` 和 `StopReason`，让调用方能区分完成、取消、provider 错误、迭代上限和工具预算耗尽。
- 每轮模型调用前按 `CompactionMaxTokens` 压缩旧上下文。
- checkpoint 可使用内存版或文件版，文件版保存为 `<checkpoint_dir>/<run_id>.json`。
- CLI 配置新增 `harness` 段，默认 `checkpoint_store: "memory"`，避免无配置时自动写入文件；显式设为 `file` 时，`checkpoint_dir` 相对 CLI 配置文件目录解析，默认写到 `~/.orangecoding/checkpoints/`。`config get/set` 支持 `harness.checkpoint_store` 这类 dotted key。

### 长推理

`ReasoningPolicy` 统一描述 harness 侧推理预算：

- OpenAI-compatible provider 会透传 `reasoning_effort`。
- Anthropic provider 会把 `ReasoningBudgetTokens` 映射为 `thinking: {type: "enabled", budget_tokens: ...}`。
- 系统提示要求模型使用充分内部推理，但只输出摘要、证据和决策理由，不输出隐藏推理链。

### 中文表达

默认 `OutputLanguageChinese`：

- 注入系统提示：默认简体中文回答。
- 保留代码、命令、路径、API 名称和错误文本原文。
- 中文输出先给结论，再给证据和下一步。

## 测试覆盖

关键测试：

- `TestAgentContext_ApplyHarnessProfileAppendsChineseLongTaskGuidance`
- `TestAgentLoop_StopsWhenLongTaskToolBudgetExceeded`
- `TestDefaultLoopConfig`
- `TestOpenAIProviderIncludesReasoningEffort`
- `TestAnthropicProviderIncludesThinkingBudget`
- `TestHarnessEngine_InMemoryStateMachineRecordsTraceAndCheckpoint`
- `TestHarnessContextBuilder_BuildsStableMemoryAndRecentBlocksWithinBudget`
- `TestHarnessMemoryManager_RecallAndLearnFacts`
- `TestHarnessGuardrailPipeline_BlocksDangerousAndRepeatedToolCalls`
- `TestFileCheckpointStore_RoundTrip`
- `TestAgentLoop_UsesHarnessGuardrailCheckpointAndMemory`
- `TestAgentLoopConfigFromCLIConfigDefaultsToInMemoryCheckpoints`
- `TestAgentLoopConfigFromCLIConfigUsesConfigSiblingCheckpointDir`
- `TestConfigManagerSetGetNestedHarnessField`
- `TestBuildToolDefinitionsPreservesToolParameterSchema`
- `TestTaskToolDelegateActionBuildsSubAgentBrief`
- `TestNewUltraWorkKeepsDefaultLongTaskHarnessPolicy`

全量 Go 模块测试命令：

```bash
go test ./modules/core ./modules/ai ./modules/audit ./modules/config \
  ./modules/control-protocol ./modules/session ./modules/tools \
  ./modules/agent/... ./modules/mesh ./modules/mcp ./modules/tui \
  ./modules/worker ./modules/control-server ./modules/cli ./modules/invariant
```

## 下一步演进方向

> 详细的行业调研和差距分析见 `docs/harness_research_2026Q2.md`。

基于对 OpenAI Agents SDK、Anthropic Claude Code 和行业 Harness Engineering 最佳实践的调研，以下是按优先级排列的改进方向：

### 高优先级

1. **Guardrail 全面接线** — 将 `pre_model`、`post_tool`、`final_output` 三个 phase 接入 `loop.go`。当前只有 `pre_tool` 被使用，其余三个是死代码。
2. **Handoff 模式** — 从 fire-and-forget sub-agent 升级为类型安全的 agent handoff（参考 OpenAI Agents SDK）。
3. **Per-Tool 调用预算** — 在全局 `MaxToolCalls` 之外，支持每个工具独立的调用次数限制。
4. **Checkpoint 原子性** — `FileCheckpointStore` 改用 write-to-temp + rename 模式，增加 List/Delete/TTL。

### 中优先级

5. **Trace 查询与导出** — 独立 `TraceStore` 接口，支持按 run_id/session_id/time_range 查询，OpenTelemetry 导出。
6. **语义记忆** — 从关键词匹配升级到 embedding-based 语义检索，支持记忆去重和过期。
7. **Guardrail 策略配置化** — 从硬编码改为外部 YAML/JSON 配置，支持 per-project 和 per-user 定制。
8. **Agent 级别 Model Settings** — 每个 agent 实例支持独立的模型参数配置。

### 长期方向

9. **Session Resume CLI** — `orangecoding resume <run_id>` / `replay <run_id>`。
10. **Streaming Backpressure** — 工具结果流式返回，背压控制。
11. **Sandbox Execution** — 文件操作和网络访问的沙箱隔离。
12. **Prompt Caching** — 利用 Anthropic/OpenAI 的 prompt caching API 降低长会话成本。

## 已完成的演进（Phase 13-17）

### Phase 13: Guardrail 全面激活 ✅

**交付文件**: `harness_guardrail.go`, `loop.go`

- `loop.go` 接入完整 4-phase guardrail pipeline：
  - `GuardrailPhasePreModel`：模型调用前评估
  - `GuardrailPhasePreTool`：工具调用前检查（原有，保留）
  - `GuardrailPhasePostTool`：工具返回后评估
  - `GuardrailPhaseFinalOutput`：最终输出前安全审查
- `GuardrailLogger`：线程安全的 guardrail 决策日志，支持 Recent/Warnings 查询
- `TokenBudgetGuardrail`：token 预算接近时发出警告
- `OutputLengthGuardrail`：输出超长时发出警告
- `LLMGuardrail`：基于外部 LLM 的安全评估 guardrail（可配置 Provider）
- 所有 guardrail 决策均通过 logger 记录，支持审计

### Phase 14: Checkpoint 生产化 ✅

**交付文件**: `harness_state.go`, `harness_checkpoint_file.go`

- `CheckpointStore` 接口扩展：新增 `List(prefix)` 和 `Delete(runID)` 方法
- `CheckpointSummary` 轻量视图类型
- `FileCheckpointStore` 原子写入（write-to-temp + rename）
- `FileCheckpointStoreWithTTL` 支持 TTL-based 自动过期
- `CleanupExpired()` 方法主动清理过期 checkpoint
- `List` 返回按 `UpdatedAt` 降序排列的结果

### Phase 15: Trace 与可观测性 ✅

**交付文件**: `harness_trace.go`

- `TraceStore` 接口：`Append` + `Query`
- `TraceQuery` 多维过滤：RunID、SessionID、FromState、ToState、时间范围、Limit
- `MemoryTraceStore`：内存版，测试用
- `FileTraceStore`：NDJSON 文件持久化，每 run 一个文件
- `TraceSchemaVersion` 版本化，确保向后兼容
- `TraceEventsToSpans`：转换为 OTLP span 格式，支持 OpenTelemetry 导出

### Phase 16: Agent Handoff 与编排 ✅

**交付文件**: `harness_handoff.go`

- `HandoffRequest` / `HandoffResult`：类型安全的 agent 间控制权转移
- `HandoffHandler` 接口：可插拔的 handoff 处理器
- `ToolUseBudget`：per-tool 调用次数限制，支持 soft/hard limit
- `AgentModelSettings`：per-agent 模型配置（model/temperature/top_p/max_tokens/reasoning）
- `Orchestrator` 任务编排器：依赖图 + 顺序调度 + 结果收集
- 支持 DAG 依赖：`ReadyTasks()` 返回所有前置依赖成功的任务

### Phase 17: 智能记忆 ✅

**交付文件**: `harness_embedding.go`

- `EmbeddingProvider` 接口：`Embed(text)` + `Dimension()`
- `SemanticMemoryEntry`：带 embedding 的记忆条目
- `SemanticMemoryManager`：
  - 存储时自动生成 embedding（如果 provider 可用）
  - Recall 使用 cosine similarity 语义检索 + keyword fallback
  - 自动去重（相似度阈值过滤）
  - TTL 过期 + 容量限制 + LRU 驱逐
  - `CleanupExpired()` 主动清理
- `cosineSimilarity` 向量计算工具函数
