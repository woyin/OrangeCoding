# TODO — Coding Agent 系统升级计划

> 基于 `docs/analysis.md` 分析报告生成的实现任务列表。
> 每条 TODO 可独立实现，复杂度不超过约 1 小时。

---

## 状态说明

- `[ ]` 未开始
- `[~]` 进行中
- `[x]` 已完成
- `[!]` 阻塞

---

## tools/ — 工具系统增强

### [T-001] 工具元数据扩展

**状态**: `[x]`

**目标**:
为 Tool trait 增加元数据方法，支持并发安全性、只读性、破坏性等属性声明。

**设计**:
1. 在 `ceair-tools/src/lib.rs` 中定义 `ToolMetadata` 结构体
2. 为 Tool trait 增加 `fn metadata(&self) -> ToolMetadata` 默认方法
3. ToolMetadata 包含: `is_read_only`, `is_concurrency_safe`, `is_destructive`, `is_enabled`
4. 提供安全默认值（TOOL_DEFAULTS 模式）: read_only=false, concurrency_safe=false, destructive=false, enabled=true

**验证方法**:
- 单元测试: 验证默认值正确
- 单元测试: 验证自定义覆盖生效
- 单元测试: 验证现有工具默认行为不变
- 编译测试: 现有工具代码无需修改即可编译

**依赖**: 无

---

### [T-002] 输入验证层

**状态**: `[x]`

**目标**:
为 Tool trait 增加 `validate_input` 方法，在执行前进行语义校验。

**设计**:
1. 定义 `ValidationResult` 枚举: `Ok`, `Warning(String)`, `Error(String)`
2. 为 Tool trait 增加 `fn validate_input(&self, params: &Value) -> ValidationResult` 默认实现（返回 Ok）
3. 在 `ToolExecutor::execute_tool_call` 中，call() 前调用 validate_input
4. Warning 记录日志但继续执行，Error 直接返回 ToolResult::error

**验证方法**:
- 单元测试: 默认实现返回 Ok
- 单元测试: Error 时阻止执行
- 单元测试: Warning 时继续执行
- 集成测试: EditTool 自定义校验（如检测空内容）

**依赖**: 无

---

### [T-003] 权限检查层

**状态**: `[x]`

**目标**:
为 Tool trait 增加 `check_permissions` 方法，支持 allow/deny/ask 三种权限决策。

**设计**:
1. 定义 `PermissionDecision` 枚举: `Allow`, `Deny(String)`, `Ask(String)`
2. 定义 `PermissionContext` 结构体: 包含 `working_dir`, `allowed_paths`, `denied_patterns`
3. 为 Tool trait 增加 `fn check_permissions(&self, params: &Value, ctx: &PermissionContext) -> PermissionDecision`
4. 默认实现返回 `Allow`
5. 在 `ToolExecutor` 中，validate_input 后、call 前调用 check_permissions
6. Deny 直接返回错误，Ask 通过回调询问用户

**验证方法**:
- 单元测试: 默认返回 Allow
- 单元测试: Deny 时阻止执行
- 单元测试: PermissionContext 的路径匹配
- 单元测试: BashTool 的危险命令检测

**依赖**: 无

---

### [T-004] 并发执行分区

**状态**: `[x]`

**目标**:
将 `execute_batch` 改为按读写安全性分区执行：只读工具并发，写入工具串行。

**设计**:
1. 使用 T-001 的 `ToolMetadata::is_concurrency_safe` 进行分区
2. 实现 `partition_tool_calls(calls) -> Vec<Batch>` 函数
3. 每个 Batch 包含: `calls: Vec<ToolCall>`, `concurrent: bool`
4. 按原始顺序扫描，连续的 concurrency_safe 工具组成并发 Batch，
   非安全工具单独组成串行 Batch
5. 修改 `execute_batch` 使用分区策略

**验证方法**:
- 单元测试: 全只读工具 → 单个并发 Batch
- 单元测试: 全写入工具 → 多个单元素 Batch
- 单元测试: 混合工具 → 正确分区
- 单元测试: 空列表 → 空结果
- 集成测试: 验证分区后执行结果正确

**依赖**: T-001

---

### [T-005] 执行后钩子系统

**状态**: `[x]`

**目标**:
为工具执行增加 post-execution hook 链，支持结果修改、进度报告和阻塞错误。

