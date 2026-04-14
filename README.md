# OrangeCoding — AI 编程助手 (Rust 实现)

> 基于公开文档的 Clean-room 重新实现，使用 Rust 构建的终端 AI 编程代理。

## 📊 项目状态

| 指标 | 数值 |
|------|------|
| **Crates** | 14 个工作空间成员 |
| **源文件** | 120+ 个 .rs 文件 |
| **代码行数** | ~55,000 行 |
| **测试数量** | 1,254 个单元测试 |
| **测试状态** | ✅ 全部通过 |
| **许可证** | Apache-2.0 |

## 🏗️ 架构

| 模块 | 说明 | 核心功能 |
|------|------|---------|
| **chengcoding-core** | 核心类型 | 消息系统、错误处理、事件总线 |
| **chengcoding-session** | 会话管理 | JSONL 树形存储、分支、Blob 存储 |
| **chengcoding-ai** | AI 提供者 | OpenAI/Anthropic/DeepSeek/通义千问/文心一言、模型角色路由、Fallback 链 |
| **chengcoding-agent** | 代理引擎 | 11 专业 Agent、Category 路由、Intent Gate、工作流编排、Hook/技能系统 |
| **chengcoding-tools** | 工具集 | 22+ 内置工具、权限系统、安全沙箱 |
| **chengcoding-config** | 配置管理 | JSONC 解析、多工具配置发现、加密存储、模型配置 |
| **chengcoding-audit** | 审计安全 | 日志链、数据脱敏、密钥混淆 |
| **chengcoding-mesh** | 多代理协作 | Agent 通信、任务协商、任务重分配、消息总线、共享状态 |
| **chengcoding-mcp** | MCP 协议 | JSON-RPC 2.0、stdio/SSE 传输 |
| **chengcoding-tui** | 终端界面 | Markdown 渲染、主题系统、会话选择器 |
| **chengcoding-cli** | 命令行 | 斜杠命令、RPC 模式、OAuth 认证、Zellij/Tmux 集成 |
| **chengcoding-control-protocol** | 控制协议 | 浏览器/服务器/Worker 共用消息类型、会话/审批/事件模型 |
| **chengcoding-control-server** | 控制服务 | HTTP API、WebSocket、本地令牌鉴权、流式事件分发 |
| **chengcoding-worker** | Worker 运行时 | Agent 生命周期管理、审批桥接、事件转换 |

## 🤖 专业 Agent 系统（11 个）

| Agent | 角色 | 默认模型 | 特性 |
|-------|------|---------|------|
| **Sisyphus** | 主编排器 | claude-opus-4-6 | 全工具权限，可委托 |
| **Hephaestus** | 深度工作者 | gpt-5.4 | 全工具权限，可委托 |
| **Prometheus** | 战略规划器 | claude-opus-4-6 | 规划状态机，可委托 |
| **Atlas** | 任务执行器 | claude-sonnet-4-6 | 执行编排，不可委托 |
| **Oracle** | 架构顾问 | claude-opus-4-6 | 只读，不可委托 |
| **Librarian** | 文档搜索 | minimax-m2.7 | 只读，不可委托 |
| **Explore** | 代码搜索 | grok-code-fast-1 | 只读，不可委托 |
| **Metis** | 计划顾问 | claude-opus-4-6 | 差距分析 |
| **Momus** | 计划审核 | gpt-5.4 | 批评审核 |
| **Junior** | 任务执行 | 按类别分配 | 不可委托 |
| **Multimodal** | 视觉分析 | gpt-5.4 | 白名单模式 |

## 📂 Category 路由（8 种）

| 类别 | 模型 | 用途 |
|------|------|------|
| visual-engineering | gemini | 视觉工程 |
| ultrabrain | gpt-5.4 (xhigh) | 超级大脑 |
| deep | gpt-5.4 (medium) | 深度思考 |
| artistry | gemini (high) | 创意工作 |
| quick | gpt-5.4-mini | 快速响应 |
| unspecified-low | sonnet | 默认低级 |
| unspecified-high | opus (max) | 默认高级 |
| writing | gemini-flash | 写作 |

## 🔧 内置工具（22+ 个）

| 工具 | 说明 |
|------|------|
| `bash` | Shell 命令执行、超时控制、输出截断 |
| `read` | 文件读取 |
| `write` | 文件写入 |
| `edit` | 精确字符串匹配编辑 |
| `grep` | 正则搜索、glob 过滤、上下文行 |
| `find` | 文件查找、glob 匹配、类型过滤 |
| `python` | Python REPL、语法校验、安全检查 |
| `notebook` | Jupyter Notebook 操作 |
| `browser` | 网页交互与截图 |
| `ssh` | 远程命令执行 |
| `lsp` | 语言服务器协议集成 |
| `ask` | 结构化用户交互 |
| `todo` | 分阶段任务跟踪 |
| `task` | 子任务代理委派 |
| `fetch` | URL 内容抓取、HTML 转文本 |
| `web_search` | 多引擎搜索（Brave/Jina） |
| `calc` | 数学表达式求值 |
| `ast_grep` | AST 代码搜索与编辑 |
| `session_*` | 会话列表/读取/搜索/信息 |
| `task_*` | 任务创建/查询/列表/更新 |

