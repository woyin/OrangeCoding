# Reference 代码系统化分析报告

> 本文档基于 `reference/` 目录中的 TypeScript 参考实现进行系统分析，
> 提取可迁移到当前 Rust 系统的设计模式和核心实现方式。

---

## 目录

1. [Tools Calling](#1-tools-calling)
2. [Context 管理](#2-context-管理)
3. [Agent 架构](#3-agent-架构)
4. [Verification Agent](#4-verification-agent)
5. [Memory 系统](#5-memory-系统)
6. [Buddy System](#6-buddy-system)
7. [KAIROS](#7-kairos)

---

## 1. Tools Calling

### 解决的问题

AI 模型需要安全、可靠地调用外部工具（文件操作、Shell 命令、代码搜索等），
同时需要处理权限控制、参数校验、并发执行和错误恢复等复杂场景。

### 设计思想

采用 **"注册-发现-校验-执行-格式化"** 五阶段管道模式：
- 工具通过 `buildTool()` 工厂函数注册，内置安全默认值
- 运行时通过权限上下文过滤可用工具
- 三层校验（Schema 校验 → 语义校验 → 权限校验）保证安全性
- 支持并行批量执行和流式进度回报
- 错误不使用重试，而是诊断根因

### 核心实现方式

#### 1.1 工具定义结构

```
buildTool(def) → BuiltTool
  - TOOL_DEFAULTS 提供安全默认值：
    isEnabled: () => true
    isConcurrencySafe: () => false    // 默认不允许并发
    isReadOnly: () => false
    isDestructive: () => false
    checkPermissions: () => 'allow'
  - 用户定义覆盖默认值
  - 类型系统确保完整签名
```

#### 1.2 调用流程

```
AI 请求 ToolCall
  ↓
1. validateInput()     — 工具特定规则校验
   - 语义校验（如检测无效编辑）
   - 安全校验（如检测密钥泄露）
   - 文件存在性和过期检查
   ↓
2. checkPermissions()  — 权限决策
   - deny：直接拒绝
   - allow：自动通过
   - ask：请求用户确认
   ↓
3. call()              — 执行工具，支持流式进度回报
   ↓
4. mapToolResult()     — 序列化结果为 API 格式
```

#### 1.3 参数校验

- 使用 Zod **strictObject** 确保严格参数校验
- 支持 **discriminatedUnion** 实现类型安全的多态输出
- **semanticNumber** 自定义类型强制转换（如 "10" → 10）
- **lazySchema** 延迟加载解决循环依赖

#### 1.4 错误处理策略

```
错误分类：
  - 权限错误：在执行前拦截
  - 参数错误：validateInput() 返回 errorCode
  - 执行错误：工具运行时异常
  - 超时错误：tokio::timeout 保护
  - 语义错误：命令退出码的上下文感知解释

关键设计：不使用重试循环，而是诊断根因
  grep 退出码 1 ≠ 错误（无匹配）
  diff 退出码 1 ≠ 错误（有差异）
```

#### 1.5 并发执行模型

```
partitionToolCalls(toolUseMessages) → Batch[]
  ├─ 安全只读工具（Read, Glob, Grep, LSP）→ 并发执行
  └─ 写入工具（Edit, Write）→ 串行执行

runTools():
  for each Batch:
    if isConcurrencySafe:
      runToolsConcurrently()
    else:
      runToolsSerially()
```

#### 1.6 文件编辑的过期检测

```
lastWriteTime = getFileModificationTime(path)
if lastWriteTime > readTimestamp:
  // 回退策略：如果是完整读取，比较内容
  if isFullRead && content == readTimestamp.content:
    return OK  // 时间戳变了但内容没变（Windows 云同步）
  return REJECT  // 内容确实变了
```

### 可迁移到当前系统的设计

1. **三层校验管道**：当前 `ToolExecutor` 只有单层执行，需增加 `validateInput` 和 `checkPermissions`
2. **并发分区策略**：当前 `execute_batch` 无差别并发，需按读写分区
3. **语义错误解释**：为 BashTool 增加命令退出码语义映射
4. **工具默认值模式**：`TOOL_DEFAULTS` 减少样板代码
5. **文件过期检测**：为 EditTool 增加读写时间戳追踪

---

## 2. Context 管理

### 解决的问题

AI 对话的上下文窗口有限（通常 100K-200K tokens），但用户会话可能持续数小时、
产生数百万 tokens 的工具输出。需要在保留关键信息的同时控制 token 用量。

### 设计思想

采用 **"分层递进压缩"** 策略，从轻量级到重量级依次应用：
1. **微压缩（MicroCompact）**：实时截断旧工具输出
2. **会话记忆压缩**：快速丢弃旧消息但保留结构
3. **自动压缩（AutoCompact）**：AI 生成对话摘要
4. **反应式压缩（ReactiveCompact）**：API 返回 prompt-too-long 时紧急压缩

### 核心实现方式

#### 2.1 Token 预算跟踪

```
checkTokenBudget() 状态机：
  - 如果 budget ≤ 0 或在多 Agent 上下文中：STOP
  - 如果连续 3+ 轮 AND delta < 500 tokens：STOP（收益递减）
  - 如果使用率 < 90%：CONTINUE（添加提示："已使用 X%"）
  - 否则：STOP

阈值常量：
  COMPLETION_THRESHOLD = 0.9 (90%)
  DIMINISHING_THRESHOLD = 500 tokens/轮
```

#### 2.2 Token 估算

```
估算优先链：
  1. countTokensWithAPI()        — 精确 API 计数
  2. countTokensViaHaikuFallback() — Haiku 模型回退
  3. roughTokenCountEstimation() — 字符数估算
     JSON: 2 bytes/token
     默认: 4 bytes/token
```

#### 2.3 微压缩（MicroCompact）

```
目的：实时截断旧工具输出，不需要 AI 参与

只压缩可压缩工具：FileRead, Shell, Grep, Glob, WebSearch, WebFetch
规则：
  - 如果消息年龄 > 阈值 AND 超过保留数量
  - 替换为: "[Old tool result content cleared]"
  - 保留工具结构和名称

时间复杂度：O(n) 扫描，无 AI 调用
```

#### 2.4 自动压缩（AutoCompact）

```
触发条件：
  effectiveContextWindow = model.contextWindow - reservedTokens
  autoCompactThreshold = effectiveContextWindow - 13000 (buffer)

压缩流程：
  1. 调用前钩子（用户/插件自定义指令）
  2. stripImagesFromMessages()  — 图片替换为 [image]
  3. stripReinjectedAttachments() — 移除技能附件
  4. streamCompactSummary()     — AI 生成摘要
     失败时截断最旧的 API 轮次并重试（最多 3 次）
  5. 压缩后重新注入：
     - 最近读取的文件（最多 5 个，每个 ≤5K tokens）
     - 技能附件（≤25K tokens）
     - 活跃计划

断路器：连续 3 次失败后停止自动压缩
```

#### 2.5 压缩摘要的关键保留项

```
摘要必须包含：
  1. 主要请求和意图
  2. 关键技术概念
  3. 文件和代码片段（完整代码块）
  4. 错误和修复
  5. 问题解决过程
  6. 所有用户消息（非工具结果）
  7. 待办任务
  8. 当前工作（包含近期上下文）
  9. 可选的下一步（直接引用）
```

#### 2.6 消息分组

```
按 API 轮次分组：
  Group 0: [user, assistant(id=1), tool_use(id=1), tool_result]
  Group 1: [assistant(id=2), tool_use(id=2), tool_result]
  Group 2: [user, assistant(id=3)]

好处：反应式压缩可以按组丢弃最旧的轮次
```

#### 2.7 上下文折叠（Context Collapse）

```
实验性功能，用于超长会话：
  - 将历史任务折叠为摘要节点
  - 支持展开/导航
  - 与自动压缩互斥（避免死锁）
```

### 可迁移到当前系统的设计

1. **分层压缩策略**：当前 `ContextCompactor` 只有单层全量压缩，需增加微压缩层
2. **Token 预算状态机**：增加收益递减检测和续写提示
3. **按 API 轮次分组**：为反应式压缩提供精细粒度
4. **断路器模式**：连续失败后停止自动压缩
5. **压缩后重注入**：自动恢复最近文件和技能上下文
6. **CJK 感知的 Token 估算**：当前已有基础实现，需增加文件类型感知

---

## 3. Agent 架构

### 解决的问题

复杂任务需要多个专业化 Agent 协作，每个 Agent 有不同的模型偏好、工具权限和行为约束。
需要支持从简单的子任务委派到复杂的团队协作等多种协作模式。

### 设计思想

三层架构：
1. **任务系统（Task System）**：低级执行原语（进程、Agent、Shell）
2. **团队系统（Team/Swarm）**：高级 Agent 协调和管理
3. **通信层（Mailbox）**：基于文件的异步消息传递

### 核心实现方式

#### 3.1 任务生命周期

```
任务类型：
  local_bash        — Shell 命令执行
  local_agent       — 后台 Agent（独立进程）
  in_process_teammate — 进程内 Agent（同一运行时）
  remote_agent      — 远程 Agent
  dream             — 记忆整理任务

状态机：
  pending → running → completed | failed | killed
  终态不可逆转

任务 ID 生成：
  前缀 + 8 位随机字符（36^8 ≈ 2.8 万亿组合）
  前缀标识类型：a=agent, t=teammate, b=bash
```

#### 3.2 三种 Agent 派生模式

```
A. Fork 模式（隐式子进程）
   - 继承父 Agent 的完整对话历史和系统提示
   - 共享 prompt cache（缓存命中率 ~92%）
   - 防止递归 fork
   - 子 Agent 输出结构化结果：Scope, Result, Key files, Issues

B. 命名子 Agent 模式（显式类型）
   - 通过 subagent_type 指定角色（researcher, test-runner 等）
   - 从 loadAgentsDir 加载配置
   - 工具白名单或全量
   - 权限模式：bubble（弹出到父终端）

C. 团队模式（进程内 Teammate）
   - 多 Agent 共享同一运行时
   - AsyncLocalStorage 隔离上下文
   - 标识格式：agentName@teamName
```

#### 3.3 团队系统

```
创建团队：
  TeamCreateTool → .claude/teams/{name}/team.json
  分配 team lead（创建者）

生成 Teammate：
  TeammateIdentity { agentId, agentName, teamName, color, planModeRequired }
  AbortController 层级：父 abort → 子 abort 级联
  消息上限：UI 镜像 50 条（防止内存爆炸）

通信方式：
  文件邮箱：.claude/teams/{team}/inboxes/{agent}.json
  轮询间隔：500ms
  支持结构化消息：shutdown_request, plan_approval_response
  支持广播：to="*"
```

#### 3.4 协调者模式

```
协调者 Agent（主 Agent）
  ↓
派生 Worker Agent
  ↓
Worker 有限制的工具池：
  SIMPLE: Read, Glob, Grep, Bash, FileEdit
  FULL: 所有分析和文件工具
  禁止：TeamCreate, TeamDelete, SendMessage（防止递归）
```

#### 3.5 Agent Summary（进度报告）

```
每 30 秒一次后台摘要：
  Fork 子 Agent，共享 prompt cache
  提示："用 3-5 个词描述最近动作（-ing 形式），说文件/函数名"
  示例："Reading runAgent.ts"
  约束："说新的东西，不要重复"
```

### 可迁移到当前系统的设计

1. **任务 ID 前缀策略**：当前系统可采用类型前缀 + 随机字符
2. **Fork 模式的 cache 共享**：当前系统需实现 prompt cache 共享机制
3. **基于文件的邮箱通信**：适合 Rust 的进程间通信
4. **AbortController 层级**：Rust 中可用 `CancellationToken` 实现
5. **消息上限保护**：防止大规模 Agent 团队内存爆炸
6. **工具黑白名单**：当前 `AgentDefinition` 已有 `blocked_tools`，需增加白名单模式

---

## 4. Verification Agent

### 解决的问题

Agent 在自主执行任务时可能产生错误、不完整或违反设计约束的结果。
需要独立的验证机制来保证输出质量。

### 设计思想

**非阻塞质量门**：验证作为异步后台任务运行，不阻塞主 Agent 流程。
使用 Fork 模式共享 prompt cache，以最小成本获得验证信号。

### 核心实现方式

#### 4.1 工具执行编排（验证基础）

```
工具执行后的钩子链：
  runPostToolUseHooks<Input, Output>()
    对每个钩子：
      传入 toolUseContext, tool, toolInput, toolResponse
      钩子可返回：
        - AttachmentMessage（展示给用户）
        - ProgressMessage（更新进度）
        - { updatedMCPToolOutput }（修改输出）
        - { blockingError }（阻止执行）
```

#### 4.2 工具使用摘要（验证辅助）

```
生成 git-commit 风格的工具使用摘要：
  1. 截断工具输入/输出到 300 字符
  2. 提取最近助手消息（200 字符）
  3. Fork Haiku 生成 1 行摘要
  示例："Searched in auth/", "Fixed NPE in UserService"
```

#### 4.3 Fork 验证链

```
验证用例：
  1. Prompt Suggestion — 预测下一步
  2. Agent Summary    — 描述进度（30s 更新）
  3. Magic Docs Update — 自动刷新文档
  4. Speculation       — 预计算结果
  5. Tool Use Summary  — 摘要批量操作

模板：
  runForkedAgent({
    promptMessages: [userMessage],
    cacheSafeParams,              // 复用父 cache
    canUseTool: () => 'deny',     // 禁用工具
    querySource: 'verification',
    skipTranscript: true,         // 不记录到会话
    skipCacheWrite: true,         // 不污染 cache
  })

关键：保持与父相同的 system + tools + messages 参数
  → cache 命中率 ~92%
  → 验证成本约为正常请求的 1/10
```

#### 4.4 Speculation（投机执行验证）

```
预执行建议的操作，展示结果给用户：
  - 最多 20 轮，100 条消息
  - 使用 overlay 文件系统隔离（临时目录 + nonce）
  - 相同权限检查
  - 随时可中止
  - 用户接受后 copy overlay 回主文件系统

状态追踪：
  SpeculationState =
    | idle
    | active { boundary: complete | bash | edit | denied_tool }
```

### 可迁移到当前系统的设计

1. **Post-Tool 钩子链**：当前 `ToolExecutor` 无钩子，需增加执行后回调
2. **Fork 验证模式**：低成本验证，复用 prompt cache
3. **工具使用摘要**：为长任务提供进度可见性
4. **Speculation 机制**：overlay 文件系统隔离的试运行
5. **非阻塞验证**：验证结果不阻塞主流程，作为信号供用户参考

---

## 5. Memory 系统

### 解决的问题

Agent 需要跨会话记住用户偏好、项目约定、技术决策等信息。
会话内需要追踪工作进度，空闲时需要整理和淘汰过期记忆。

### 设计思想

五个协调子系统：
1. **Memdir**：基于文件的分类记忆存储
2. **Session Memory**：会话内短期笔记
3. **Extract Memories**：后台自动提取持久记忆
4. **AutoDream**：空闲时记忆整合
5. **Away Summary**：用户离开时的简要回顾

### 核心实现方式

#### 5.1 记忆目录（Memdir）

```
存储位置：~/.claude/projects/<sanitized-git-root>/memory/

文件格式（Markdown + Frontmatter）：
  ---
  name: "用户是数据科学家"
  description: "用户角色和专注领域"
  type: "user"
  ---
  ## 详细内容...

4 种类型（封闭分类）：
  user     — 角色、目标、偏好（始终私有）
  feedback — 指导：避免什么/重复什么（默认私有）
  project  — 进行中的工作、里程碑、截止日期（偏向团队）
  reference — 外部系统指针（通常团队共享）

索引文件 MEMORY.md（仅指针，不存内容）：
  - [User Profile](user_profile.md) — 数据科学家, 偏好简洁
  - [Testing Policy](feedback_db_mocking.md) — 不要 mock DB
  限制：≤200 行, ≤25KB
```

#### 5.2 查询时记忆召回

```
findRelevantMemories(query):
  1. 扫描阶段：读取所有 .md 文件（≤200 个）
     提取 frontmatter 的前 30 行
     按 mtime 排序（最新优先）
  2. 过滤阶段：排除 MEMORY.md 和已展示的
  3. 选择阶段：Sonnet 模型选择最多 5 条相关记忆
     输入格式：Query: {query}\n\nAvailable memories:\n{manifest}
     输出 schema：JSON { selected_memories: string[] }
     max_tokens: 256
  4. 新鲜度警告：>1 天的记忆附加过期提醒

关键设计：使用 AI 模型做语义相关性匹配，而非关键词搜索
```

#### 5.3 会话记忆（Session Memory）

```
目的：会话内短期笔记（不跨会话持久化）
存储：~/.claude/session_memory/<session-id>.md

触发逻辑 shouldExtractMemory()：
  初始化：tokenCount >= 10000
  更新：
    (tokenDelta >= 5000 AND toolCalls >= 3) OR
    (tokenDelta >= 5000 AND 最后一轮无工具调用)

执行方式：Fork 子 Agent，权限沙箱化
  允许：FileRead, Grep, Glob, 只读 Bash
  允许写入：仅记忆目录内
  禁止：MCP, Agent, 写入 Bash
```

#### 5.4 AutoDream（空闲时整合）

```
触发门控（从最便宜到最贵）：
  1. KAIROS 活跃？→ 跳过
  2. 远程模式？→ 跳过
  3. 自动记忆已启用？→ 继续
  4. 时间门：距上次 ≥ 24 小时
  5. 扫描节流：距上次扫描 ≥ 10 分钟
  6. 会话门：≥ 5 个新会话
  7. 分布式锁：文件锁 + PID + 过期检测

四阶段梦境：
  Phase 1 — 定向：ls 记忆目录，读 MEMORY.md
  Phase 2 — 收集信号：读日志，grep 会话记录
  Phase 3 — 整合：合并到主题文件，转换相对日期为绝对
  Phase 4 — 修剪和索引：更新 MEMORY.md，删除矛盾记忆

分布式锁机制：
  锁文件：.consolidate-lock
  获取：写入 PID → 重读验证所有权
  过期：mtime < 1 小时 AND PID 存活 → 阻塞
  回滚：失败时恢复原始 mtime
```

#### 5.5 Away Summary

```
用户回来时的简短摘要（1-3 句）：
  - 高层任务描述（在构建/调试什么）
  - 具体下一步
  - 跳过状态报告和 commit 回顾

使用快速小模型，读取最后 30 条消息 + 会话记忆
```

### 可迁移到当前系统的设计

1. **文件型记忆存储**：当前 `MemoryStore` 用 JSON，需增加 Markdown + Frontmatter 格式
2. **AI 驱动的相关性召回**：当前关键词搜索需升级为语义选择
3. **4 种类型分类**：当前只有 `tags`，需增加结构化类型
4. **AutoDream 整合**：当前 `consolidate` 仅合并，需增加 AI 驱动的整理
5. **分布式锁**：当前无锁机制，多进程场景需要
6. **新鲜度警告**：当前无过期提醒，需为旧记忆附加验证建议
7. **Fork 子 Agent 提取**：当前记忆提取是同步的，需改为后台 Fork

---

## 6. Buddy System

### 解决的问题

主 Agent 在自主执行时缺乏外部反馈。需要一个"伙伴"角色提供非侵入式观察、
验证信号和情感连接，但不干扰主流程。

### 设计思想

**异步观察者模式**：Buddy 作为 Fork 子 Agent 在后台观察主 Agent 的行为，
生成简短评论。通过确定性身份系统建立个性化连接。

### 核心实现方式

#### 6.1 确定性身份生成

```
算法：hash(userId + SALT) → Mulberry32 PRNG seed

从 PRNG 掷骰：
  稀有度：Common(60%), Uncommon(25%), Rare(10%), Epic(4%), Legendary(1%)
  物种：18 种变体（鸭子、水滴、猫、龙等）
  眼睛：6 种风格（·, ✦, ×, ◉, @, °）
  帽子：8 种选项
  闪光：1% 概率
  属性：5 个（DEBUGGING, PATIENCE, CHAOS, WISDOM, SNARK）

关键设计：
  - Bones（外观）每次从 hash 重新生成 → 不可篡改
  - Soul（个性）存储一次 → 持久化
  - 读取时 bones 覆盖存储的 bones → 防止破坏旧伙伴
```

#### 6.2 异步反应生成

```
数据流：
  用户输入/Agent 回复
    ↓
  fireCompanionObserver(messages)
    ↓
  Fork 子 Agent：
    System: "你是伙伴，简短评论你观察到的"
    Messages: 最近 N 轮
    Tools: 全部禁用（canUseTool → deny）
    Model: Sonnet（成本优化）
    ↓
  提取文本回复（第一个 text block）
    ↓
  setAppState({ companionReaction: reaction })
    ↓
  CompanionSprite 渲染气泡动画（20 ticks ≈ 10秒后淡出）
```

#### 6.3 视觉反馈系统

```
精灵动画：
  IDLE_SEQUENCE = [0,0,0,0,1,0,0,0,-1,0,0,2,0,0,0]
  70% 静止，偶尔动作，偶尔眨眼

气泡机制：
  显示时长：20 ticks（~10 秒）
  淡出窗口：最后 6 ticks
  文本换行：最大 30 字符
  尾巴指向伙伴

宠物动画：
  触发：/buddy pet 命令
  效果：爱心飘浮 5 ticks（~2.5 秒）

终端适配：
  窄屏（<100 列）：紧凑模式
  宽屏：完整精灵 + 气泡
```

### 可迁移到当前系统的设计

1. **确定性身份系统**：基于用户 ID hash 的不可篡改身份生成
2. **异步观察者模式**：Fork 子 Agent 后台观察，不阻塞主流程
3. **反应信号机制**：AppState 中的 companionReaction 作为验证信号
4. **成本优化**：使用 canUseTool 禁用工具而非移除工具数组（保持 cache 命中）
5. **TUI 精灵动画**：适合当前 ratatui 框架的动画系统

---

## 7. KAIROS

### 解决的问题

Agent 需要在正确的时机做出正确的决策：何时建议下一步、何时预计算结果、
何时更新文档、何时显示提示。需要一个上下文感知的决策辅助系统。

### 设计思想

**多信号决策引擎**：整合多个独立的上下文信号源（Prompt Suggestion、Speculation、
MagicDocs、Tips、Policy Limits），通过后采样钩子在合适时机触发。

### 核心实现方式

#### 7.1 Prompt Suggestion（下一步预测）

```
决策树：
  tryGenerateSuggestion()
    ├─ 抑制条件：早期对话、错误、限速、待权限确认
    ├─ 抑制条件：引导激活、计划模式、cache 未预热
    ├─ generateSuggestion() — Fork 子 Agent
    ├─ shouldFilterSuggestion() — 16 个拒绝过滤器
    └─ 返回 { suggestion, promptId } 或 null

核心测试："用户会想'我正要打这个'吗？"
  Bug 修复后 → "run the tests"
  代码写好后 → "try it out"
  任务完成后 → "commit this"

格式：2-12 个词，匹配用户风格

16 个拒绝过滤器：
  done, meta_text, meta_wrapped, error_message,
  prefixed_label, too_few_words, too_many_words, too_long,
  multiple_sentences, has_formatting, evaluative, claude_voice
```

#### 7.2 Speculation（投机执行）

```
目的：预执行建议的操作，展示"会发生什么"

约束：
  - 最多 20 轮，100 条消息
  - Overlay 文件系统隔离
  - 相同权限检查
  - 随时可中止
  - 用户接受后复制回主文件系统

完成边界类型：
  complete      — 正常完成
  bash          — 到达 bash 命令
  edit          — 到达文件编辑
  denied_tool   — 工具被拒绝
```

#### 7.3 Magic Docs（智能文档更新）

```
触发：读取带有 "# MAGIC DOC:" 头部的文件
  ↓
注册到追踪列表
  ↓
对话进行中...
  ↓
后采样钩子（空闲时、无工具调用时）：
  对每个追踪的 Magic Doc：
    Fork 子 Agent
    canUseTool: 仅允许 FILE_EDIT 该路径
    提示："根据对话更新文档"
    piggyback 父 cache
```

#### 7.4 Tips 系统（上下文感知提示）

```
选择算法：
  getTipToShowOnSpinner(context)
    ├─ 过滤：按上下文相关性
    ├─ 排序：按距上次展示的会话数（最久优先）
    ├─ 返回最久未展示的提示
    └─ 记录展示历史

冷却期：每个提示有最小间隔会话数
```

#### 7.5 Policy Limits（策略限制）

```
响应 Schema：
  PolicyLimitsResponse = {
    restrictions: {
      [policyKey]: { allowed: boolean }
    }
  }

模式：
  - API 获取 + ETag 缓存（304 = cache 有效）
  - 只返回被阻止的策略（缺失 key = 允许）
  - 按用户、按会话执行
```

#### 7.6 后采样钩子（决策驱动器）

```
模型生成响应后、工具执行前触发：
  registerPostSamplingHook(async (context) => {
    // 读取/修改：messages, toolUseContext, appState
  })

并行执行：
  1. Magic Docs 更新
  2. Prompt Suggestion 生成
  3. Speculation 执行
  4. Agent Summary 更新
```

#### 7.7 状态驱动决策

```
AppState 中的决策关键字段：
  toolPermissionContext  — 权限和行为路由
  promptSuggestion       — 下一步预测
  speculation            — 投机执行状态
  companionReaction      — Buddy 反应
  kairosEnabled          — KAIROS 模式开关
  mainLoopModel          — AI 行为控制
```

### 可迁移到当前系统的设计

1. **后采样钩子系统**：当前 agent_loop 无钩子，需增加响应后回调
2. **Prompt Suggestion**：基于上下文预测下一步操作
3. **16 个拒绝过滤器**：防止低质量建议
4. **Speculation 沙箱**：overlay 文件系统预执行
5. **上下文感知 Tips**：基于冷却期的提示系统
6. **策略限制 API**：可扩展的权限控制
7. **AppState 作为决策中心**：统一状态驱动所有子系统决策

---

## 总结：系统设计全景

```
                    ┌─────────────────────────────┐
                    │         KAIROS 决策引擎       │
                    │  Suggestion / Speculation    │
                    │  MagicDocs / Tips / Policy   │
                    └───────────┬─────────────────┘
                                │
                    ┌───────────▼─────────────────┐
                    │       主 Agent 循环            │
                    │   agent_loop + 后采样钩子      │
                    └───┬───────┬──────┬──────────┘
                        │       │      │
              ┌─────────▼─┐  ┌─▼────┐ ┌▼─────────┐
              │ Tool 系统   │  │Context│ │  Memory  │
              │ 三层校验    │  │ 管理  │ │  系统    │
              │ 并发分区    │  │       │ │          │
              │ 语义错误    │  │ micro │ │ memdir   │
              └─────────┬─┘  │ auto  │ │ session  │
                        │    │ react │ │ extract  │
              ┌─────────▼─┐  └───────┘ │ dream    │
              │ 子 Agent   │            └──────────┘
              │ Fork/Team  │
              │ Mailbox    │  ┌────────────────────┐
              │ Coordinator│  │   Buddy System     │
              └────────────┘  │ 异步观察者 + 反应   │
                              │ 确定性身份          │
                              └────────────────────┘
```

### 优先级建议

| 优先级 | 模块 | 理由 |
|--------|------|------|
| P0 | Tool 三层校验 | 安全基础，所有功能依赖 |
| P0 | 微压缩 | 防止长会话崩溃 |
| P1 | Token 预算状态机 | 控制成本和质量 |
| P1 | Fork 子 Agent | 多 Agent 协作的基础 |
| P1 | 记忆类型系统 | 跨会话记忆的结构化基础 |
| P2 | 后采样钩子 | KAIROS 和验证的基础设施 |
| P2 | AutoDream | 记忆自动整理 |
| P2 | 团队邮箱通信 | 复杂协作场景 |
| P3 | Buddy System | 增强用户体验和验证 |
| P3 | Prompt Suggestion | 智能辅助决策 |
| P3 | Speculation | 高级预计算功能 |
