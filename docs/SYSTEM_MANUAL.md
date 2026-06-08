# OrangeCoding 系统说明书

## 目录

1. [项目概述](#1-项目概述)
2. [系统架构](#2-系统架构)
3. [CLI 命令参考](#3-cli-命令参考)
4. [Agent 系统](#4-agent-系统)
5. [意图分类与 Category 路由](#5-意图分类与-category-路由)
6. [工具系统](#6-工具系统)
7. [权限与安全](#7-权限与安全)
8. [Hook 系统](#8-hook-系统)
9. [技能系统](#9-技能系统)
10. [多代理协作（Mesh）](#10-多代理协作mesh)
11. [工作流编排](#11-工作流编排)
12. [AI 提供者](#12-ai-提供者)
13. [会话管理](#13-会话管理)
14. [终端界面（TUI）](#14-终端界面tui)
15. [控制协议与服务器](#15-控制协议与服务器)
16. [Worker 运行时](#16-worker-运行时)
17. [配置系统](#17-配置系统)
18. [快速开始](#18-快速开始)

---

## 1. 项目概述

OrangeCoding 是基于 Rust 构建的终端 AI 编程助手，使用 Clean-room 方式重新实现。系统由 14 个 workspace crate 组成，包含 120+ 源文件，约 55,000 行代码，1,254 个单元测试。采用 Apache-2.0 许可证。

### 1.1 核心特性

- 11 个专业 Agent，支持意图路由和任务委托
- 22+ 内置工具，带权限控制和沙箱保护
- 5 种 AI 提供者（OpenAI、Anthropic、DeepSeek、通义千问、文心一言）
- 多代理协作系统（Mesh）
- 基于 ratatui 的终端界面
- HTTP/WebSocket 控制服务器
- 完整的权限和审计安全体系

### 1.2 技术栈

- 编程语言：Rust
- 异步运行时：tokio
- 终端 UI：ratatui
- 序列化：serde
- HTTP 客户端：reqwest
- WebSocket：tokio-tungstenite

---

## 2. 系统架构

OrangeCoding 采用模块化设计，由 14 个独立的 crate 组成工作空间。

### 2.1 架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                         用户界面层                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   orangecoding-cli │  │  orangecoding-tui  │  │  orangecoding-control-server   │  │
│  │  (命令行)    │  │  (终端界面)  │  │     (HTTP/WebSocket)    │  │
│  └──────┬──────┘  └──────┬──────┘  └────────────┬────────────┘  │
└─────────┼────────────────┼──────────────────────┼───────────────┘
          │                │                      │
          └────────────────┴──────────────────────┘
                           │
┌──────────────────────────┼──────────────────────────────────────┐
│                          ▼                                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              orangecoding-worker (Worker 运行时)                │   │
│  │         Agent 生命周期管理、审批桥接、事件转换            │   │
│  └─────────────────────────┬───────────────────────────────┘   │
│                            │                                    │
│  ┌─────────────────────────┴───────────────────────────────┐   │
│  │              orangecoding-control-protocol (控制协议)           │   │
│  │         浏览器/服务器/Worker 共用消息类型                │   │
│  └─────────────────────────┬───────────────────────────────┘   │
│                            │                                    │
│  ┌─────────────────────────┴───────────────────────────────┐   │
│  │               orangecoding-agent (代理引擎)                     │   │
│  │    11 专业 Agent、Category 路由、Intent Gate、工作流编排   │   │
│  │         Hook/技能系统、工具执行、权限检查                  │   │
│  └─────────────────────────┬───────────────────────────────┘   │
│                            │                                    │
│  ┌─────────────┬───────────┴───────────┬─────────────┐         │
│  │             │                       │             │         │
│  ▼             ▼                       ▼             ▼         │
│ ┌────────┐ ┌──────────┐ ┌───────────┐ ┌──────────┐ ┌────────┐ │
│ │orangecoding-ai│ │orangecoding-mesh│ │orangecoding-tools│ │orangecoding-mcp │ │orangecoding-  │ │
│ │AI 提供 │ │多代理协作│ │  工具集   │ │ MCP 协议 │ │config │ │
│ │  者   │ │          │ │           │ │          │ │ 配置   │ │
│ └────────┘ └──────────┘ └───────────┘ └──────────┘ └────────┘ │
│     │                                              │          │
│     │         ┌──────────────┐                     │          │
│     └────────►│ orangecoding-session │◄────────────────────┘          │
│               │   会话管理     │                               │
│               └──────────────┘                               │
│                      │                                        │
│     ┌────────────────┼────────────────┐                      │
│     ▼                ▼                ▼                      │
│ ┌────────┐     ┌──────────┐     ┌──────────┐                 │
│ │orangecoding-  │     │orangecoding-    │     │orangecoding-    │                 │
│ │ core   │     │  audit   │     │  audit   │                 │
│ │核心类型│     │ 审计安全 │     │ 审计安全 │                 │
│ └────────┘     └──────────┘     └──────────┘                 │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

### 2.2 Crate 说明

| 模块 | 说明 | 核心功能 |
|------|------|---------|
| orangecoding-core | 核心类型 | 消息系统、错误处理、事件总线 |
| orangecoding-session | 会话管理 | JSONL 树形存储、分支、Blob 存储 |
| orangecoding-ai | AI 提供者 | OpenAI/Anthropic/DeepSeek/通义千问/文心一言、模型角色路由、Fallback 链 |
| orangecoding-agent | 代理引擎 | 11 专业 Agent、Category 路由、Intent Gate、工作流编排、Hook/技能系统 |
| orangecoding-tools | 工具集 | 22+ 内置工具、权限系统、安全沙箱 |
| orangecoding-config | 配置管理 | JSONC 解析、多工具配置发现、加密存储、模型配置 |
| orangecoding-audit | 审计安全 | 日志链、数据脱敏、密钥混淆 |
| orangecoding-mesh | 多代理协作 | Agent 通信、任务协商、任务重分配、消息总线、共享状态 |
| orangecoding-mcp | MCP 协议 | JSON-RPC 2.0、stdio/SSE 传输 |
| orangecoding-tui | 终端界面 | Markdown 渲染、主题系统、会话选择器 |
| orangecoding-cli | 命令行 | 斜杠命令、RPC 模式、OAuth 认证、Zellij/Tmux 集成 |
| orangecoding-control-protocol | 控制协议 | 浏览器/服务器/Worker 共用消息类型、会话/审批/事件模型 |
| orangecoding-control-server | 控制服务 | HTTP API、WebSocket、本地令牌鉴权、流式事件分发 |
| orangecoding-worker | Worker 运行时 | Agent 生命周期管理、审批桥接、事件转换 |

---

## 3. CLI 命令参考

### 3.1 主命令

#### OrangeCoding launch
启动 AI 代理，支持交互式或单任务模式。

```bash
OrangeCoding launch [OPTIONS]
```

**选项：**

| 选项 | 说明 | 示例 |
|------|------|------|
| `--model <MODEL>` | 覆盖默认模型 | `--model gpt-5.4` |
| `--provider <PROVIDER>` | 覆盖 AI 提供者 | `--provider anthropic` |
| `--prompt <PROMPT>` | 单次执行模式（非交互） | `--prompt "解释这段代码"` |

**示例：**

```bash
# 交互式启动
OrangeCoding launch

# 使用指定模型
OrangeCoding launch --model claude-opus-4-6

# 单次执行
OrangeCoding launch --prompt "优化这个函数的性能"
```

#### OrangeCoding config
管理 OrangeCoding 配置。

```bash
OrangeCoding config <SUBCOMMAND>
```

**子命令：**

| 子命令 | 说明 | 示例 |
|--------|------|------|
| `show` | 显示当前配置 | `OrangeCoding config show --json` |
| `set <key> <value>` | 设置配置值 | `OrangeCoding config set ai.model gpt-5.4` |
| `get <key>` | 获取配置值 | `OrangeCoding config get ai.provider` |
| `init` | 初始化默认配置 | `OrangeCoding config init` |

**配置键支持 dot-notation：**

```bash
# 设置嵌套配置
OrangeCoding config set ai.provider anthropic
OrangeCoding config set ai.model claude-opus-4-6
OrangeCoding config set tools.allowed_paths '[".", "src"]'
```

#### OrangeCoding init
初始化项目脚手架，创建必要的配置文件。

```bash
OrangeCoding init [PATH]
```

**创建的文件：**

- `.orangecoding/config.toml` - 项目级配置
- `AGENTS.md` - Agent 行为指南
- `.orangecoding/commands/` - 自定义斜杠命令目录

#### OrangeCoding serve
启动本地 Web 控制服务器。

```bash
OrangeCoding serve [OPTIONS]
```

**选项：**

| 选项 | 说明 | 默认值 |
|------|------|--------|
| `--bind <ADDR>` | 绑定地址 | `127.0.0.1:3200` |

**示例：**

```bash
OrangeCoding serve --bind 0.0.0.0:3200
```

#### OrangeCoding status
显示系统状态，包括配置加载情况、Agent 状态、会话数量等。

```bash
OrangeCoding status
```

#### OrangeCoding version
显示版本信息。

```bash
OrangeCoding version
```

### 3.2 内置斜杠命令

在聊天中输入 `/command` 使用以下命令：

| 命令 | 说明 | 参数 |
|------|------|------|
| `/help` | 显示帮助信息 | 无 |
| `/hotkeys` | 显示快捷键 | 无 |
| `/model` | 模型选择器 | 无 |
| `/models` | 显示可用模型 | 无 |
| `/plan` | 切换规划模式 | 无 |
| `/compact [focus]` | 压缩上下文 | 可选焦点描述 |
| `/new` | 新建会话 | 无 |
| `/resume` | 打开会话选择器 | 无 |
| `/export [path]` | 导出会话为 HTML | 可选导出路径 |
| `/session` | 显示会话信息 | 无 |
| `/usage` | 显示使用统计 | 无 |
| `/exit` | 退出 | 无 |
| `/quit` | 退出 | 无 |
| `/settings` | 设置菜单 | 无 |
| `/tree` | 会话树导航 | 无 |
| `/branch` | 分支选择器 | 无 |
| `/fork` | 从消息分叉 | 无 |
| `/copy` | 复制最后一条消息 | 无 |
| `/debug` | 调试信息 | 无 |
| `/init-deep [path]` | 深度初始化项目 | 可选路径 |
| `/ralph-loop [focus]` | Ralph 持续改进循环 | 可选焦点 |
| `/ulw-loop [config]` | UltraWork 全自动模式 | 可选配置 |
| `/refactor [target]` | 重构助手 | 可选目标 |
| `/start-work [task]` | 开始新工作会话 | 可选任务描述 |
| `/stop-continuation` | 停止自动继续 | 无 |
| `/handoff [agent]` | 任务移交给其他 Agent | 可选目标 Agent |

### 3.3 自定义斜杠命令

自定义命令放在以下位置：

- 全局：`~/.orangecoding/commands/`
- 项目：`.orangecoding/commands/`

**文件格式：** Markdown，支持参数替换。

**示例命令文件**（`.orangecoding/commands/commit.md`）：

```markdown
# commit

生成并提交 Git commit。

## 用法

```
/commit [message]
```

## 执行

请帮我：
1. 查看当前 git status
2. 根据变更生成合适的 commit message
3. 执行 git add 和 git commit

如果提供了参数 $1，使用它作为 commit message；否则根据变更自动生成。
```

**参数替换：**

- `$1`, `$2`, ... - 第 N 个参数
- `$@` - 所有参数

### 3.4 RPC 模式

OrangeCoding 支持 JSONL stdio 协议，用于编程式访问。

**消息类型：**

| 类型 | 方向 | 说明 |
|------|------|------|
| `UserMessage` | C->S | 用户消息 |
| `AssistantText` | S->C | 助手文本增量 |
| `ToolCall` | S->C | 工具调用请求 |
| `ToolResult` | C->S | 工具执行结果 |
| `Status` | S->C | 状态更新 |
| `Usage` | S->C | Token 用量 |
| `ModelInfo` | S->C | 模型信息 |
| `SessionInfo` | S->C | 会话信息 |
| `Error` | S->C | 错误信息 |
| `Ping` | C->S | 心跳请求 |
| `Pong` | S->C | 心跳响应 |
| `Exit` | 双向 | 退出信号 |

**示例 RPC 会话：**

```json
{"type": "UserMessage", "content": "你好"}
{"type": "AssistantText", "content": "你好！"}
{"type": "AssistantDone"}
```

### 3.5 终端复用器集成

OrangeCoding 自动检测并集成 Zellij 或 tmux。

**功能：**

- 自动检测可用的终端复用器
- 为 Agent spawning 管理面板创建
- 支持布局管理

**配置：**

```toml
[cli]
multiplexer = "auto"  # 可选: "zellij", "tmux", "none"
```

### 3.6 OAuth 2.1 认证

用于 MCP 服务器认证，支持：

- PKCE (RFC 7636) - S256 方法
- RFC 9728/8414 发现端点
- 动态客户端注册 (RFC 7591)

---

## 4. Agent 系统

OrangeCoding 包含 11 个专业 Agent，每个 Agent 都有特定的角色和能力。

### 4.1 Agent 列表

| Agent | 角色 | 默认模型 | 特性 |
|-------|------|---------|------|
| Sisyphus | 主编排器 | claude-opus-4-6 | 全工具权限，可委托 |
| Hephaestus | 深度工作者 | gpt-5.4 | 全工具权限，可委托 |
| Prometheus | 战略规划器 | claude-opus-4-6 | 规划状态机，可委托 |
| Atlas | 任务执行器 | claude-sonnet-4-6 | 执行编排，不可委托 |
| Oracle | 架构顾问 | claude-opus-4-6 | 只读，不可委托 |
| Librarian | 文档搜索 | minimax-m2.7 | 只读，不可委托 |
| Explore | 代码搜索 | grok-code-fast-1 | 只读，不可委托 |
| Metis | 计划顾问 | claude-opus-4-6 | 差距分析 |
| Momus | 计划审核 | gpt-5.4 | 批评审核 |
| Junior | 任务执行 | 按类别分配 | 不可委托 |
| Multimodal | 视觉分析 | gpt-5.4 | 白名单模式（仅 read 工具） |

### 4.2 AgentDefinition Trait

每个 Agent 实现以下 trait：

```rust
pub trait AgentDefinition {
    /// 默认模型名称
    fn default_model(&self) -> String;
    
    /// Fallback 链（按优先级排序的模型列表）
    fn fallback_chain(&self) -> Vec<String>;
    
    /// 被阻止的工具列表
    fn blocked_tools(&self) -> Vec<String>;
    
    /// 仅允许的工具列表（空表示允许所有）
    fn allowed_tools_only(&self) -> Vec<String>;
    
    /// 系统提示词
    fn system_prompt(&self) -> String;
    
    /// 是否可以委托任务给其他 Agent
    fn can_delegate(&self) -> bool;
    
    /// 是否为只读模式
    fn is_read_only(&self) -> bool;
}
```

### 4.3 Agent 选择

**自动选择：**

系统根据 Intent Gate 的意图分类自动选择合适的 Agent。

**手动指定：**

```bash
# 通过环境变量
OrangeCoding_AGENT=oracle OrangeCoding launch

# 通过配置
OrangeCoding config set agent.default oracle
```

**斜杠命令切换：**

```
/handoff oracle
```

---

## 5. 意图分类与 Category 路由

### 5.1 Intent Gate 意图分类

IntentKind 枚举定义了 7 种意图类型：

| 意图 | 关键词 | 权重 | 映射类别 |
|------|--------|------|----------|
| Research | research, explore, look into, study, what is, how does, explain, describe | 3 | deep |
| Implementation | build, create, implement, add, develop, make, write, code, construct | 3 | unspecified-high |
| Fix | fix, bug, broken, not working, error, crash, issue, problem, wrong | 4 | unspecified-low |
| Investigation | investigate, debug, trace, root cause, why does, how come, analyze | 5 | ultrabrain |
| Refactor | refactor, restructure, reorganize, clean up, simplify, optimize, improve | 4 | deep |
| Planning | plan, design, architect, strategy, propose, blueprint, roadmap | 4 | unspecified-high |
| QuickFix | typo, rename, quick, simple, just change, just update, trivial | 5 | quick |

**特殊触发词：**

| 触发词 | 意图 | 置信度 |
|--------|------|--------|
| "ultrawork", "ulw" | Implementation | 1.0 |
| "search ", "find " | Research | 0.9 |
| "analyze ", "investigate " | Investigation | 0.9 |

### 5.2 Category 路由（8 种）

| 类别 | 模型 | temperature | 用途 |
|------|------|-------------|------|
| visual-engineering | gemini-3.1-pro (high) | 0.3 | 视觉工程 |
| ultrabrain | gpt-5.4 (xhigh) | 0.1 | 超级大脑 |
| deep | gpt-5.4 (medium) | 0.1 | 深度思考 |
| artistry | gemini-3.1-pro (high) | 0.7 | 创意工作 |
| quick | gpt-5.4-mini | 0.1 | 快速响应 |
| unspecified-low | claude-sonnet-4-6 | 0.1 | 默认低级 |
| unspecified-high | claude-opus-4-6 (max) | 0.1 | 默认高级 |
| writing | gemini-3-flash | 0.5 | 写作 |

### 5.3 CategoryConfig 结构

```rust
pub struct CategoryConfig {
    /// 模型名称
    pub model: String,
    
    /// 温度参数
    pub temperature: f32,
    
    /// Top-p 采样
    pub top_p: Option<f32>,
    
    /// 思考配置
    pub thinking: Option<ThinkingConfig>,
    
    /// 推理努力程度
    pub reasoning_effort: Option<ReasoningEffort>,
    
    /// 输出详细程度
    pub verbosity: Option<Verbosity>,
    
    /// 工具控制
    pub tools: Option<ToolsControl>,
    
    /// 最大 token 数
    pub max_tokens: Option<u32>,
    
    /// 是否为不稳定 Agent
    pub is_unstable_agent: bool,
}

pub struct ThinkingConfig {
    pub thinking_type: ThinkingType,
    pub budget_tokens: u32,
}

pub enum ReasoningEffort {
    Low,
    Medium,
    High,
    XHigh,
}

pub enum Verbosity {
    Low,
    Medium,
    High,
}

pub struct ToolsControl {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}
```

### 5.4 用户自定义 Category

在配置文件中覆盖默认 Category：

```toml
[category.custom-deep]
model = "claude-opus-4-6"
temperature = 0.1
reasoning_effort = "high"
max_tokens = 8192

[category.custom-deep.tools]
allow = ["read", "edit", "bash"]
deny = ["write", "delete"]
```

---

## 6. 工具系统

OrangeCoding 提供 22+ 内置工具，分为多个类别。

### 6.1 文件操作工具（受 FileOperationGuard 保护）

| 工具 | 说明 | 参数 |
|------|------|------|
| `read_file` | 读取文件内容 | `file_path`, `offset`, `limit` |
| `write_file` | 写入文件 | `file_path`, `content` |
| `edit_file` | 编辑文件 | `file_path`, `old_string`, `new_string` |
| `edit` | 简写编辑 | `file_path`, `old_string`, `new_string` |
| `list_directory` | 列出目录 | `path` |
| `search_files` | 搜索文件 | `pattern`, `path` |
| `delete_file` | 删除文件 | `file_path` |

### 6.2 搜索与路径工具（受保护）

| 工具 | 说明 | 参数 |
|------|------|------|
| `grep` | 正则搜索 | `pattern`, `path`, `output_mode` |
| `find` | 文件查找 | `pattern`, `path` |
| `ast_grep` | AST 代码搜索 | `pattern`, `lang`, `paths` |

### 6.3 执行类工具

| 工具 | 说明 | 参数 |
|------|------|------|
| `bash` | Shell 命令执行 | `command`, `timeout` |
| `ssh` | 远程命令执行 | `host`, `command` |
| `python` | Python REPL | `code` |
| `notebook` | Jupyter Notebook 操作 | `action`, `path` |

### 6.4 交互类工具

| 工具 | 说明 | 参数 |
|------|------|------|
| `ask` | 结构化用户交互 | `question`, `options` |
| `todo` | 分阶段任务跟踪 | `todos` |
| `task` | 子任务代理委派 | `description`, `prompt` |

### 6.5 信息类工具

| 工具 | 说明 | 参数 |
|------|------|------|
| `fetch` | URL 内容抓取 | `url`, `format` |
| `web_search` | 多引擎搜索 | `query`, `num_results` |
| `calc` | 数学表达式求值 | `expression` |
| `browser` | 网页交互与截图 | `action`, `url` |
| `lsp` | 语言服务器协议集成 | `action`, `file_path` |

### 6.6 会话工具

| 工具 | 说明 | 参数 |
|------|------|------|
| `session_list` | 列出会话 | `limit`, `project_path` |
| `session_read` | 读取会话 | `session_id`, `include_todos` |
| `session_search` | 搜索会话 | `query` |
| `session_info` | 会话信息 | `session_id` |

### 6.7 任务工具

| 工具 | 说明 | 参数 |
|------|------|------|
| `task_create` | 创建任务 | `description` |
| `task_query` | 查询任务 | `task_id` |
| `task_list` | 列出任务 | `status` |
| `task_update` | 更新任务 | `task_id`, `status` |

### 6.8 工具元数据

每个工具声明以下属性：

```rust
pub struct ToolMetadata {
    /// 是否为只读工具
    pub is_read_only: bool,
    
    /// 是否支持并发执行
    pub is_concurrency_safe: bool,
    
    /// 是否为破坏性操作
    pub is_destructive: bool,
    
    /// 是否启用
    pub is_enabled: bool,
}
```

### 6.9 工具执行

**ToolExecutor：**

- 处理 AI 工具调用
- 超时保护（默认 30 秒）
- 权限检查

**批量执行：**

```rust
pub async fn execute_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
    // 使用 batch_partition 分区
    // 并发安全工具并行执行
    // 不安全工具串行执行
}
```

**执行流程：**

1. 解析工具调用
2. 检查权限（check_permissions）
3. 验证参数
4. 执行工具
5. 返回结果

---

## 7. 权限与安全

### 7.1 权限类型（5 种）

| 权限 | 说明 | 受控操作 |
|------|------|----------|
| `Edit` | 文件编辑 | write_file, edit_file, delete_file |
| `Bash` | 命令执行 | bash, ssh |
| `WebFetch` | 网络获取 | fetch, web_search |
| `DoomLoop` | 循环检测 | 自动继续机制 |
| `ExternalDirectory` | 外部目录 | 工作目录外的文件操作 |

### 7.2 权限级别（3 级）

| 级别 | 说明 | 行为 |
|------|------|------|
| `Ask` | 询问用户 | 弹出确认对话框 |
| `Allow` | 直接允许 | 无需确认直接执行 |
| `Deny` | 禁止 | 拒绝执行并返回错误 |

### 7.3 权限配置

```toml
[permissions]
edit = "ask"          # 编辑操作需要确认
bash = "ask"          # 命令执行需要确认
web_fetch = "allow"   # 网络获取直接允许
doom_loop = "deny"    # 禁止自动继续
external_directory = "ask"

[permissions.paths]
allowed = [".", "src", "tests"]  # 白名单
denied = ["node_modules", ".git"]  # 黑名单
```

### 7.4 路径检查逻辑

```
1. 检查白名单 (allowed_paths)
   - 如果匹配 → Allow
   
2. 检查黑名单 (denied_patterns)
   - 如果匹配 → Deny
   
3. 检查工作目录
   - 如果在工作目录内 → Allow
   - 如果在工作目录外 → 检查 ExternalDirectory 权限
```

### 7.5 默认阻止路径（14 个）

系统自动阻止以下敏感路径：

- `~/.ssh`
- `~/.aws`
- `~/.config`
- `/etc`
- `/usr/bin`
- `/bin`
- `/sbin`
- `/usr/sbin`
- `/var`
- `/root`
- `~/.gnupg`
- `~/.docker`
- `~/.kube`
- `~/.npmrc`

### 7.6 安全沙箱

**FileOperationGuard：**

- 包装所有文件操作工具
- 路径规范化（解析符号链接、.. 等）
- 权限验证
- 审计日志

### 7.7 审计系统

**审计日志链：**

- SHA-256 链式哈希
- 每条记录包含前一条的哈希
- 防止篡改

**密钥脱敏：**

- 自动检测 API 密钥模式
- 日志中脱敏显示
- 支持自定义脱敏规则

---

## 8. Hook 系统

Hook 系统提供 26 种内置 Hook，支持在关键事件点插入自定义逻辑。

### 8.1 Hook 事件类型

| 事件 | 触发时机 |
|------|----------|
| `PreSession` | 会话开始前 |
| `PostSession` | 会话结束后 |
| `PreMessage` | 消息处理前 |
| `PostMessage` | 消息处理后 |
| `PreToolCall` | 工具调用前 |
| `PostToolCall` | 工具调用后 |
| `PreCompaction` | 上下文压缩前 |
| `PostCompaction` | 上下文压缩后 |

### 8.2 Hook 动作

| 动作 | 说明 |
|------|------|
| `Continue` | 继续正常流程 |
| `Modify(String)` | 修改内容后继续 |
| `Block(String)` | 阻止操作并返回错误 |
| `Skip` | 跳过当前操作 |

### 8.3 Hook 优先级

| 优先级 | 值 | 说明 |
|--------|-----|------|
| `Critical` | 0 | 最先执行 |
| `High` | 1 | 高优先级 |
| `Normal` | 2 | 默认优先级 |
| `Low` | 3 | 低优先级 |

### 8.4 内置 Hook 列表

| Hook | 事件 | 功能 |
|------|------|------|
| `KeywordDetector` | PreMessage | 检测关键词触发特殊模式 |
| `ThinkMode` | PreMessage | 思考模式切换 |
| `CommentChecker` | PostToolCall | 检查代码注释 |
| `EditErrorRecovery` | PostToolCall | 编辑错误恢复 |
| `WriteExistingFileGuard` | PreToolCall | 写入现有文件保护 |
| `SessionRecovery` | PreSession | 会话恢复 |
| `TodoContinuationEnforcer` | PreMessage | Todo 继续强制执行 |
| `CompactionTodoPreserver` | PreCompaction | 压缩时保留 Todo |
| `BackgroundNotification` | PostToolCall | 后台任务通知 |
| `ToolOutputTruncator` | PostToolCall | 工具输出截断 |
| `RalphLoop` | PreMessage | Ralph 持续改进循环 |
| `StartWork` | PreMessage | 开始工作流 |
| `StopContinuationGuard` | PreMessage | 停止继续保护 |
| `PrometheusMdOnly` | PreMessage | Prometheus 仅 Markdown |
| `HashlineReadEnhancer` | PreToolCall | Hashline 读取增强 |
| `HashlineEditDiffEnhancer` | PostToolCall | Hashline 编辑差异增强 |
| `DirectoryAgentsInjector` | PreSession | 目录 Agent 注入 |
| `RulesInjector` | PreMessage | 规则注入 |
| `CompactionContextInjector` | PreCompaction | 压缩上下文注入 |
| `AutoUpdateChecker` | PostSession | 自动更新检查 |
| `RuntimeFallback` | PostToolCall | 运行时 Fallback |
| `ModelFallback` | PostMessage | 模型 Fallback |
| `AnthropicEffort` | PreMessage | Anthropic 努力度设置 |
| `AgentUsageReminder` | PreMessage | Agent 使用提醒 |
| `DelegateTaskRetry` | PostToolCall | 委托任务重试 |
| `UnstableAgentBabysitter` | PostMessage | 不稳定 Agent 看护 |

### 8.5 自定义 Hook

```rust
pub trait Hook {
    /// Hook 名称
    fn name(&self) -> &str;
    
    /// 优先级
    fn priority(&self) -> HookPriority;
    
    /// 感兴趣的事件
    fn interested_events(&self) -> Vec<HookEvent>;
    
    /// 处理事件
    fn handle(&self, event: HookEvent, context: &Context) -> HookAction;
}
```

---

## 9. 技能系统

技能系统允许打包和复用 Agent 能力。

### 9.1 内置技能（6 种）

| 技能 | 说明 |
|------|------|
| `git-master` | Git 操作专家 |
| `playwright` | Playwright 浏览器自动化 |
| `playwright-cli` | Playwright CLI 工具 |
| `agent-browser` | Agent 浏览器工具 |
| `dev-browser` | 开发浏览器工具 |
| `frontend-ui-ux` | 前端 UI/UX 设计 |

### 9.2 SkillPack 结构

```rust
pub struct SkillPack {
    /// 技能名称
    pub name: String,
    
    /// 描述
    pub description: String,
    
    /// 版本
    pub version: String,
    
    /// 规则列表
    pub rules: Vec<String>,
    
    /// 上下文文件
    pub context_files: Vec<PathBuf>,
    
    /// 工具列表
    pub tools: Vec<String>,
    
    /// 来源
    pub source: SkillSource,
    
    /// 是否启用
    pub enabled: bool,
}

pub enum SkillSource {
    Builtin,      // 内置
    UserGlobal,   // 用户全局
    Project,      // 项目级
}
```

### 9.3 SkillRegistry API

```rust
impl SkillRegistry {
    /// 注册技能
    pub fn register(&mut self, skill: SkillPack);
    
    /// 获取技能
    pub fn get(&self, name: &str) -> Option<&SkillPack>;
    
    /// 列出启用的技能
    pub fn list_enabled(&self) -> Vec<&SkillPack>;
    
    /// 启用技能
    pub fn enable(&mut self, name: &str);
    
    /// 禁用技能
    pub fn disable(&mut self, name: &str);
    
    /// 从目录发现技能
    pub fn discover_from_dir(&mut self, path: &Path);
    
    /// 收集所有规则
    pub fn collect_rules(&self) -> Vec<String>;
    
    /// 技能数量
    pub fn count(&self) -> usize;
}
```

### 9.4 技能配置

```toml
[skills]
enabled = ["git-master", "playwright"]

[[skills.pack]]
name = "my-custom-skill"
description = "我的自定义技能"
version = "1.0.0"
rules = [
    "始终使用 TypeScript",
    "遵循项目的 ESLint 配置"
]
tools = ["read", "edit", "bash"]
```

---

## 10. 多代理协作（Mesh）

Mesh 系统支持多个 Agent 之间的协作。

### 10.1 核心组件

| 组件 | 说明 |
|------|------|
| `AgentCommBus` | 点对点消息传递 + 广播模式 |
| `NegotiationProtocol` | 任务协商协议 |
| `HandoffManager` | 任务重分配管理 |
| `MessageBus` | tokio::broadcast 发布/订阅 |
| `SharedState` | 线程安全 KV 存储，支持 TTL |
| `AgentRegistry` | Agent 生命周期和状态追踪 |
| `ModelRouter` | 基于任务类型动态选择模型 |
| `RoleSystem` | Agent 角色和权限定义 |
| `TaskOrchestrator` | DAG 工作流管理 |

### 10.2 AgentCommBus

```rust
pub struct AgentCommBus {
    /// 发送消息到指定 Agent
    pub async fn send_to(&self, target: AgentId, message: Message);
    
    /// 广播消息给所有 Agent
    pub async fn broadcast(&self, message: Message);
    
    /// 订阅消息
    pub fn subscribe(&self) -> broadcast::Receiver<Message>;
}
```

### 10.3 任务协商协议

```
协商流程：

1. 请求方发送 TaskRequest
   ├─ 任务描述
   ├─ 所需能力
   └─ 优先级

2. 候选 Agent 发送 Proposal
   ├─ 能力匹配度
   ├─ 当前负载
   └─ 预计完成时间

3. 请求方选择最佳 Proposal

4. 双方确认：Accept 或 Reject
```

### 10.4 任务重分配

触发条件：

- Agent 过载（负载超过阈值）
- 能力不匹配（无法完成任务）
- 超时（未在期限内完成）

```rust
pub struct HandoffManager {
    /// 请求任务重分配
    pub async fn request_handoff(&self, task_id: TaskId, reason: HandoffReason);
    
    /// 查找替代 Agent
    pub async fn find_replacement(&self, requirements: &TaskRequirements) -> Vec<AgentId>;
}

pub enum HandoffReason {
    Overloaded,
    CapabilityMismatch,
    Timeout,
}
```

### 10.5 共享状态

```rust
pub struct SharedState {
    /// 设置值（带 TTL）
    pub async fn set(&self, key: &str, value: Value, ttl: Option<Duration>);
    
    /// 获取值
    pub async fn get(&self, key: &str) -> Option<Value>;
    
    /// 删除值
    pub async fn delete(&self, key: &str);
    
    /// 原子更新
    pub async fn update<F>(&self, key: &str, f: F) -> Result<Value>
    where F: FnOnce(Option<Value>) -> Option<Value>;
}
```

---

## 11. 工作流编排

### 11.1 UltraWork (ulw)

输入 "ultrawork" 或 "ulw" 触发全自动模式。

**特性：**

- 自动规划
- 深度研究
- 并行 Agent 执行
- 自我修正

**使用：**

```
/ulw-loop 优化这个项目的性能
```

### 11.2 Prometheus 规划

状态机驱动的战略规划工作流。

**状态：**

1. `GatherContext` - 收集上下文
2. `AnalyzeRequirements` - 分析需求
3. `IdentifyGaps` - 识别差距
4. `ProposeSolutions` - 提出方案
5. `EvaluateTradeoffs` - 评估权衡
6. `FinalizePlan` - 确定计划

### 11.3 Atlas 执行

任务编排和智慧系统。

**功能：**

- 任务分解
- 依赖管理
- 进度追踪
- 错误恢复

### 11.4 Boulder 系统

会话连续性系统，支持崩溃恢复。

**特性：**

- 定期状态快照
- 崩溃后自动恢复
- 断点续传

### 11.5 自动上下文压缩

当上下文接近限制时自动触发。

**策略：**

- 保留关键消息
- 摘要历史对话
- 维护 Todo 列表

### 11.6 AutoDream

空闲时无意识处理。

**功能：**

- 后台索引代码
- 预生成摘要
- 优化会话存储

### 11.7 TTSR 引擎

基于正则触发的零成本流式规则注入。

**原理：**

- 正则匹配消息内容
- 动态注入上下文规则
- 无需重新加载配置

### 11.8 Hashline 编辑

SHA-256 内容哈希锚点精确定位。

**使用：**

```
在文件中找到以下内容（哈希：a1b2c3d4...）：
[代码块]
```

**优势：**

- 精确定位，不受行号变化影响
- 支持大文件编辑
- 避免模糊匹配错误

---

## 12. AI 提供者

### 12.1 提供者列表

| 提供者 | API 格式 | 特点 |
|--------|----------|------|
| OpenAI 兼容 | Chat Completions | 支持 GPT-5.4/GPT-4o，可配置 base_url |
| Anthropic | Messages API | Claude Opus/Sonnet，系统消息作为顶层字段 |
| DeepSeek | OpenAI 兼容 | DeepSeek Chat/Coder |
| 通义千问 | DashScope | input.messages + parameters 结构 |
| 文心一言 | OAuth 2.0 | API Key + Secret Key 获取 access_token |

### 12.2 统一接口

所有提供者实现 `AiProvider` trait：

```rust
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// 发送消息并获取响应
    async fn chat(&self, messages: Vec<Message>, options: ChatOptions) -> Result<ChatResponse>;
    
    /// 流式响应
    async fn chat_stream(&self, messages: Vec<Message>, options: ChatOptions) -> Result<Stream>;
    
    /// 获取可用模型
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;
    
    /// 检查连接
    async fn health_check(&self) -> Result<bool>;
}
```

### 12.3 ChatOptions

```rust
pub struct ChatOptions {
    /// 模型名称
    model: Option<String>,
    
    /// 温度参数
    temperature: Option<f32>,
    
    /// 最大 token 数
    max_tokens: Option<u32>,
    
    /// Top-p 采样
    top_p: Option<f32>,
    
    /// 系统消息
    system: Option<String>,
    
    /// 工具定义
    tools: Option<Vec<ToolDefinition>>,
}

impl ChatOptions {
    pub fn with_model(mut self, model: &str) -> Self;
    pub fn temperature(mut self, temp: f32) -> Self;
    pub fn max_tokens(mut self, tokens: u32) -> Self;
}
```

### 12.4 Token 用量

```rust
pub struct TokenUsage {
    /// 提示 token 数
    pub prompt_tokens: u32,
    
    /// 完成 token 数
    pub completion_tokens: u32,
    
    /// 总 token 数
    pub total_tokens: u32,
}
```

### 12.5 模型角色路由

| 角色 | 说明 |
|------|------|
| `Default` | 默认模型 |
| `Smol` | 轻量级模型（快速响应） |
| `Slow` | 慢速模型（高质量） |
| `Plan` | 规划专用模型 |
| `Commit` | 提交信息生成模型 |

### 12.6 思考级别

| 级别 | 说明 |
|------|------|
| `Off` | 关闭思考 |
| `Minimal` | 最小思考 |
| `Low` | 低级别 |
| `Medium` | 中级别 |
| `High` | 高级别 |
| `XHigh` | 最高级别 |

### 12.7 Fallback 系统

```rust
pub struct FallbackConfig {
    /// 启用 Fallback
    pub enabled: bool,
    
    /// Fallback 链
    pub chain: Vec<String>,
    
    /// 冷却期（秒）
    pub cooldown_secs: u64,
}
```

**可重试错误：**

- 429 - 速率限制
- 503 - 服务不可用
- 529 - 服务器过载

**配置错误（不重试）：**

- 401 - 未授权
- 403 - 禁止访问

---

## 13. 会话管理

### 13.1 JSONL 树形存储格式

**文件结构：**

```jsonl
{"type": "header", "id": "sess_abc123", "timestamp": 1234567890, "cwd": "/project", "version": "1.0"}
{"type": "entry", "entry_type": "Message", "data": {...}}
{"type": "entry", "entry_type": "Message", "data": {...}}
{"type": "entry", "entry_type": "Compaction", "data": {...}}
```

**特性：**

- 首行为 SessionHeader
- 后续行为 SessionEntry（追加式，不重写整个文件）
- 损坏韧性：跳过格式错误的行

### 13.2 Entry 类型

| 类型 | 说明 |
|------|------|
| `Message` | 用户/助手消息 |
| `ThinkingLevel` | 思考级别变更 |
| `ModelChange` | 模型切换 |
| `Compaction` | 上下文压缩 |
| `BranchSummary` | 分支摘要 |
| `Label` | 标签 |
| `ModeChange` | 模式变更 |
| `Custom` | 自定义条目 |

### 13.3 EntryData 结构

```rust
pub enum EntryData {
    Message(MessageEntry),
    ThinkingLevel(ThinkingLevelEntry),
    ModelChange(ModelChangeEntry),
    Compaction(CompactionEntry),
    BranchSummary(BranchSummaryEntry),
    Label(LabelEntry),
    ModeChange(ModeChangeEntry),
    Custom(Value),
}

pub struct MessageEntry {
    pub role: Role,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub model: Option<String>,
    pub token_usage: Option<TokenUsage>,
}

pub struct CompactionEntry {
    pub summary: String,
    pub short_summary: String,
    pub first_kept_entry_id: String,
    pub tokens_before: u32,
}
```

### 13.4 SessionManager API

```rust
impl SessionManager {
    /// 创建新会话
    pub async fn create_session(&self, cwd: PathBuf) -> Result<Session>;
    
    /// 恢复最近的会话（按 CWD）
    pub async fn resume_latest(&self, cwd: &Path) -> Result<Option<Session>>;
    
    /// 按 ID 前缀恢复会话
    pub async fn resume_by_id(&self, id_prefix: &str) -> Result<Option<Session>>;
    
    /// 列出会话
    pub async fn list_sessions(&self, filter: SessionFilter) -> Result<Vec<SessionInfo>>;
    
    /// 清理旧会话
    pub async fn cleanup_old_sessions(&self, max_age: Duration) -> Result<usize>;
}
```

### 13.5 Session 操作

```rust
impl Session {
    /// 添加用户消息
    pub async fn add_user_message(&mut self, content: &str) -> Result<EntryId>;
    
    /// 添加助手消息
    pub async fn add_assistant_message(&mut self, content: &str, model: &str) -> Result<EntryId>;
    
    /// 添加工具结果
    pub async fn add_tool_result(&mut self, tool_call_id: &str, result: &str) -> Result<EntryId>;
    
    /// 获取上下文消息
    pub fn context_messages(&self, limit: usize) -> Vec<Message>;
    
    /// 创建分支
    pub async fn branch(&mut self, from_entry_id: &str) -> Result<Session>;
    
    /// 压缩上下文
    pub async fn compact(&mut self, focus: Option<&str>) -> Result<CompactionResult>;
    
    /// 获取会话信息
    pub fn info(&self) -> SessionInfo;
}
```

### 13.6 Blob 存储

内容寻址存储，用于大文件和附件。

```rust
pub struct BlobStore {
    /// 存储数据
    pub async fn put(&self, data: Vec<u8>) -> Result<BlobId>;
    
    /// 根据哈希获取内容
    pub async fn get(&self, id: &BlobId) -> Result<Option<Vec<u8>>>;
    
    /// 删除内容
    pub async fn delete(&self, id: &BlobId) -> Result<bool>;
    
    /// 垃圾回收（基于引用计数）
    pub async fn gc(&self) -> Result<usize>;
}
```

**特性：**

- SHA-256 内容寻址
- 引用计数垃圾回收
- 去重存储

---

## 14. 终端界面（TUI）

基于 ratatui 构建的终端用户界面。

### 14.1 交互模式

| 模式 | 说明 | 快捷键 |
|------|------|--------|
| `Normal` | 浏览模式 | 方向键滚动 |
| `Input` | 输入模式 | 直接输入 |
| `Command` | Vim 式命令 | `:` 进入 |
| `Help` | 帮助显示 | `?` 进入 |

### 14.2 组件

**SessionView：**

- 消息列表渲染
- 角色着色（用户/助手/系统）
- Markdown 渲染

**InputView：**

- 文本缓冲区
- 光标定位
- 命令历史（上下箭头）

**StatusBar：**

- 当前模型名称
- Token 用量
- 连接状态

**SessionSelector：**

- 历史会话浏览器
- 搜索过滤
- 快速恢复

### 14.3 特性

- 流式消息显示
- 语法高亮（代码块）
- 角色着色
- Token 用量实时显示
- 时间戳显示
- 光标闪烁
- 历史导航

### 14.4 主题系统

```toml
[tui]
theme = "dark"  # 可选: "light", "auto"
```

**自动检测：**

- 检测终端颜色能力
- 适配终端背景色

### 14.5 Markdown 渲染

支持以下元素：

- 粗体/斜体
- 行内代码
- 代码块（带语法高亮）
- 列表（有序/无序）
- 引用块
- 表格

---

## 15. 控制协议与服务器

### 15.1 ClientCommand

客户端发送到服务器的命令：

| 命令 | 说明 |
|------|------|
| `SessionAttach` | 附加到现有会话 |
| `SessionCreate` | 创建新会话 |
| `UserMessage` | 发送用户消息 |
| `TaskCancel` | 取消当前任务 |
| `ApprovalRespond` | 响应审批请求 |
| `SessionRename` | 重命名会话 |
| `SessionClose` | 关闭会话 |
| `Ping` | 心跳请求 |

### 15.2 ServerEvent

服务器推送到客户端的事件：

| 事件 | 说明 |
|------|------|
| `SessionCreated` | 会话已创建 |
| `SessionSnapshot` | 会话快照 |
| `AssistantDelta` | 助手消息增量 |
| `AssistantDone` | 助手响应完成 |
| `ToolCallStarted` | 工具调用开始 |
| `ToolCallCompleted` | 工具调用完成 |
| `ToolCallFailed` | 工具调用失败 |
| `ApprovalRequired` | 需要用户审批 |
| `ApprovalResolved` | 审批已解决 |
| `AgentStatus` | Agent 状态更新 |
| `UsageDelta` | Token 用量更新 |
| `Error` | 错误信息 |
| `Pong` | 心跳响应 |

### 15.3 HistoryEntry

历史记录条目类型：

| 类型 | 说明 |
|------|------|
| `UserMessage` | 用户消息 |
| `AssistantMessage` | 助手消息 |
| `ToolCall` | 工具调用 |

### 15.4 HTTP API

所有 API 受 `auth_middleware` 保护，需要 Bearer token。

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/sessions` | 创建会话 |
| GET | `/api/v1/sessions` | 列出会话 |
| GET | `/api/v1/sessions/:id` | 获取会话 |
| DELETE | `/api/v1/sessions/:id` | 关闭会话 |
| POST | `/api/v1/sessions/:id/cancel` | 取消任务 |
| POST | `/api/v1/approvals/:id/respond` | 审批响应 |

**示例：**

```bash
# 创建会话
curl -X POST http://127.0.0.1:3200/api/v1/sessions \
  -H "Authorization: Bearer <token>"

# 发送消息
curl -X POST http://127.0.0.1:3200/api/v1/sessions/sess_abc123/messages \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"content": "你好"}'
```

### 15.5 WebSocket

**连接：**

```
ws://127.0.0.1:3200/api/v1/ws?token=<token>
```

**协议：**

1. 使用 token 鉴权
2. 协议升级为 WebSocket
3. 客户端发送 ClientCommand
4. 服务端推送 ServerEvent

### 15.6 健康检查

```bash
curl http://127.0.0.1:3200/health
# 返回: "ok"
```

### 15.7 鉴权

**LocalAuth：**

- 启动时生成随机 token
- 存储在 `~/.config/orangecoding/server.token`
- 通过命令行获取：`OrangeCoding serve --show-token`

---

## 16. Worker 运行时

### 16.1 WorkerRuntime

管理 Agent 生命周期、session 监控、事件桥接。

```rust
pub struct WorkerRuntime {
    /// 启动 Worker
    pub async fn start(&self) -> Result<()>;
    
    /// 停止 Worker
    pub async fn stop(&self) -> Result<()>;
    
    /// 创建 Agent 实例
    pub async fn spawn_agent(&self, config: AgentConfig) -> Result<AgentHandle>;
    
    /// 获取 Agent 状态
    pub async fn agent_status(&self, agent_id: AgentId) -> Result<AgentStatus>;
}
```

### 16.2 SessionSupervisor

会话创建、持久化、恢复管理。

```rust
pub struct SessionSupervisor {
    /// 创建新会话
    pub async fn create(&self, cwd: PathBuf) -> Result<SessionHandle>;
    
    /// 恢复会话
    pub async fn resume(&self, session_id: &str) -> Result<SessionHandle>;
    
    /// 关闭会话
    pub async fn close(&self, session_id: &str) -> Result<()>;
    
    /// 取消会话中的任务
    pub async fn cancel(&self, session_id: &str) -> Result<()>;
}
```

### 16.3 AgentExecutor

```rust
#[async_trait]
pub trait AgentExecutor {
    /// 执行一轮对话
    async fn execute_turn(
        &self,
        session_id: SessionId,
        user_message: String,
        event_tx: mpsc::Sender<AgentEvent>,
        cancel_token: CancellationToken,
    ) -> Result<TurnResult>;
}
```

### 16.4 ApprovalBridge

审批桥接系统。

```rust
pub struct ApprovalBridge {
    /// 提交审批请求
    pub async fn request(&self, approval: ApprovalRequest) -> Result<ApprovalId>;
    
    /// 解决审批
    pub async fn resolve(&self, id: ApprovalId, decision: ApprovalDecision) -> Result<()>;
    
    /// 等待审批结果
    pub async fn wait_for_approval(&self, id: ApprovalId) -> Result<ApprovalDecision>;
}

pub struct ApprovalRequest {
    pub tool_name: String,
    pub tool_params: Value,
    pub session_id: SessionId,
}

pub enum ApprovalDecision {
    Allow,
    Deny { reason: String },
}
```

### 16.5 EventBridge

AgentEvent 到 ServerEvent 的转换。

```rust
pub struct EventBridge;

impl EventBridge {
    /// 转换事件
    pub fn convert(&self, event: AgentEvent) -> ServerEvent {
        match event {
            AgentEvent::MessageDelta(delta) => ServerEvent::AssistantDelta { ... },
            AgentEvent::ToolCall(call) => ServerEvent::ToolCallStarted { ... },
            AgentEvent::ToolResult(result) => ServerEvent::ToolCallCompleted { ... },
            // ...
        }
    }
}
```

---

## 17. 配置系统

### 17.1 配置结构

#### OrangeCodingConfig

```rust
pub struct OrangeCodingConfig {
    pub ai: AiConfig,
    pub agent: AgentConfig,
    pub tools: ToolsConfig,
    pub tui: TuiConfig,
    pub logging: LoggingConfig,
    pub permissions: PermissionsConfig,
    pub category: HashMap<String, CategoryConfig>,
    pub skills: SkillsConfig,
}
```

#### AiConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `provider` | String | `"openai"` | AI 提供者 |
| `api_key` | Option<String> | None | API 密钥 |
| `api_secret` | Option<String> | None | API 密钥（部分提供者需要） |
| `model` | String | `"gpt-4"` | 默认模型 |
| `temperature` | f32 | `0.7` | 温度参数 |
| `max_tokens` | u32 | `4096` | 最大 token 数 |
| `base_url` | Option<String> | None | 自定义 API 地址 |

#### AgentConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `max_iterations` | u32 | `50` | 最大迭代次数 |
| `timeout_secs` | u32 | `300` | 超时时间（秒） |
| `auto_approve_tools` | bool | `false` | 自动批准工具调用 |

#### ToolsConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `allowed_paths` | Vec<String> | `["."]` | 允许访问的路径 |
| `blocked_paths` | Vec<String> | `[]` | 禁止访问的路径 |
| `max_file_size` | u64 | `10MB` | 最大文件大小 |

#### TuiConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `theme` | String | `"dark"` | 主题 |
| `show_token_usage` | bool | `true` | 显示 token 用量 |
| `show_timestamps` | bool | `true` | 显示时间戳 |

#### LoggingConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `level` | String | `"info"` | 日志级别 |
| `file` | Option<String> | None | 日志文件路径 |
| `json_format` | bool | `false` | JSON 格式日志 |

### 17.2 配置发现

OrangeCoding 兼容多工具配置目录：

| 优先级 | 路径 | 说明 |
|--------|------|------|
| 1 | `.orangecoding/config.toml` | 项目级配置 |
| 2 | `.claude/config.toml` | Claude 兼容 |
| 3 | `.codex/config.toml` | Codex 兼容 |
| 4 | `.gemini/config.toml` | Gemini 兼容 |
| 5 | `~/.config/orangecoding/config.toml` | 全局配置（XDG 标准） |

### 17.3 JSONC 支持

配置文件支持 JSONC 格式（带注释的 JSON）：

```jsonc
{
    // 这是注释
    "ai": {
        "provider": "openai",  // 使用 OpenAI
        "model": "gpt-5.4"
    }
}
```

### 17.4 配置管理

**ConfigManager API：**

```rust
impl ConfigManager {
    /// 加载配置
    pub fn load() -> Result<OrangeCodingConfig>;
    
    /// 保存配置
    pub fn save(&self, config: &OrangeCodingConfig) -> Result<()>;
    
    /// 更新配置项
    pub fn set(&mut self, key: &str, value: Value) -> Result<()>;
    
    /// 获取配置项
    pub fn get(&self, key: &str) -> Option<Value>;
}
```

**加密存储：**

API 密钥支持加密存储：

```rust
pub struct SecureStorage;

impl SecureStorage {
    /// 存储加密值
    pub fn store(key: &str, value: &str) -> Result<()>;
    
    /// 获取解密值
    pub fn retrieve(key: &str) -> Result<Option<String>>;
}
```

### 17.5 环境变量覆盖

| 环境变量 | 说明 |
|----------|------|
| `OPENAI_API_KEY` | OpenAI API 密钥 |
| `ANTHROPIC_API_KEY` | Anthropic API 密钥 |
| `DEEPSEEK_API_KEY` | DeepSeek API 密钥 |
| `QIANWEN_API_KEY` | 通义千问 API 密钥 |
| `WENXIN_API_KEY` | 文心一言 API 密钥 |
| `OrangeCoding_API_KEY` | OrangeCoding 通用 API 密钥 |
| `OrangeCoding_CONFIG_DIR` | 自定义配置目录 |

---

## 18. 快速开始

### 18.1 构建

```bash
# 克隆仓库
git clone <repository>
cd orangecoding-rewrite-rs

# 构建发布版本
cargo build --release

# 构建调试版本
cargo build
```

### 18.2 运行

```bash
# 查看帮助
./target/release/OrangeCoding --help

# 查看版本
./target/release/OrangeCoding version
```

### 18.3 初始化项目

```bash
# 初始化当前目录
OrangeCoding init

# 初始化指定目录
OrangeCoding init /path/to/project
```

### 18.4 启动会话

```bash
# 交互式启动
OrangeCoding launch

# 使用指定模型
OrangeCoding launch --model claude-opus-4-6

# 单次执行
OrangeCoding launch --prompt "解释 Rust 的生命周期"
```

### 18.5 查看状态

```bash
OrangeCoding status
```

### 18.6 启动控制服务器

```bash
# 默认绑定
OrangeCoding serve

# 自定义绑定地址
OrangeCoding serve --bind 127.0.0.1:3200

# 显示访问令牌
OrangeCoding serve --show-token
```

### 18.7 配置示例

**完整配置模板：**

```toml
# ~/.config/orangecoding/config.toml

[ai]
provider = "anthropic"
api_key = "sk-..."
model = "claude-opus-4-6"
temperature = 0.7
max_tokens = 4096

[agent]
max_iterations = 50
timeout_secs = 300
auto_approve_tools = false

[tools]
allowed_paths = [".", "src", "tests"]
blocked_paths = ["node_modules", ".git", "target"]
max_file_size = 10485760  # 10MB

[tui]
theme = "dark"
show_token_usage = true
show_timestamps = true

[logging]
level = "info"
file = "~/.local/share/orangecoding/OrangeCoding.log"
json_format = false

[permissions]
edit = "ask"
bash = "ask"
web_fetch = "allow"
doom_loop = "deny"
external_directory = "ask"

[category.ultrabrain]
model = "gpt-5.4"
temperature = 0.1
reasoning_effort = "xhigh"
max_tokens = 8192

[skills]
enabled = ["git-master", "playwright"]
```

### 18.8 故障排除

**问题：无法连接到 AI 提供者**

- 检查 API 密钥是否正确设置
- 检查网络连接
- 查看日志文件获取详细错误

**问题：工具执行超时**

- 增加 `agent.timeout_secs` 配置
- 检查工具是否陷入死循环

**问题：会话无法恢复**

- 检查会话文件是否损坏
- 尝试使用 `OrangeCoding launch` 创建新会话

---

## 附录 A：术语表

| 术语 | 说明 |
|------|------|
| Agent | AI 代理，执行特定任务的实体 |
| Category | 任务类别，决定使用的模型和参数 |
| Hook | 钩子，在特定事件点执行的自定义逻辑 |
| Intent | 意图，用户输入的分类 |
| Mesh | 多代理协作网络 |
| Session | 会话，一次完整的交互过程 |
| Tool | 工具，Agent 可调用的功能 |
| TUI | 终端用户界面 |

## 附录 B：许可证

OrangeCoding 采用 Apache-2.0 许可证。

```
Copyright 2024 OrangeCoding Contributors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
