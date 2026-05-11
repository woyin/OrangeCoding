# BUG_FIX.md — Code Review 高优先级问题建模

> 生成时间：2026-04-11
> 状态：阶段 2 — 已复核并修复高优先级问题（2026-05-04）

---

## BUG-001：check_permissions 在工具执行路径中未被调用

优先级：**P1**

状态：**已修复**

修复摘要：
- `ToolExecutor::execute_tool_call` 在执行前调用 `validate_input` 和 `check_permissions`。
- `ToolRegistry::execute` 改为标准执行管线，并新增 `execute_with_permissions` 支持显式权限上下文。
- `orangecoding-cli` 的工具调用路径使用当前工作目录构造权限上下文，避免直接 registry 调用绕过权限检查。

问题：
Tool trait 声明了 `check_permissions` 方法（lib.rs:231-237），契约明确要求"在 validate_input 后、execute 前调用"。但在所有运行时执行入口中，权限检查被完全跳过，工具直接执行。这意味着任何工具对 `check_permissions` 的覆盖（返回 `Deny` 或 `Ask`）都是无效的——权限层在运行时是 no-op。

根因：
代码定义了权限接口但未在执行路径中调用。具体遗漏两个入口：
1. **`ToolRegistry::execute`**（registry.rs:167-172）：直接 `tool.execute(params).await`，无权限检查
2. **`ToolExecutor::execute_tool_call`**（executor.rs:74-128）：直接 `tool.execute(params)` 后加 timeout，无权限检查

权限系统存在 `PermissionContext`、`PermissionDecision` 等完整类型定义（permissions.rs），`check_permissions` 方法也有测试覆盖（lib.rs:675-756），但没有任何执行路径在运行时调用它。此外 `ToolExecutor` 不持有 `PermissionContext`，因此即使要调用也缺少上下文。

影响范围：
- `crates/orangecoding-tools/src/lib.rs` — Tool trait 定义
- `crates/orangecoding-tools/src/registry.rs` — ToolRegistry::execute
- `crates/orangecoding-tools/src/permissions.rs` — PermissionContext, PermissionDecision
- `crates/orangecoding-agent/src/executor.rs` — ToolExecutor::execute_tool_call, execute_batch
- 所有 22+ 内置工具的权限覆盖实现（如有）

修复策略：
1. **在 `ToolExecutor::execute_tool_call` 中增加权限检查**：在调用 `tool.execute(params)` 前，调用 `tool.check_permissions(&params, &permission_ctx)`，根据返回决策决定是否继续执行
2. **`ToolExecutor` 需持有 `PermissionContext`**：通过构造函数或 builder 方法注入
3. **`ToolRegistry::execute` 也需增加权限检查**：防止绕过 ToolExecutor 直接调用 registry
4. 决策映射：
   - `Allow` → 继续执行
   - `Deny(reason)` → 返回 `ToolResult::error(id, reason)`
   - `Ask(prompt)` → 当前阶段直接 Deny（无用户交互通道），或记录日志后 Allow

涉及文件（最多 3 个）：
- `crates/orangecoding-agent/src/executor.rs` — 增加权限检查逻辑
- `crates/orangecoding-tools/src/registry.rs` — 增加权限检查逻辑
- `crates/orangecoding-tools/src/lib.rs` — 可能需要调整 execute_with_permissions 签名（如需）

验证方法：
1. 创建覆盖 `check_permissions` 返回 `Deny` 的 MockTool，验证 `execute_tool_call` 返回错误而非执行
2. 创建覆盖 `check_permissions` 返回 `Ask` 的 MockTool，验证行为符合预期
3. 验证默认实现（返回 `Allow`）的工具仍正常执行
4. 搜索所有 `tool.execute(` 调用点，确认无 bypass 路径
5. 运行全量测试确认无回归

风险：
- `PermissionContext` 注入可能需要修改 `ToolExecutor::new` 签名，影响调用方
- `Ask` 决策的处理策略需明确（当前 ToolExecutor 无用户交互能力）
- `execute_batch` 通过 `execute_tool_call` 间接调用，需确认批量场景下权限检查正确

---

## BUG-002：execute_batch 未使用 batch_partition 进行安全分区

优先级：**P1**

状态：**已修复**

修复摘要：
- `ToolExecutor::execute_batch` 已接入 `partition_tool_calls`，并按并发安全元数据分批执行。
- 批量执行使用原始调用索引恢复结果顺序，避免重复 `call_id` 时回找错误。

