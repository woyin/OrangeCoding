# Agent 系统架构

## 类型系统

### AgentKind 枚举

```rust
pub enum AgentKind {
    Sisyphus,      // 主编排器
    Hephaestus,    // 深度工作者
    Prometheus,    // 战略规划器
    Atlas,         // 任务执行器
    Oracle,        // 架构顾问
    Librarian,     // 文档搜索
    Explore,       // 代码搜索
    Metis,         // 计划顾问
    Momus,         // 计划审核
    Junior,        // 任务执行
    Multimodal,    // 视觉分析
}
```

每个 Agent 通过 `AgentDefinition` trait 定义行为：

- `kind()` — 返回 AgentKind
- `default_model()` — 默认 AI 模型
- `fallback_models()` — 备用模型列表
- `blocked_tools()` — 禁止使用的工具
- `allowed_tools_only()` — 白名单模式（仅 Multimodal 使用）
- `system_prompt()` — 系统提示词
- `is_read_only()` — 是否只读
- `can_delegate()` — 是否可以委托子任务
- `can_use_tool(name)` — 判断是否可以使用指定工具

### AgentRegistry

维护所有 Agent 定义的注册表：

- `register(agent)` — 注册 Agent
- `get(kind)` — 获取 Agent 定义
- `list()` — 列出所有 Agent
- `tab_order()` — 按 Tab 顺序排列

## Category 路由系统

### 8 种内置类别

| 类别 | 模型 | 变体 | 温度 | 用途 |
|------|------|------|------|------|
| visual-engineering | gemini | default | 0.7 | 视觉工程 |
| ultrabrain | gpt-5.4 | xhigh | 0.8 | 超级大脑 |
| deep | gpt-5.4 | medium | 0.6 | 深度思考 |
| artistry | gemini | high | 0.9 | 创意工作 |
| quick | gpt-5.4-mini | default | 0.5 | 快速响应 |
| unspecified-low | sonnet | default | 0.7 | 默认低级 |
| unspecified-high | opus | max | 0.8 | 默认高级 |
| writing | gemini-flash | default | 0.7 | 写作 |

### 配置合并

`CategoryConfig` 支持 `merge_with` 方法，实现用户配置与内置默认的分层合并：

```
内置默认 ← 全局配置 ← 项目配置
```

## Intent Gate 意图分类

### 工作流程

```
用户输入 → 关键词扫描 → 权重计算 → 类别确定 → Agent分配
```

### 关键词规则

- **实现类**: implement, create, build, 实现, 创建, 构建
- **修复类**: fix, bug, debug, 修复, 修正
- **重构类**: refactor, restructure, 重构, 重写
- **搜索类**: search, find, locate, 搜索, 查找
- **分析类**: analyze, investigate, review, 分析, 调查

### 特殊触发

- `ultrawork` / `ulw` → 直接启动 UltraWork 模式（置信度 1.0）
- `search` / `find` → 搜索类 Agent（置信度 0.8）
- `analyze` / `investigate` → 分析类 Agent（置信度 0.7）

### 回退策略

未匹配的输入默认分配到 Implementation 类别，置信度 0.3。

## 模型 Fallback 链

### 故障转移机制

```
主模型 → 备用模型1 → 备用模型2 → ... → 最终回退
```

### 冷却期管理

- `CooldownManager` 跟踪失败模型的冷却时间
- 冷却期内的模型自动跳过
- 冷却结束后模型重新可用

### FallbackResolver

```rust
pub struct FallbackResolver {
    chains: HashMap<String, FallbackChain>,
    cooldowns: CooldownManager,
}
```

- `resolve(chain_name)` → 返回当前可用的最优模型
- `report_failure(model)` → 标记模型失败，启动冷却
- `report_success(model)` → 清除模型的冷却状态

## Agent 间通信

### AgentCommBus

基于 tokio broadcast channel 的消息总线：

```
Agent A ──[StatusUpdate]──→ AgentCommBus ──→ Agent B
Agent C ──[HelpRequest]──→ AgentCommBus ──→ 所有订阅者
```

消息类型：
- `StatusUpdate` — 进度更新
- `WisdomShare` — 知识共享
- `HelpRequest` — 求助请求
- `TaskResult` — 任务结果

### NegotiationProtocol

任务协商流程：

```
发起者 → TaskRequest → NegotiationProtocol
                          ↓
                    评估所有 Agent 能力
                          ↓
                    选择最佳 Agent
                          ↓
                 NegotiationOutcome (Accepted/Rejected)
```

### HandoffManager

任务重分配：

- **过载** — Agent 负载过高时转移任务
- **能力不匹配** — 任务不适合当前 Agent
- **超时** — 任务执行超时
- **自愿** — Agent 主动放弃任务
- **错误** — 执行出错时转移
