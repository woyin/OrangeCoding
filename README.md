# OrangeCoding Rust

OrangeCoding Rust 是 OrangeCoding 的 Rust 语言实现。这个分支保留原 Rust workspace，用于维护高完整度的终端 AI 编程代理、控制服务、工具系统和多 Agent 编排能力。

> 分支约定：`rust-dev` 保存 Rust 版本；`master` 保存 Go 版本。

## 当前状态

- Rust workspace：15 个 crate，位于 `crates/`
- CLI：`orangecoding-cli`
- TUI：基于 `ratatui` 和 `crossterm`
- 控制服务：`axum` HTTP API + WebSocket
- Agent runtime：工具调用、权限检查、批量执行、工作流编排
- AI provider：OpenAI、Anthropic、DeepSeek、通义千问、文心一言
- 安全能力：权限系统、审计日志、敏感信息处理、不变量检查

## Workspace 结构

| Crate | 作用 |
| --- | --- |
| `orangecoding-core` | 核心消息、事件、错误和通用类型 |
| `orangecoding-config` | JSONC 配置、配置发现、模型配置、加密存储 |
| `orangecoding-ai` | AI provider 抽象、模型角色、fallback 和流式响应 |
| `orangecoding-tools` | 内置工具、权限、沙箱、安全检查、hook |
| `orangecoding-agent` | Agent loop、上下文、工作流、技能、记忆和压缩 |
| `orangecoding-session` | JSONL 会话、树形分支、Blob 存储 |
| `orangecoding-audit` | 审计链、日志、脱敏和密钥检测 |
| `orangecoding-mesh` | 多 Agent 通信、注册表、任务协商和交接 |
| `orangecoding-mcp` | MCP/JSON-RPC 客户端、服务端和传输层 |
| `orangecoding-tui` | 终端 UI、主题、Markdown 渲染 |
| `orangecoding-cli` | 命令行入口、斜杠命令、RPC、OAuth、终端复用器集成 |
| `orangecoding-control-protocol` | 控制面共享协议类型 |
| `orangecoding-control-server` | 本地控制服务、HTTP API、WebSocket、鉴权 |
| `orangecoding-worker` | Worker runtime、审批桥接、事件桥接 |
| `orangecoding-invariant` | 运行时守卫、检查点、验证、回滚和自愈 |

## 快速开始

```bash
# 检查编译
cargo check --workspace

# 运行测试
cargo test --workspace

# 构建 release
cargo build --release

# 查看 CLI
./target/release/orangecoding --help
```

## 常用命令

```bash
# 初始化项目配置
orangecoding init

# 启动交互式会话
orangecoding launch

# 单次任务
orangecoding launch --prompt "explain this repository"

# 启动本地控制服务
orangecoding serve --bind 127.0.0.1:3200
```

## 开发说明

- Rust 版本的源代码在 `crates/`。
- Go 版本的源代码在 `master` 分支的 `modules/`。
- 根目录 `Cargo.toml` 是 Rust workspace 入口。
- 根目录 `tests/invariants/` 保存跨 crate 不变量测试。
- 新 Rust 功能应保持 crate 边界清晰，并优先复用已有核心类型和工具接口。

## 文档

- [系统说明书](docs/SYSTEM_MANUAL.md)
- [架构概览](docs/architecture/overview.md)
- [Agent 架构](docs/architecture/agent-system.md)
- [工具参考](docs/reference/tools.md)
- [权限参考](docs/reference/permissions.md)

## 许可证

Apache-2.0
