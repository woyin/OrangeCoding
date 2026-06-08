# OrangeCoding - AI Coding Agent System

## 架构
TypeScript monorepo (packages/*)。TS 是唯一实现语言。

## 构建与测试
- 构建: `npm run build` (tsc)
- 测试: `npm test` (jest)
- 类型检查: `npm run typecheck`
- 代码检查: `npm run lint` (eslint)
- 单测优先: `jest --testPathPattern <file>`

## 代码规范
- ES modules, strict mode
- camelCase 命名
- 提交前: `npm run typecheck`

## 项目结构
- `packages/` — TypeScript 实现
  - `agent/` — 核心 Agent 逻辑
  - `ai/` — AI 提供商集成
  - `audit/` — 审计日志
  - `cli/` — 命令行接口
  - `config/` — 配置管理
  - `control-protocol/` — 控制协议
  - `control-server/` — 控制服务器
  - `core/` — 核心类型和工具
  - `invariant/` — 不变量检查
  - `mcp/` — Model Context Protocol
  - `mesh/` — Agent 网格协作
  - `multiplexer/` — 多路复用器
  - `pane-agent/` — 面板 Agent
  - `plugin-sdk/` — 插件 SDK
  - `session/` — 会话管理
  - `tools/` — 工具定义
  - `tui/` — 终端 UI
  - `worker/` — 工作进程
- `docs/` — 设计文档
- `.github/` — CI/CD 配置

## 禁止事项
- 不要提交 secrets、credentials、API keys
- 不要绕过 git hooks (`--no-verify`)
- 不要引入 Go 或 Rust 代码
