# OrangeCoding Go

OrangeCoding Go 是 OrangeCoding 的 Go 语言实现。这个分支以 Go workspace 为主线，目标是提供一个可嵌入、可测试、模块边界清晰的终端 AI 编程代理。

> 分支约定：`master` 保存 Go 版本；`rust-dev` 保存 Rust 版本。

## 当前状态

- Go workspace：15 个 module，位于 `modules/`
- CLI：基于 Cobra，入口为 `modules/cli`
- 控制服务：Gin HTTP API + Gorilla WebSocket
- Agent runtime：支持会话生命周期、工具调用循环、事件流
- Harness：内建长任务推进、真实工具 schema 暴露、sub-agent delegation brief
- AI provider：OpenAI/兼容格式、Anthropic/兼容格式、DeepSeek、通义千问、文心一言；内置 GPT、Opus、Kimi K2.6、GLM-5.1 常用别名
- 测试：Go 模块测试已通过

部分用户界面仍在接线中：`launch --text`、默认 TUI 模式、若干高级工具当前是 stub。`launch -p` 已可运行单次 Agent 任务。

## 模块结构

| 模块 | 作用 |
| --- | --- |
| `modules/core` | 核心 ID、消息、事件、错误、token usage 类型 |
| `modules/config` | JSONC 配置加载、保存、查询、加密辅助 |
| `modules/ai` | AI provider 抽象、流式响应、fallback、模型路由 |
| `modules/tools` | 工具接口、注册表、权限、安全检查、内置工具 |
| `modules/agent` | Agent loop、上下文、工具执行、子 Agent、工作流 |
| `modules/session` | JSONL 会话存储、树形会话、Blob 存储 |
| `modules/audit` | 审计日志、hash chain、敏感信息处理 |
| `modules/mesh` | 多 Agent 消息总线、注册表、任务协商、任务编排 |
| `modules/mcp` | MCP/JSON-RPC 客户端、服务端、传输层 |
| `modules/tui` | Bubble Tea TUI 模型、视图、主题、Markdown 渲染 |
| `modules/control-protocol` | 控制面共享消息类型 |
| `modules/control-server` | HTTP/WebSocket 控制服务 |
| `modules/worker` | Agent session runtime 与 executor |
| `modules/cli` | `orangecoding` 命令行入口 |
| `modules/invariant` | 不变量、检查点、回滚、自愈策略 |

## 快速开始

```bash
# 查看 CLI
go run ./modules/cli --help

# 初始化配置
go run ./modules/cli init

# 单次任务模式
go run ./modules/cli launch -p "explain this repository"

# 启动控制服务
go run ./modules/cli serve --addr 127.0.0.1:3200
```

默认配置路径为 `~/.orangecoding/config.json`。

常见模型可以通过配置切换，例如 `default_provider` 使用 `gpt`、`opus`、`kimi` 或 `glm`；`harness.reasoning_effort` 和 `harness.reasoning_budget_tokens` 控制思考深度。

## 测试

仓库根目录不是单独的 Go module，不能直接使用 `go test ./...`。请按 workspace module 显式测试：

```bash
go test ./modules/core ./modules/ai ./modules/audit ./modules/config \
  ./modules/control-protocol ./modules/session ./modules/tools \
  ./modules/agent/... ./modules/mesh ./modules/mcp ./modules/tui \
  ./modules/worker ./modules/control-server ./modules/cli ./modules/invariant
```

## 开发说明

- Go 版本的源代码在 `modules/`。
- Rust 版本的源代码保留在 `rust-dev` 分支的 `crates/`。
- `go.work` 是 Go 版本的 workspace 入口。
- 新功能应优先补齐 Go 模块，并保持模块间依赖单向、清晰。
- Harness 长任务、长推理和中文表达策略见 `docs/harness_engineering_go.md`。

## 许可证

Apache-2.0