**设计**:
1. 定义 `PostToolHook` trait: `async fn on_tool_complete(&self, ctx: &HookContext) -> HookResult`
2. `HookContext` 包含: tool_name, input, output, duration, is_error
3. `HookResult` 枚举: `Continue`, `ModifyOutput(String)`, `BlockingError(String)`
4. 在 `ToolExecutor` 中增加 `hooks: Vec<Arc<dyn PostToolHook>>` 字段
5. 执行完成后按顺序调用所有钩子
6. BlockingError 将成功结果转为错误

**验证方法**:
- 单元测试: 无钩子时行为不变
- 单元测试: Continue 钩子不影响结果
- 单元测试: ModifyOutput 正确修改结果
- 单元测试: BlockingError 阻止成功
- 单元测试: 多钩子按顺序执行

**依赖**: 无

---

## context/ — 上下文管理增强

### [C-001] 微压缩（MicroCompact）

**状态**: `[x]`

**目标**:
实现实时截断旧工具输出的微压缩层，无需 AI 参与。

**设计**:
1. 在 `ceair-agent/src/compaction.rs` 中新增 `MicroCompactor` 结构体
2. 定义可压缩工具白名单: `["file_read", "bash", "grep", "glob", "web_search", "fetch"]`
3. 配置项: `preserve_recent: usize`（保留最近 N 个工具结果不压缩）
4. 压缩规则: 超过 preserve_recent 的旧工具结果，内容替换为 `"[旧工具结果已清除]"`
5. 保留工具名称和调用 ID 结构
6. 时间复杂度: O(n) 单次扫描

**验证方法**:
- 单元测试: 少于 preserve_recent 时不压缩
- 单元测试: 超过时正确截断旧结果
- 单元测试: 非白名单工具不被压缩
- 单元测试: 保留工具名称和 ID
- 单元测试: 空列表返回空

**依赖**: 无

---

### [C-002] Token 预算状态机

**状态**: `[x]`

**目标**:
实现 token 预算跟踪和续写决策状态机。

**设计**:
1. 新建 `ceair-agent/src/token_budget.rs`
2. 定义 `TokenBudget` 结构体: `context_window`, `reserved_tokens`, `usage_history: Vec<usize>`
3. 定义 `BudgetDecision` 枚举: `Continue(String)`, `Stop(String)`
4. 实现 `check_budget(&self) -> BudgetDecision`:
   - budget ≤ 0 → Stop
   - 连续 3+ 轮 AND delta < 500 → Stop("收益递减")
   - 使用率 < 90% → Continue("已使用 X%")
   - 否则 → Stop
5. 实现 `record_usage(&mut self, tokens: usize)` 追踪每轮 token 用量

**验证方法**:
- 单元测试: 充足预算 → Continue
- 单元测试: 预算耗尽 → Stop
- 单元测试: 收益递减检测（3轮+小delta）
- 单元测试: 90% 阈值触发
- 单元测试: usage_history 正确记录

**依赖**: 无

---

### [C-003] 消息分组

**状态**: `[x]`

**目标**:
按 API 轮次将消息分组，为反应式压缩提供精细粒度。

**设计**:
1. 在 `ceair-agent/src/compaction.rs` 中新增 `MessageGrouper`
2. 分组规则: 每次新的 assistant 消息（不同的 message_id）开始新的组
3. 组内包含: assistant message + tool_use messages + tool_result messages
4. 首组特殊: 包含 system + user + 第一个 assistant
5. 返回 `Vec<MessageGroup>`，每组包含 messages 和 total_tokens

**验证方法**:
- 单元测试: 单轮对话 → 单组
- 单元测试: 多轮对话 → 正确分组
- 单元测试: 工具调用归属到正确的组
- 单元测试: 空列表返回空
- 单元测试: 组内 token 统计正确

**依赖**: 无

---

### [C-004] 自动压缩触发器与断路器

**状态**: `[x]`

**目标**:
实现 autoCompact 阈值检测和连续失败断路器。

**设计**:
1. 在 `ContextCompactor` 中增加 `AutoCompactConfig`:
   - `buffer_tokens: usize`（默认 13000）
   - `max_consecutive_failures: u32`（默认 3）
2. 实现 `should_auto_compact(current_tokens, context_window, reserved) -> bool`:
   - threshold = context_window - reserved - buffer_tokens
   - current_tokens > threshold → true
3. 增加 `consecutive_failures: u32` 状态
4. 失败时递增，成功时重置
5. 超过 max_consecutive_failures 时禁止自动压缩

