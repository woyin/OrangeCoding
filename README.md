# OrangeCoding

TypeScript monorepo 实现的 AI 编码代理系统。可嵌入、可测试、模块边界清晰。

## 快速开始

```bash
# 环境要求: Node.js >= 20
npm install
npm run build

# 运行 CLI
node packages/cli/dist/main.js --help

# 单次任务模式
node packages/cli/dist/main.js launch -p "explain this repository"
```

默认配置路径: `~/.orangecoding/config.json`

## 开发

```bash
npm run typecheck   # 类型检查
npm run lint        # 代码检查
npm test            # 运行测试
npm run build       # 构建所有包
npm run clean       # 清理构建产物
```

## 架构

18 个 TypeScript 包，分层设计，依赖单向：

```
┌─────────────────────────────────────────────────┐
│  cli  tui  multiplexer  plugin-sdk              │  用户层
├─────────────────────────────────────────────────┤
│  worker  pane-agent  agent                      │  Agent 层
├─────────────────────────────────────────────────┤
│  mesh  control-server  control-protocol         │  协作层
├─────────────────────────────────────────────────┤
│  tools  mcp  ai                                 │  能力层
├─────────────────────────────────────────────────┤
│  config  session  audit  invariant              │  基础设施
├─────────────────────────────────────────────────┤
│  core                                          │  核心类型
└─────────────────────────────────────────────────┘
```

| 包 | 职责 |
| --- | --- |
| `core` | 核心 ID、消息、事件、错误、token usage 类型 |
| `ai` | AI provider 抽象、流式响应、fallback、模型路由 |
| `config` | JSONC 配置加载、保存、查询、加密辅助 |
| `tools` | 工具接口、注册表、权限、安全检查、内置工具 |
| `agent` | Agent loop、上下文、工具执行、子 Agent、工作流 |
| `session` | JSONL 会话存储、树形会话、Blob 存储 |
| `audit` | 审计日志、hash chain、敏感信息处理 |
| `mesh` | 多 Agent 消息总线、注册表、任务协商、任务编排 |
| `mcp` | MCP/JSON-RPC 客户端、服务端、传输层 |
| `tui` | 终端 UI 模型、视图、主题、Markdown 渲染 |
| `control-protocol` | 控制面共享消息类型 |
| `control-server` | WebSocket 控制服务 |
| `worker` | Agent session runtime 与 executor |
| `multiplexer` | 终端复用器（tmux/zellij）后端 |
| `invariant` | 不变量检查、检查点、回滚、自愈策略 |
| `cli` | `orangecoding` 命令行入口 |
| `pane-agent` | 终端面板 Agent 可执行文件 |
| `plugin-sdk` | TypeScript SDK，用于编写工具插件 |

## AI Provider 支持

通过配置切换 provider，支持：

- OpenAI / 兼容格式（GPT-5.1 等）
- Anthropic / 兼容格式（Claude Opus 4.7 等）
- DeepSeek
- 通义千问（Qianwen）
- 文心一言（Wenxin）
- Kimi K2.6 / Moonshot
- GLM-5.1 / BigModel

配置项 `harness.reasoning_effort` 和 `harness.reasoning_budget_tokens` 控制推理深度。

## 许可证

Apache-2.0