## 🔄 编排工作流

- **UltraWork (ulw)** — 输入"ultrawork"触发全自动模式：自动规划、深度研究、并行 Agent、自我修正
- **Prometheus 规划** — 状态机驱动的战略规划工作流
- **Atlas 执行** — 任务编排和智慧系统
- **Boulder 系统** — 会话连续性，崩溃恢复

## 🔌 Agent 间通信

- **AgentCommBus** — 点对点消息传递 + 广播模式
- **NegotiationProtocol** — 任务协商：请求→提议→接受/拒绝
- **HandoffManager** — 任务重分配（过载/能力不匹配/超时）

## 🪝 扩展系统

- **40+ Hook** — 26 种内置 Hook，支持 PreToolUse/PostToolUse/Message/Event/Transform/Params
- **6 内置技能** — git-master, playwright, playwright-cli, agent-browser, dev-browser, frontend-ui-ux
- **权限系统** — Edit/Bash/WebFetch/DoomLoop/ExternalDirectory，Ask/Allow/Deny 三级控制
- **自定义命令** — Markdown 自定义斜杠命令

## 🤖 AI 提供者

- **OpenAI 兼容** — GPT-5.4/GPT-4o 等，可配置 base_url（支持 Ollama/LM Studio/vLLM）
- **Anthropic** — Claude Opus/Sonnet 系列，Messages API 格式
- **DeepSeek** — DeepSeek Chat/Coder
- **通义千问** — 阿里云 DashScope
- **文心一言** — 百度 ERNIE
- **模型 Fallback 链** — 自动故障转移，支持冷却期管理

## 🔐 安全特性

- **沙箱路径** — FileOperationGuard 包装所有文件操作工具
- **默认阻止路径** — ~/.ssh, ~/.aws, /etc 等 14 个敏感路径
- **权限系统** — 5 种权限类型 × 3 级控制
- **审计链** — SHA-256 链式哈希审计日志
- **密钥检测** — 自动检测和脱敏 API 密钥
- **OAuth 2.1** — PKCE (S256), RFC 9728/8414 合规

## 🖥️ 终端集成

- **Zellij** — 子 Agent 运行在 Zellij 面板中，支持布局管理
- **Tmux** — Tmux 回退支持
- **自动检测** — MultiplexerBackend::detect() 自动选择可用后端

## ⚡ 核心特性

- **TTSR 引擎** — 基于正则触发的零成本流式规则注入
- **Hashline 编辑** — SHA-256 内容哈希锚点精确定位
- **上下文压缩** — 自动/手动对话摘要，保持上下文窗口可控
- **记忆系统** — 跨会话知识提取与整合（默认关闭）
- **Intent Gate** — 意图分类引擎（中英文关键词支持）
- **JSONC 配置** — 支持注释和尾逗号的配置文件格式
- **配置发现** — 兼容 .orangecoding/.claude/.codex/.gemini 多工具配置
- **RPC 模式** — JSONL stdio 协议，支持编程式访问
- **MCP 协议** — Model Context Protocol 客户端/服务器

## 🚀 快速开始

```bash
# 构建
cargo build --release

# 运行
./target/release/orangecoding --help

# 初始化项目
orangecoding init

# 启动会话
orangecoding launch

# 查看状态
orangecoding status
```

## 📋 开发

```bash
# 运行所有测试
cargo test --workspace

# 检查编译
cargo check --workspace

# 构建发布版
cargo build --release
```

## 📚 文档

- [快速入门](docs/user-guide/getting-started.md)
- [Agent 系统](docs/user-guide/agents.md)
- [命令参考](docs/user-guide/commands.md)
- [配置指南](docs/user-guide/configuration.md)
- [工作流指南](docs/user-guide/workflows.md)
- [工具参考](docs/reference/tools.md)
- [Hook 参考](docs/reference/hooks.md)
- [权限参考](docs/reference/permissions.md)
- [OAuth 参考](docs/reference/oauth.md)
- [架构概览](docs/architecture/overview.md)
- [Agent 架构](docs/architecture/agent-system.md)
- [Mesh 架构](docs/architecture/mesh.md)
- [安全架构](docs/architecture/security.md)

## 📜 许可证

Apache-2.0

---

> 本项目为 Clean-room 重新实现，仅参考公开文档，未参考任何原始源代码。