**验证方法**:
- 单元测试: 未达阈值 → false
- 单元测试: 超过阈值 → true
- 单元测试: 连续失败后禁止
- 单元测试: 成功后重置计数
- 单元测试: 自定义 buffer 生效

**依赖**: 无

---

### [C-005] 压缩后重注入

**状态**: `[x]`

**目标**:
压缩完成后自动恢复最近读取的文件内容和活跃计划。

**设计**:
1. 定义 `ReinjectionConfig`: `max_files: usize`(5), `max_tokens_per_file: usize`(5000), `total_budget: usize`(50000)
2. 定义 `ReinjectionSource` trait: `fn get_recent_files() -> Vec<FileContent>`
3. `CompactionResult` 增加 `reinjected_context: Vec<ReinjectionItem>` 字段
4. 压缩完成后调用 `reinject()`:
   - 收集最近读取的文件（按时间排序）
   - 截断到 max_tokens_per_file
   - 总量不超过 total_budget
   - 追加到 kept_messages 前面

**验证方法**:
- 单元测试: 无文件时不注入
- 单元测试: 文件数不超过 max_files
- 单元测试: 单文件截断到 max_tokens_per_file
- 单元测试: 总量不超过 total_budget
- 单元测试: 按时间排序（最新优先）

**依赖**: C-001

---

## agent/ — Agent 架构增强

### [A-001] 任务系统基础

**状态**: `[x]`

**目标**:
实现任务生命周期管理：ID 生成、状态机、任务注册表。

**设计**:
1. 新建 `ceair-agent/src/task_system.rs`
2. 定义 `TaskId`: 前缀(1 char) + 8 位随机小写字母数字
   - 前缀: 'a'=agent, 't'=teammate, 'b'=bash, 'd'=dream
3. 定义 `TaskStatus` 枚举: `Pending`, `Running`, `Completed`, `Failed`, `Killed`
4. 实现 `is_terminal()` 方法
5. 定义 `TaskState` 结构体: id, status, created_at, updated_at, owner, metadata
6. 实现 `TaskRegistry`: 注册/查询/更新/清理终态任务

**验证方法**:
- 单元测试: TaskId 格式正确（前缀 + 8字符）
- 单元测试: TaskId 唯一性（1000次生成无重复）
- 单元测试: 状态转换合法性（终态不可逆）
- 单元测试: TaskRegistry CRUD 操作
- 单元测试: 终态任务清理

**依赖**: 无

---

### [A-002] CancellationToken 层级

**状态**: `[x]`

**目标**:
实现父子 Agent 之间的级联取消机制。

**设计**:
1. 新建 `ceair-agent/src/cancellation.rs`
2. 使用 `tokio_util::sync::CancellationToken` 或自实现
3. 定义 `CancellationHierarchy`:
   - `create_root() -> CancellationToken`
   - `create_child(parent: &CancellationToken) -> CancellationToken`
4. 父 token 取消时，所有子 token 自动取消
5. 子 token 取消不影响父
6. 支持 `is_cancelled()` 和 `cancelled().await` 两种检查方式

**验证方法**:
- 单元测试: 父取消 → 子取消
- 单元测试: 子取消 → 父不受影响
- 单元测试: 多级层级正确级联
- 单元测试: 已取消的 parent 创建的 child 立即取消
- 异步测试: cancelled().await 正确唤醒

**依赖**: 无

---

### [A-003] Fork Agent 模式

**状态**: `[x]`

**目标**:
实现 Fork 子 Agent 派生模式：继承父对话历史，独立执行。

**设计**:
1. 新建 `ceair-agent/src/fork.rs`
2. 定义 `ForkConfig`: max_turns, tool_filter(allow/deny), skip_transcript
3. 定义 `ForkResult`: messages, final_response, token_usage
4. 实现 `fork_agent(parent_context, config) -> ForkResult`:
   - 克隆父 context 的 conversation（共享 prompt cache 基础）
   - 追加 fork 专用的 user message（指令）
   - 工具过滤通过 `can_use_tool` 回调而非删除工具数组
   - 独立执行循环，不写入父 transcript
5. 防递归: 检测 fork depth，限制最大 3 层

**验证方法**:
- 单元测试: Fork 继承父对话历史
- 单元测试: 工具过滤正确（deny 模式）
- 单元测试: max_turns 限制生效
- 单元测试: 防递归检测
- 单元测试: ForkResult 结构正确