问题：
`batch_partition` 模块实现了基于 `ToolMetadata.is_concurrency_safe` 的工具调用分区（batch_partition.rs:55-91），设计目标是将写入类工具（edit_file, write_file, bash）隔离为串行批次，防止并发竞态。但 `ToolExecutor::execute_batch`（executor.rs:140-165）直接用 `join_all` 并行执行所有工具调用，完全绕过分区逻辑。当模型一次返回多个工具调用时，写入操作会与相邻调用并发执行，可能导致文件读写的竞态条件和交叉副作用。

根因：
`execute_batch` 的实现中没有任何对 `batch_partition` 模块的引用。全局搜索确认：
- `orangecoding-agent` 中无 `partition_tool_calls`、`ToolCallInfo`、`ExecutionBatch` 的导入或使用
- `execute_batch` 注释（executor.rs:132）明确写"使用 `tokio::join!` 语义（通过 `futures::future::join_all`）并行执行所有工具调用"

此外，要使用 partitioner 需要将 `ToolCall`（agent 层数据结构）转换为 `ToolCallInfo`（partition 层数据结构），并查询 `ToolMetadata.is_concurrency_safe`，但当前 `ToolExecutor` 不持有足够的元数据查询能力。

影响范围：
- `crates/orangecoding-tools/src/batch_partition.rs` — 分区逻辑（已实现但未使用）
- `crates/orangecoding-agent/src/executor.rs` — execute_batch
- `crates/orangecoding-tools/src/lib.rs` — ToolMetadata, Tool::metadata()
- `crates/orangecoding-agent/src/agent_loop.rs:287` — 唯一调用 execute_batch 的位置

修复策略：
1. **在 `execute_batch` 中引入分区逻辑**：
   - 遍历 `tool_calls`，对每个调用查找对应工具的 `metadata().is_concurrency_safe`
   - 构建 `Vec<ToolCallInfo>` 调用 `partition_tool_calls`
   - 对返回的 `Vec<ExecutionBatch>` 顺序执行：`concurrent=true` 的批次用 `join_all`，`concurrent=false` 的批次顺序执行
2. **保持向后兼容**：如果工具未注册（无法查 metadata），默认为 unsafe（保守策略）
3. **保持结果顺序一致**：分区执行后，结果需按原始 tool_calls 顺序重新排列

涉及文件（最多 2 个）：
- `crates/orangecoding-agent/src/executor.rs` — 重写 execute_batch
- （可能）`crates/orangecoding-tools/src/batch_partition.rs` — 如需调整接口

验证方法：
1. 构造包含 safe + unsafe 工具的混合调用列表，验证 unsafe 工具不与相邻调用并发
2. 验证所有 safe 工具仍并发执行（性能不退化）
3. 验证结果顺序与输入顺序一致
4. 验证空列表和单元素列表的边界情况
5. 运行现有 `test_execute_batch` 和 `test_execute_batch_empty` 确认无回归

风险：
- 执行语义变化：某些调用方可能依赖全并发行为，改为分区后时序不同
- 需确保结果顺序不变——partition 改变了执行时序，但返回值顺序必须对齐输入
- `ToolCall` → `ToolCallInfo` 转换需要 `registry.get()` 查找，增加了一次查找开销

---

## BUG-003：OutputTruncationHook 按字节截断导致 UTF-8 panic

优先级：**P2**

状态：**已修复**

修复摘要：
- `OutputTruncationHook` 已改为按字符截断，不再使用可能切断 UTF-8 边界的字节切片。
- 截断统计改为字符数语义。

问题：
`OutputTruncationHook::on_tool_complete`（tool_hooks.rs:185-199）使用 `&ctx.output[..self.max_chars]` 截断字符串。Rust 的字符串索引是字节级的，而非字符级。当输出包含多字节 UTF-8 字符（如中文、emoji）且 `max_chars` 的字节边界恰好落在字符中间时，切片操作会产生无效 UTF-8，导致 panic。

具体触发路径：
- Line 187: `ctx.output.len() > self.max_chars` — `len()` 返回字节数
- Line 190: `&ctx.output[..self.max_chars]` — 字节级切片

例如：中文输出 `"你好世界"`（12 字节），`max_chars=5` 时，`&s[..5]` 会切断第二个 UTF-8 字符，直接 panic。

根因：
混淆了"字符数"和"字节数"的语义。字段名 `max_chars` 和注释"指定字符数"暗示的是字符级截断，但实现用的是字节级切片。Rust 的 `str` 索引要求字节边界对齐 UTF-8 字符边界，否则 panic。

影响范围：
- `crates/orangecoding-tools/src/tool_hooks.rs` — OutputTruncationHook（lines 172-199）
- 所有使用此 hook 的 post-tool 执行路径
- 任何产生非 ASCII 输出的工具（bash、python、read_file 等都可能产生中文输出）

