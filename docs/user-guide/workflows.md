# 工作流文档

> OrangeCoding 内置多种工作流模式，支持从手动交互到全自动开发的灵活切换。

## 目录

- [工作流概述](#工作流概述)
- [UltraWork (ULW) 模式](#ultrawork-ulw-模式)
- [Prometheus 规划工作流](#prometheus-规划工作流)
- [Atlas 执行编排](#atlas-执行编排)
- [Boulder 系统](#boulder-系统)
- [Ralph 持续改进循环](#ralph-持续改进循环)
- [任务协商和重分配](#任务协商和重分配)
- [工作流组合模式](#工作流组合模式)

---

## 工作流概述

OrangeCoding 的工作流系统分为三个层次：

```
┌─────────────────────────────────────────────────┐
│              UltraWork 全自动模式                  │
│  ┌───────────────────────────────────────────┐  │
│  │          Prometheus + Atlas 协作            │  │
│  │  ┌─────────────┐   ┌─────────────────┐   │  │
│  │  │ Prometheus  │ → │     Atlas       │   │  │
│  │  │ 规划工作流   │   │   执行编排      │   │  │
│  │  └─────────────┘   └─────────────────┘   │  │
│  └───────────────────────────────────────────┘  │
│              Boulder 会话连续性                    │
└─────────────────────────────────────────────────┘
```

| 工作流 | 适用场景 | 自动化程度 |
|--------|---------|-----------|
| **UltraWork** | 完整的功能开发 | ⭐⭐⭐⭐⭐ 全自动 |
| **Prometheus + Atlas** | 需要规划后执行的任务 | ⭐⭐⭐⭐ 半自动 |
| **Ralph 循环** | 持续迭代改进 | ⭐⭐⭐⭐ 半自动 |
| **Boulder** | 会话状态持久化 | ⭐⭐ 辅助功能 |
| **手动交互** | 简单任务、探索性工作 | ⭐ 手动 |

---

## UltraWork (ULW) 模式

UltraWork 是 OrangeCoding 的全自动开发模式，能够自主完成从需求分析到代码验证的完整开发流程。

### 触发方式

**方式一：斜杠命令**

```
/ulw-loop
/ulw-loop custom-config
```

**方式二：关键词触发**

在对话中包含 `ultrawork` 或 `ulw` 关键词：

```
> ulw 实现用户注册和登录功能
> ultrawork 重构数据库连接池
```

关键词触发采用**大小写不敏感**的前缀匹配。Intent Gate 系统会自动识别并启动 UltraWork 流程，分类为 `IntentKind::Implementation`，置信度为 1.0，使用 `unspecified-high` 模型类别。

### 六阶段工作流

```
┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────┐
│ Scanning │ → │Exploring │ → │ Planning │ → │Executing │ → │Verifying │ → │ Done │
│   扫描    │   │   探索    │   │   规划   │   │   执行    │   │   验证    │   │ 完成  │
└──────────┘   └──────────┘   └──────────┘   └──────────┘   └──────────┘   └──────┘
```

#### 阶段 1：Scanning（扫描）

- 扫描项目文件结构
- 识别项目类型和技术栈
- 发现关键配置文件
- 建立文件索引

#### 阶段 2：Exploring（探索）

- 深入阅读关键源代码
- 分析现有架构和设计模式
- 理解模块依赖关系
- 收集上下文信息

#### 阶段 3：Planning（规划）

- 基于需求和代码分析制定实现计划
- 确定修改范围和影响面
- 规划实现步骤和依赖顺序
- 预估风险点

#### 阶段 4：Executing（执行）

- 按计划顺序执行代码修改
- 支持多 Agent 并行执行（最多 3 个）
- 自动纠错（当 `enable_self_correction` 启用时）
- 实时记录执行进度

#### 阶段 5：Verifying（验证）

- 运行项目测试套件
- 验证修改的正确性
- 检查是否引入回归
- 如验证失败，回到执行阶段修复

#### 阶段 6：Done（完成）

- 生成执行总结报告
- 输出修改的文件列表
- 记录经验到 Wisdom 系统

### UltraWork 配置

```jsonc
{
  "ultrawork": {
    "max_parallel_agents": 3,          // 最大并行 Agent 数
    "enable_self_correction": true,     // 启用自动纠错
    "enable_deep_research": false       // 启用深度研究（更慢但更彻底）
  }
}
```

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `max_parallel_agents` | `3` | 最多同时运行多少个 Agent |
| `enable_self_correction` | `true` | 执行出错时自动修复 |
| `enable_deep_research` | `false` | 是否进行更深入的代码分析 |

### UltraWork 使用示例

```
> ulw 为项目添加 REST API 用户认证模块

[Scanning] 正在扫描项目结构...
  ✓ 识别到 Rust 项目（Cargo workspace）
  ✓ 发现 11 个 crate
  ✓ 发现 120+ 个源文件

[Exploring] 正在分析现有代码...
  ✓ 分析了 auth 相关模块
  ✓ 理解了 API 路由结构
  ✓ 识别了数据库连接方式

[Planning] 正在制定实现计划...
  ✓ 计划包含 8 个任务
  ✓ 依赖关系已解析

[Executing] 正在执行计划...
  ✓ 任务 1/8: 创建 User 模型
  ✓ 任务 2/8: 实现密码哈希
  ...

[Verifying] 正在验证...
  ✓ 全部 47 个测试通过
  ✓ 无编译警告

[Done] 完成！
  修改了 12 个文件，新增 3 个文件
```

---

## Prometheus 规划工作流

Prometheus 是 OrangeCoding 的战略规划器，通过结构化的状态机流程生成高质量的开发计划。

### 规划状态机

```
┌───────────┐   ┌────────────────┐   ┌────────────────┐
│ Interview │ → │ ClearanceCheck │ → │ PlanGeneration │
│   访谈     │   │   许可检查      │   │   计划生成      │
└───────────┘   └────────────────┘   └────────────────┘
                                             │
                                             ▼
    ┌──────┐   ┌─────────────┐   ┌──────────────┐
    │ Done │ ← │ MomusReview │ ← │ MetisConsult │
    │ 完成  │   │  Momus 审核  │   │  Metis 咨询  │
    └──────┘   └─────────────┘   └──────────────┘
```

### 阶段详解

#### 1. Interview（访谈阶段）

Prometheus 通过结构化对话收集需求信息。需要确认 **5 项内容** 才能进入下一阶段：

| 确认项 | 说明 | 示例 |
|--------|------|------|
| **Objective（目标）** | 明确要实现什么 | "实现 JWT 用户认证" |
| **Scope（范围）** | 涉及哪些模块和文件 | "auth 模块、API 路由、数据库" |
| **Ambiguities（歧义）** | 澄清不明确的需求 | "Token 过期时间是多少？" |
| **Approach（方案）** | 确认技术方案 | "使用 jsonwebtoken crate" |
| **Testing（测试）** | 测试策略 | "单元测试 + 集成测试" |

**交互示例：**

```
用户: 帮我规划用户认证功能
Prometheus: 让我确认几个关键问题：
  1. ✅ 目标：实现 JWT 用户认证
  2. ✅ 范围：auth 模块 + API 路由
  3. ❓ 歧义：Token 过期时间是多少？需要支持刷新 Token 吗？
  4. ❓ 方案：偏好使用哪个 JWT 库？
  5. ❓ 测试：是否需要集成测试？

用户: Token 1小时过期，需要刷新 Token，使用 jsonwebtoken，需要集成测试
Prometheus: 所有确认项已完成，开始生成计划...
```

#### 2. ClearanceCheck（许可检查）

验证规划范围是否在 Agent 权限内：
- 检查是否涉及受限目录
- 验证工具权限是否满足
- 确认没有超出安全边界

#### 3. PlanGeneration（计划生成）

生成结构化的实现计划：

```
PlanTask {
    id: "auth-model",
    title: "创建用户认证模型",
    description: "在 src/models/ 下创建 User 和 Token 结构体",
    file_references: ["src/models/user.rs", "src/models/token.rs"],
    acceptance_criteria: [
        "User 结构体包含 id, email, password_hash 字段",
        "Token 结构体包含 access_token, refresh_token, expires_at",
        "实现 From<User> for UserResponse 转换",
    ],
    depends_on: [],
}
```

#### 4. MetisConsult（Metis 咨询）

调用 Metis Agent 进行差距分析：
- 检查计划是否遗漏关键步骤
- 验证技术方案的可行性
- 补充安全和性能考虑
- 识别潜在风险

#### 5. MomusReview（Momus 审核）

调用 Momus Agent 进行批评审核：
- 逻辑一致性检查
- 可执行性评估
- 成本和时间估算
- 提出改进建议

**注意：** Metis（claude-opus-4-6）和 Momus（gpt-5.4）使用不同的模型，确保审核视角的多样性。

#### 6. Done（完成）

输出最终计划文档，保存到 `.sisyphus/plans/` 目录。

### 手动触发 Prometheus

```
# 方式一：切换到 Prometheus Agent（Tab 3）
# 方式二：通过 Sisyphus 委托
> 帮我规划一个新的缓存模块

# 方式三：使用 /plan 命令进入计划模式
/plan
```

---

## Atlas 执行编排

Atlas 负责将 Prometheus 生成的计划转化为实际的代码修改。

### 执行流程

```
接收计划 → 依赖分析 → 按顺序执行 → 积累经验 → 验证结果
                         ↓
                   ┌──────────┐
                   │ 任务执行  │
                   │ ┌──────┐ │
                   │ │ 工具  │ │ → Wisdom 积累
                   │ │ 调用  │ │ → Notepad 记录
                   │ └──────┘ │
                   └──────────┘
```

### Wisdom 系统（经验积累）

Atlas 在执行过程中自动积累五类经验，并注入后续任务的上下文：

#### 经验类型

```
┌─────────────────────────────────────────────────────────┐
│                    Wisdom 系统                          │
├──────────────┬──────────────────────────────────────────┤
│ conventions  │ 项目约定：命名规范、代码风格、架构模式        │
│ successes    │ 成功经验：哪些方法有效、最佳实践              │
│ failures     │ 失败教训：哪些路径走不通、需要避免的坑         │
│ gotchas      │ 意外发现：隐含的依赖、不明显的约束条件        │
│ commands     │ 有效命令：构建、测试、部署的有效命令组合       │
└──────────────┴──────────────────────────────────────────┘
```

#### Wisdom 注入机制

```
任务 1 执行 → 积累 Wisdom
                    ↓
任务 2 执行 ← 注入 Wisdom（任务 1 的经验）
                    ↓
任务 3 执行 ← 注入 Wisdom（任务 1 + 2 的经验）
```

随着任务的执行，Atlas 变得越来越"聪明"。它会学习项目的编码规范、记住哪些命令能用、避免重复之前犯过的错误。

### Notepad 系统（执行笔记）

Atlas 在执行过程中维护一个结构化笔记本：

| 字段 | 说明 | 示例 |
|------|------|------|
| `learnings` | 学到的知识 | "项目使用 snake_case 命名" |
| `decisions` | 做出的决策 | "选择 serde_json 而非 simd-json" |
| `issues` | 发现的问题 | "数据库连接池大小需要调整" |
| `verification` | 验证记录 | "cargo test 通过 (1254 tests)" |
| `problems` | 未解决问题 | "性能测试需要在 CI 中补充" |

### Atlas 使用示例

通常 Atlas 不直接使用，而是由 Sisyphus 或 UltraWork 工作流自动调用：

```
# 方式一：Prometheus 规划后自动交给 Atlas 执行
用户: 帮我实现缓存模块
Prometheus: [生成计划，包含 5 个任务]
Atlas: [按依赖顺序执行]
  任务 1: 创建 Cache trait → ✅
  Wisdom: 学到了项目使用 async trait
  任务 2: 实现 RedisCache → ✅
  Wisdom: 发现需要 redis crate 0.24+
  任务 3: 添加缓存中间件 → ✅
  任务 4: 编写测试 → ✅
  任务 5: 更新文档 → ✅

# 方式二：切换到 Atlas Agent（Tab 4）
# 然后给 Atlas 一个已有的计划
```

### Atlas 与其他 Agent 的区别

| 维度 | Atlas | Sisyphus | Junior |
|------|-------|----------|--------|
| 可委托 | ❌ 不可以 | ✅ 可以 | ❌ 不可以 |
| Wisdom | ✅ 有 | ❌ 没有 | ❌ 没有 |
| Notepad | ✅ 有 | ❌ 没有 | ❌ 没有 |
| 适合 | 有计划的多步骤任务 | 灵活的综合任务 | 简单独立任务 |

---

## Boulder 系统

Boulder 是 OrangeCoding 的会话连续性系统，以西西弗斯推动的巨石命名。它记录工作状态，支持跨会话的任务延续。

### 状态文件

Boulder 状态保存在 `.sisyphus/boulder.json`：

```json
{
  "active_plan": "implement-user-auth",
  "plan_name": "用户认证模块实现",
  "session_ids": [
    "session-a1b2c3d4",
    "session-e5f6g7h8"
  ],
  "started_at": "2024-01-15T10:30:00Z",
  "progress": {
    "checked_count": 3,
    "total_count": 8
  }
}
```

### 状态字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `active_plan` | `String` | 当前活跃计划的 ID |
| `plan_name` | `String` | 计划的可读名称 |
| `session_ids` | `Vec<String>` | 关联的会话 ID 列表 |
| `started_at` | `DateTime` | 工作开始时间 |
| `progress` | `Progress` | 执行进度 |

### 进度追踪

```rust
Progress {
    checked_count: u32,    // 已完成的任务数
    total_count: u32,      // 总任务数
}

// 方法
progress.percentage()  → f64    // 完成百分比
progress.is_complete() → bool   // 是否全部完成
```

### Boulder 工作流

#### 初始化 Boulder

```
/init-deep          # 扫描项目并初始化 boulder.json
/start-work 任务名   # 创建新工作会话
```

#### 跨会话延续

```
# 第一个会话
> /start-work 实现用户认证
> 完成了 JWT token 生成...
# 退出

# 第二个会话
OrangeCoding                  # 重新启动
> 继续上次的工作        # Boulder 自动恢复上下文
```

#### 检查进度

```
/session               # 查看当前会话和 Boulder 状态
```

### Boulder 与 Session 的关系

```
┌─────────────────────────────────────┐
│           Boulder 状态               │
│  ┌─────────────────────────────┐   │
│  │ Plan: "用户认证"             │   │
│  │ Progress: 3/8 (37.5%)       │   │
│  │                              │   │
│  │ Session 1 ──────────────┐   │   │
│  │ Session 2 ──────────┐   │   │   │
│  │ Session 3 (当前) ┐  │   │   │   │
│  │                   │  │   │   │   │
│  └───────────────────┘──┘───┘   │   │
└─────────────────────────────────────┘
```

一个 Boulder 可以跨越多个会话，每个会话都是对同一个计划的延续。

---

## Ralph 持续改进循环

Ralph 循环是一个迭代式的持续改进工作流，自动执行 plan → implement → review → refine 的循环。

### 触发方式

```
/ralph-loop              # 全局改进
/ralph-loop API layer    # 聚焦 API 层
```

### 循环流程

```
┌─────────────────────────────────────────────────────┐
│                 Ralph 改进循环                        │
│                                                      │
│  ┌──────────┐                      ┌──────────┐    │
│  │   Plan   │  ←─── 反馈 ──────── │  Refine  │    │
│  │   规划    │                      │   精炼    │    │
│  └────┬─────┘                      └────▲─────┘    │
│       │                                  │          │
│       ▼                                  │          │
│  ┌──────────┐                      ┌──────────┐    │
│  │Implement │  ────────────────── →│  Review  │    │
│  │   实现    │                      │   审查    │    │
│  └──────────┘                      └──────────┘    │
│                                                      │
└─────────────────────────────────────────────────────┘
```

#### Plan（规划）

- 分析当前代码状态
- 识别改进机会
- 制定改进计划

#### Implement（实现）

- 执行改进修改
- 更新相关测试
- 确保编译通过

#### Review（审查）

- 检查修改质量
- 验证测试通过
- 评估改进效果

#### Refine（精炼）

- 根据审查结果调整
- 识别下一轮改进目标
- 决定是否继续循环

### 停止循环

```
/stop-continuation       # 手动停止循环
```

### 适用场景

- 代码质量持续提升
- 技术债务清理
- 性能优化迭代
- 测试覆盖率提升

---

## 任务协商和重分配

OrangeCoding 通过 `chengcoding-mesh` 模块支持多 Agent 间的任务协商和动态重分配。

### 通信机制

```
┌──────────┐     消息总线      ┌──────────┐
│ Agent A  │ ←──────────────→ │ Agent B  │
│          │   chengcoding-mesh     │          │
└──────────┘                  └──────────┘
      ↑                            ↑
      │        ┌──────────┐        │
      └───────→│ 共享状态  │←──────┘
               └──────────┘
```

### 任务协商流程

```
1. Sisyphus 接收用户需求
2. 分析任务特征（意图、难度、领域）
3. 选择最合适的 Agent
4. 发送任务请求
5. 目标 Agent 确认或拒绝
6. 执行并返回结果
```

### 任务重分配

当 Agent 在执行过程中发现任务不适合自己时，可以请求重分配：

```
Junior: 这个任务需要深度推理，我的模型能力不够
  → 重分配给 Hephaestus

Oracle: 这个任务需要修改代码，但我是只读的
  → 重分配给 Sisyphus

Atlas: 这个子任务需要独立的深度研究
  → 无法委托（Atlas 不可委托），记录到 problems
```

### 手动任务交接

使用 `/handoff` 命令手动将任务交给指定 Agent：

```
/handoff prometheus      # 交给 Prometheus 进行规划
/handoff hephaestus      # 交给 Hephaestus 进行深度开发
/handoff oracle          # 交给 Oracle 进行架构分析
```

### 共享状态

Agent 间可以共享以下信息：

| 共享内容 | 说明 |
|---------|------|
| 工作上下文 | 当前正在处理的文件和模块 |
| Wisdom | Atlas 积累的经验 |
| Boulder 状态 | 当前计划和进度 |
| 工具调用结果 | 之前的工具调用缓存 |

---

## 工作流组合模式

### 模式一：完整自动化（推荐新手）

```
/ulw-loop
```

UltraWork 自动组合所有工作流：扫描 → 探索 → Prometheus 规划 → Atlas 执行 → 验证。

### 模式二：先规划后执行

```
# 步骤 1：切换到 Prometheus（Tab 3）
> 帮我规划用户认证模块
# Prometheus 生成计划

# 步骤 2：切换到 Atlas（Tab 4）
> 执行上面的计划
# Atlas 按计划执行
```

### 模式三：深度开发

```
# 切换到 Hephaestus（Tab 2）
> 重构整个数据库层，使用连接池
# Hephaestus 深度工作，自主完成
```

### 模式四：先分析后开发

```
# 步骤 1：用 Oracle 分析（Tab 5）
> 分析当前的错误处理架构

# 步骤 2：用 Prometheus 规划（Tab 3）
> 基于 Oracle 的分析，规划错误处理重构

# 步骤 3：用 Atlas 执行（Tab 4）
> 执行重构计划
```

### 模式五：持续改进

```
# 启动 Ralph 循环
/ralph-loop 性能优化

# 循环运行直到满意
/stop-continuation
```

### 模式六：Boulder 跨会话工作

```
# 第 1 天
/start-work 大型功能开发
> 完成数据库模型和 API 路由
# 退出

# 第 2 天
OrangeCoding
> 继续昨天的工作    # Boulder 自动恢复
> 完成前端集成和测试

# 第 3 天
OrangeCoding
> 完成最后的文档和部署配置
```

---

## 工作流速查表

| 场景 | 推荐工作流 | 命令 |
|------|-----------|------|
| 快速修复 Bug | 手动（Sisyphus） | 直接描述问题 |
| 实现新功能 | UltraWork | `/ulw-loop` |
| 架构设计 | Prometheus | Tab 3 + 描述需求 |
| 代码重构 | Hephaestus | Tab 2 + `/refactor` |
| 代码审查 | Oracle | Tab 5 + 描述审查目标 |
| 持续优化 | Ralph 循环 | `/ralph-loop` |
| 跨天工作 | Boulder | `/start-work` |
| 复杂多步骤 | Prometheus + Atlas | Tab 3 → Tab 4 |