**依赖**: A-001, A-002

---

### [A-004] Agent 邮箱通信

**状态**: `[x]`

**目标**:
实现基于文件的 Agent 间异步消息传递系统。

**设计**:
1. 新建 `ceair-agent/src/mailbox.rs`
2. 邮箱路径: `~/.ceair/teams/{team_name}/inboxes/{agent_name}.json`
3. 消息结构: `MailboxMessage { from, text, timestamp, read, summary }`
4. 操作:
   - `read_mailbox(agent, team) -> Vec<MailboxMessage>`
   - `read_unread(agent, team) -> Vec<MailboxMessage>`
   - `write_to_mailbox(agent, message, team)`
   - `mark_read(agent, index, team)`
5. 文件锁: 使用 `fs2::FileExt` 的 `lock_exclusive` 防止并发写入
6. 支持结构化消息: `shutdown_request`, `shutdown_response`

**验证方法**:
- 单元测试: 读写基本消息
- 单元测试: 未读过滤正确
- 单元测试: mark_read 生效
- 单元测试: 空邮箱返回空列表
- 集成测试: 并发写入不丢失消息

**依赖**: A-001

---

## memory/ — 记忆系统增强

### [M-001] 记忆类型分类系统

**状态**: `[x]`

**目标**:
为记忆条目增加 4 种类型分类（user/feedback/project/reference）。

**设计**:
1. 定义 `MemoryType` 枚举: `User`, `Feedback`, `Project`, `Reference`
2. 为 `MemoryEntry` 增加 `memory_type: MemoryType` 字段
3. 增加 `MemoryStore::search_by_type(memory_type) -> Vec<&MemoryEntry>`
4. 修改 `add()` 方法接受 `memory_type` 参数
5. 保持向后兼容: 默认类型为 `Reference`

**验证方法**:
- 单元测试: 4 种类型的创建和查询
- 单元测试: 按类型过滤正确
- 单元测试: 默认类型为 Reference
- 单元测试: 序列化/反序列化保留类型
- 单元测试: 现有测试不破坏

**依赖**: 无

---

### [M-002] Markdown + Frontmatter 存储格式

**状态**: `[x]`

**目标**:
实现基于文件的 Memdir 存储，每个记忆一个 Markdown 文件。

**设计**:
1. 新建 `ceair-agent/src/memdir.rs`
2. 存储路径: `~/.ceair/projects/{project_hash}/memory/`
3. 文件格式:
   ```
   ---
   name: "记忆名称"
   description: "简要描述"
   type: "user"
   ---
   ## 详细内容
   ```
4. 实现 `MemdirStore`:
   - `write_memory(entry) -> PathBuf`
   - `read_memory(path) -> MemoryEntry`
   - `list_memories() -> Vec<MemoryEntry>`
   - `delete_memory(path)`
5. Frontmatter 解析使用 `---` 分隔符手动解析（避免额外依赖）
6. 文件名: `{sanitized_name}.md`

**验证方法**:
- 单元测试: 写入并读回完整性
- 单元测试: Frontmatter 解析正确
- 单元测试: 列出所有记忆文件
- 单元测试: 删除操作
- 单元测试: 特殊字符文件名处理

**依赖**: M-001

---

### [M-003] MEMORY.md 索引文件

**状态**: `[x]`

**目标**:
维护记忆目录的索引文件，限制 ≤200 行、≤25KB。

**设计**:
1. 在 `memdir.rs` 中增加 `MemoryIndex` 结构体
2. 索引格式: `- [{Type}] {name} — {description}` 每行一条
3. 实现 `rebuild_index(memories_dir) -> String`:
   - 扫描目录中所有 .md 文件
   - 提取 frontmatter 的 name, type, description
   - 按类型分组排序
   - 截断到 200 行
4. 实现 `update_index(memories_dir)`: 重建并写入 MEMORY.md
5. 大小限制: 超过 25KB 时截断最旧条目的描述

**验证方法**:
- 单元测试: 空目录 → 空索引
- 单元测试: 多文件正确索引
- 单元测试: 200 行限制
- 单元测试: 25KB 大小限制
- 单元测试: 按类型分组排序

**依赖**: M-002

---

### [M-004] 会话记忆（短期）

**状态**: `[x]`

**目标**:
实现会话内短期笔记系统，按阈值触发提取。