修复策略：
将字节级切片替换为字符级截断。最小改动方案：
```rust
// 之前（byte-based，会 panic）：
&ctx.output[..self.max_chars]

// 之后（char-safe）：
let truncated: String = ctx.output.chars().take(self.max_chars).collect();
```
同时修正 `ctx.output.len()` 的语义：截断判断条件应改为字符数比较 `ctx.output.chars().count() > self.max_chars`，或保持字节比较但修正截断方式。

最优方案（避免 `chars().count()` 的 O(n) 开销）：
使用 `is_char_boundary()` 找到最近的安全截断点：
```rust
let truncation_point = ctx.output
    .char_indices()
    .take_while(|(byte_idx, _)| *byte_idx < self.max_chars)
    .last()
    .map(|(i, c)| i + c.len_utf8())
    .unwrap_or(0);
&ctx.output[..truncation_point]
```

涉及文件（1 个）：
- `crates/orangecoding-tools/src/tool_hooks.rs`

验证方法：
1. 构造包含中文的输出，`max_chars` 恰好落在多字节字符中间，验证不 panic 且输出完整字符
2. 构造纯 ASCII 输出，验证截断行为与之前一致
3. 构造空字符串、短于 max_chars 的字符串，验证不触发截断
4. 构造包含 emoji（4 字节 UTF-8）的输出，验证边界情况
5. 运行现有 `test_truncation_below_limit` 和 `test_truncation_above_limit` 确认无回归

风险：
- `chars().count()` 是 O(n) 操作，对于长输出可能有性能影响——可通过 `is_char_boundary` 方案规避
- 截断后实际字符数可能少于 `max_chars`（因为多字节字符不能分割），但这是正确行为

---

## BUG-004：memory recall 未按 modified_at 排序导致召回质量退化

优先级：**P2**

状态：**已修复**

修复摘要：
- `recall_memories` 已在 AI selector 前按 `modified_at` 降序排序。
- 相同修改时间使用原始顺序作为稳定 tie-breaker。

问题：
`recall_memories` 函数（memory_recall.rs:88-140）的文档注释声称流程第一步是"按修改时间排序（最新优先）"，但实际代码中不存在任何排序操作。`filtered_summaries`（line 110-111）仅做了 `already_shown` 过滤后直接克隆，保留了调用方的原始顺序。如果调用方传入未排序的候选列表，AI selector 看到的顺序是任意的，后续 `take(MAX_RESULTS)` 可能丢弃更新的记忆。

根因：
注释描述的排序步骤从未被实现。`recall_memories` 中：
- Line 100-103: 过滤 `already_shown`
- Line 110-111: 克隆为 `filtered_summaries`（保留原始顺序）
- Line 114: 传给 AI selector

缺少一个 `sort_by(|a, b| b.modified_at.cmp(&a.modified_at))` 步骤。

影响范围：
- `crates/orangecoding-agent/src/memory_recall.rs` — recall_memories 函数
- 所有调用 `recall_memories` 的上层逻辑
- 记忆系统的召回质量

修复策略：
在构建 `filtered_summaries` 后、调用 `selector.select()` 前，按 `modified_at` 降序排序：
```rust
let mut filtered_summaries: Vec<MemorySummary> =
    filtered.iter().map(|s| (*s).clone()).collect();
filtered_summaries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
```

涉及文件（1 个）：
- `crates/orangecoding-agent/src/memory_recall.rs`

验证方法：
1. 构造 unsorted 候选列表（旧记忆在前、新记忆在后），验证排序后新记忆优先被选择
2. 验证相同 modified_at 的记忆顺序稳定
3. 验证 already_shown 过滤仍然正确
4. 验证空列表和单元素列表边界情况
5. 运行现有 `test_recall_memories_*` 系列测试确认无回归

风险：
- 排序改变了候选列表顺序，可能影响依赖原始顺序的 selector 实现
- `SystemTime` 比较在某些平台可能有精度问题，但 Rust 标准库已处理
- 排序是 O(n log n)，但候选列表通常很小（<100 条），性能影响可忽略

---

## 修复优先级排序

| 顺序 | BUG-ID | 优先级 | 风险等级 | 估计改动 |
|------|--------|--------|----------|----------|
| 1 | BUG-001 | P1 | 高 — 安全权限完全失效 | 2-3 文件 |
| 2 | BUG-002 | P1 | 高 — 并发竞态可导致数据损坏 | 1-2 文件 |
| 3 | BUG-003 | P2 | 中 — 特定输入下 panic | 1 文件 |
| 4 | BUG-004 | P2 | 低 — 召回质量退化 | 1 文件 |

## 依赖关系

- BUG-001 和 BUG-002 都涉及 `executor.rs`，建议按顺序修复（先权限检查，后分区调度）
- BUG-003 和 BUG-004 完全独立，可并行修复
