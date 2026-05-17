# Go Harness Engineering Notes

本文件记录 `master` 分支 Go 版本的 agent harness 强化方向。

## 依据

这次调整参考了公开的 agent harness 工程实践：

- OpenAI Agents SDK 的 sessions/context 管理强调会话历史、恢复中断运行、限制历史检索和自动压缩。
- OpenAI Agents SDK 的 tracing/guardrails 强调运行过程可观测、可审计和可控。
- OpenAI reasoning 文档提供 `reasoning_effort` 一类 provider 级推理预算入口。
- Anthropic extended thinking 使用 `thinking.budget_tokens` 为复杂任务分配显式思考预算。
- Anthropic prompt caching/context 文档强调长上下文应稳定复用、必要时裁剪，而不是无限增长。

参考链接：

- <https://openai.github.io/openai-agents-js/guides/sessions/>
- <https://openai.github.io/openai-agents-python/tracing/>
- <https://platform.openai.com/docs/guides/reasoning>
- <https://docs.anthropic.com/en/docs/build-with-claude/prompt-engineering/extended-thinking-tips>

## 已落地能力

### 长任务

`modules/agent/harness_profile.go` 新增 `LongTaskPolicy`：

- 默认启用长任务模式。
- 默认 `MaxIterations` 从 20 调整为 60。
- 增加 `MaxToolCalls`，防止 agent 在工具循环中无界运行。
- 增加 `ProgressSnapshot` 和 `StopReason`，让调用方能区分完成、取消、provider 错误、迭代上限和工具预算耗尽。
- 每轮模型调用前按 `CompactionMaxTokens` 压缩旧上下文。

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

全量 Go 模块测试命令：

```bash
go test ./modules/core ./modules/ai ./modules/audit ./modules/config \
  ./modules/control-protocol ./modules/session ./modules/tools \
  ./modules/agent ./modules/mesh ./modules/mcp ./modules/tui \
  ./modules/worker ./modules/control-server ./modules/cli ./modules/invariant
```