**设计**:
1. 新建 `ceair-agent/src/session_memory.rs`
2. 存储: `~/.ceair/session_memory/{session_id}.md`
3. `SessionMemory` 结构体:
   - `notes: Vec<String>`
   - `total_tokens: usize`
   - `last_extract_tokens: usize`
   - `tool_calls_since_extract: usize`
4. 触发条件 `should_extract()`:
   - 初始: total_tokens >= 10000
   - 更新: (delta >= 5000 AND tool_calls >= 3) OR (delta >= 5000 AND 最后一轮无工具)
5. 实现 `add_note(text)`, `get_notes() -> Vec<String>`, `save()`, `load()`

**验证方法**:
- 单元测试: 初始触发阈值（10000 tokens）
- 单元测试: 更新触发条件组合
- 单元测试: 笔记添加和检索
- 单元测试: 保存和加载持久化
- 单元测试: 禁用时不触发

**依赖**: 无

---

### [M-005] AI 驱动的记忆召回

**状态**: `[x]`

**目标**:
实现基于 AI 的语义相关性记忆召回，替代关键词搜索。

**设计**:
1. 在 `memdir.rs` 中新增 `RelevantMemoryFinder`
2. 流程:
   - 扫描所有 .md 文件，提取前 30 行
   - 按 mtime 排序（最新优先）
   - 构建 manifest: `Query: {query}\n\nAvailable memories:\n{list}`
   - 调用 AI 模型选择最多 5 条
   - 输出: `Vec<SelectedMemory>` 含路径和完整内容
3. 新鲜度警告: 超过 1 天的记忆附加 `[⚠ 此记忆可能已过期]`
4. 定义 `MemorySelector` trait 抽象 AI 调用（便于测试）

**验证方法**:
- 单元测试: Mock AI 选择正确返回
- 单元测试: 最多返回 5 条
- 单元测试: 新鲜度警告正确附加
- 单元测试: 空目录返回空
- 单元测试: 排除已展示的记忆

**依赖**: M-002, M-003

---

### [M-006] AutoDream 门控与锁

**状态**: `[x]`

**目标**:
实现 AutoDream 的触发门控条件和分布式文件锁。

**设计**:
1. 新建 `ceair-agent/src/auto_dream.rs`
2. 门控条件链（从便宜到贵）:
   - 记忆系统已启用？
   - 距上次整合 ≥ 24 小时？
   - 距上次扫描 ≥ 10 分钟？
   - ≥ 5 个新会话？
3. 分布式锁 `DreamLock`:
   - 锁文件: `~/.ceair/memory/.consolidate-lock`
   - 获取: 写入 PID → 重读验证所有权
   - 过期: mtime > 1 小时 → 可抢占
   - 释放: 删除锁文件
4. 定义 `DreamGate::should_dream() -> bool`

**验证方法**:
- 单元测试: 所有门控条件通过 → true
- 单元测试: 任一门控失败 → false
- 单元测试: 锁获取和释放
- 单元测试: 过期锁可抢占
- 单元测试: 重复获取同一锁 → 失败

**依赖**: M-002

---

### [M-007] AutoDream 整合流程

**状态**: `[x]`

**目标**:
实现 AutoDream 的四阶段记忆整合流程。

**设计**:
1. 在 `auto_dream.rs` 中新增 `DreamExecutor`
2. 四阶段:
   - Phase 1 — 定向: 读取 MEMORY.md，ls 记忆目录
   - Phase 2 — 收集: 读取会话日志，提取新信号
   - Phase 3 — 整合: AI 驱动的合并（相关主题合并，相对日期转绝对）
   - Phase 4 — 修剪: 删除矛盾记忆，更新 MEMORY.md 索引
3. 定义 `DreamPhase` 枚举追踪进度
4. 使用 Fork Agent 模式执行（沙箱化）
5. 失败时回滚: 恢复原始文件

**验证方法**:
- 单元测试: 4 个阶段按序执行
- 单元测试: 失败时回滚
- 单元测试: 索引更新正确
- 集成测试: 端到端整合流程
- 单元测试: DreamPhase 状态追踪

**依赖**: M-003, M-006, A-003

---

## verification/ — 验证系统

### [V-001] 验证 Agent 框架

**状态**: `[x]`

**目标**:
实现基于 Fork 模式的自动验证 Agent。

**设计**:
1. 新建 `ceair-agent/src/verification.rs`
2. 定义 `VerificationCheck` 枚举:
   - `DesignCompliance` — 是否符合设计文档
   - `BreakingChanges` — 是否破坏现有模块
   - `DesignDefects` — 是否存在明显缺陷
   - `TestEffectiveness` — 测试是否有效
   - `MeaninglessTests` — 是否存在空测试
3. 定义 `VerificationResult`: `passed: bool`, `issues: Vec<Issue>`, `suggestions: Vec<String>`
4. 实现 `VerificationAgent::verify(context, checks) -> VerificationResult`:
   - Fork 子 Agent，工具设为只读（deny write tools）
   - 传入变更描述和设计文档引用
   - 解析 Agent 返回的结构化验证结果
5. 失败策略: 返回 issues，由调用方决定是否阻止 commit

**验证方法**:
- 单元测试: 验证检查类型完整
- 单元测试: Mock fork 返回的解析
- 单元测试: 通过/失败结果正确构建
- 单元测试: 多种检查类型组合
- 单元测试: 空变更 → 自动通过

**依赖**: A-003

---

### [V-002] 工具使用摘要

**状态**: `[x]`

**目标**:
为工具执行生成 git-commit 风格的简短摘要。

**设计**:
1. 在 `ceair-agent/src/tool_summary.rs` 中实现
2. 输入: tool_name, input(截断到 300 字符), output(截断到 300 字符), recent_assistant_msg(200 字符)
3. 输出: 1 行摘要（如 "Searched in auth/", "Fixed NPE in UserService"）
4. 实现 `ToolSummaryGenerator`:
   - `generate_summary(tool_name, input, output) -> String`
   - 简单工具（Read, Grep）使用模板生成（不需要 AI）
   - 复杂工具（Bash, Edit）使用 AI 生成
5. 定义 `SummaryTemplate` 用于模板匹配

**验证方法**:
- 单元测试: Read 工具摘要格式正确
- 单元测试: Grep 工具摘要格式正确
- 单元测试: 输入截断到 300 字符
- 单元测试: 空输出的处理
- 单元测试: 模板匹配优先于 AI

**依赖**: 无

---

## buddy/ — Buddy System

### [B-001] 确定性身份生成

**状态**: `[x]`

**目标**:
基于用户 ID hash 生成不可篡改的确定性 Buddy 身份。

**设计**:
1. 新建 `ceair-agent/src/buddy.rs`
2. 实现 Mulberry32 PRNG（32 位乘法 PRNG）
3. 种子: `hash(user_id + SALT)` 使用 SHA-256
4. 从 PRNG 确定性选择:
   - 稀有度: Common(60%), Uncommon(25%), Rare(10%), Epic(4%), Legendary(1%)
   - 物种: 18 种变体
   - 属性: 5 个维度（DEBUGGING, PATIENCE, CHAOS, WISDOM, SNARK），各 1-10
5. `BuddyIdentity` 结构体: species, rarity, attributes, name
6. `generate_buddy(user_id) -> BuddyIdentity` — 同一 user_id 始终生成同一 buddy

**验证方法**:
- 单元测试: 同一 user_id → 同一 buddy
- 单元测试: 不同 user_id → 不同 buddy
- 单元测试: 稀有度分布近似预期（统计测试，1000 次）
- 单元测试: Mulberry32 输出确定性
- 单元测试: 所有属性在 1-10 范围内

**依赖**: 无

---

### [B-002] 异步观察者模式

**状态**: `[x]`

**目标**:
实现 Buddy 的异步反应生成，作为 Fork 子 Agent 在后台观察。

**设计**:
1. 在 `buddy.rs` 中新增 `BuddyObserver`
2. 数据流:
   - 接收最近 N 条消息
   - Fork 子 Agent，全部工具 deny
   - 系统提示: "你是 {buddy_name}，简短评论你观察到的"
   - 提取文本回复（第一个 text block）
3. `BuddyReaction` 结构体: text, timestamp, display_ticks(20)
4. 异步执行: `tokio::spawn` 后台运行，结果通过 channel 返回
5. 非阻塞: 主 Agent 循环不等待 buddy 反应

**验证方法**:
- 单元测试: BuddyReaction 结构正确
- 单元测试: display_ticks 默认 20
- 单元测试: Mock fork 返回提取正确
- 异步测试: 非阻塞执行验证
- 单元测试: 空消息不触发观察

**依赖**: A-003, B-001

---

## kairos/ — KAIROS 决策辅助

### [K-001] 后采样钩子系统

**状态**: `[x]`

**目标**:
实现模型响应后、工具执行前的后采样钩子链。

**设计**:
1. 新建 `ceair-agent/src/post_sampling.rs`
2. 定义 `PostSamplingHook` trait:
   `async fn on_response(&self, ctx: &PostSamplingContext) -> PostSamplingResult`
3. `PostSamplingContext`: messages, tool_calls, app_state ref
4. `PostSamplingResult`: `Continue` | `InjectMessage(Message)` | `ModifyToolCalls(Vec<ToolCall>)`
5. 实现 `PostSamplingPipeline`:
   - `register(hook: Arc<dyn PostSamplingHook>)`
   - `execute(ctx) -> Vec<PostSamplingResult>` 并行执行所有钩子
6. 在 agent_loop 中，模型返回后、工具执行前调用

**验证方法**:
- 单元测试: 无钩子时透传
- 单元测试: Continue 不影响流程
- 单元测试: InjectMessage 正确注入
- 单元测试: 多钩子并行执行
- 单元测试: 钩子异常不崩溃主流程

**依赖**: 无

---

### [K-002] Prompt Suggestion 引擎

**状态**: `[x]`

**目标**:
实现基于上下文的下一步操作预测系统。

**设计**:
1. 新建 `ceair-agent/src/prompt_suggestion.rs`
2. 生成流程:
   - Fork 子 Agent，全部工具 deny
   - 提示: "预测用户下一步操作，2-12 个词"
   - 应用 16 个拒绝过滤器
3. 拒绝过滤器（部分）:
   - 太短（<2 词）或太长（>12 词）
   - 包含格式化标记
   - 包含评价性语言
   - 包含 AI 腔调
   - 包含错误消息
   - 多个句子
4. 抑制条件: 早期对话、错误状态、限速
5. `PromptSuggestion` 结构体: text, prompt_id, confidence

**验证方法**:
- 单元测试: 16 个过滤器各自的拒绝逻辑
- 单元测试: 有效建议通过所有过滤器
- 单元测试: 抑制条件生效
- 单元测试: prompt_id 唯一
- 单元测试: 空上下文不生成建议

**依赖**: A-003, K-001

---

### [K-003] 上下文感知 Tips

**状态**: `[x]`

**目标**:
实现基于上下文和冷却期的提示系统。

**设计**:
1. 新建 `ceair-agent/src/tips.rs`
2. `Tip` 结构体: id, text, context_tags, min_session_gap
3. `TipStore`:
   - 内置提示库（~20 条常用提示）
   - `show_history: HashMap<String, u64>` 记录每条提示的上次展示会话号
   - `get_tip(context) -> Option<Tip>`:
     1. 按上下文标签过滤
     2. 按距上次展示的会话数排序（最久优先）
     3. 返回冷却期已过的最佳提示
   - `record_shown(tip_id, session_number)`

**验证方法**:
- 单元测试: 按上下文过滤正确
- 单元测试: 冷却期内不重复展示
- 单元测试: 最久未展示优先
- 单元测试: 无匹配提示返回 None
- 单元测试: 展示历史正确记录

**依赖**: 无

---

## 依赖关系图

```
T-001 ──→ T-004
T-002 (独立)
T-003 (独立)
T-005 (独立)

C-001 ──→ C-005
C-002 (独立)
C-003 (独立)
C-004 (独立)

A-001 ──→ A-003, A-004
A-002 ──→ A-003 (需要 A-001, A-002)
A-001 + A-002 ──→ A-003
A-004 ──→ (独立)

M-001 ──→ M-002 ──→ M-003, M-005
M-004 (独立)
M-006 (独立, 依赖 M-002)
M-007 (依赖 M-003, M-006, A-003)

V-001 (依赖 A-003)
V-002 (独立)

B-001 (独立)
B-002 (依赖 A-003, B-001)

K-001 (独立)
K-002 (依赖 A-003, K-001)
K-003 (独立)
```

## 建议实现顺序

**第 1 批（无依赖）**:
T-001, T-002, T-003, T-005, C-001, C-002, C-003, C-004, A-001, A-002, M-001, M-004, M-006, V-002, B-001, K-001, K-003

**第 2 批（依赖第 1 批）**:
T-004, C-005, A-003, A-004, M-002

**第 3 批（依赖第 2 批）**:
M-003, M-005, V-001, B-002, K-002

**第 4 批（依赖第 3 批）**:
M-007
