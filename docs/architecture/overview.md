# 架构概览

## Crate 架构图

```
                    ┌──────────────┐
                    │  ceair-cli   │  命令行入口
                    │  (OAuth/Zellij/RPC/斜杠命令) │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
      ┌───────▼──────┐ ┌──▼──────┐ ┌───▼──────────┐
      │  ceair-tui   │ │ceair-mcp│ │ ceair-agent   │
      │  终端界面    │ │MCP协议  │ │ 代理引擎      │
      └──────────────┘ └─────────┘ │ (11 Agent     │
                                   │  工作流/Hook)  │
                                   └───────┬───────┘
                           ┌───────────────┼───────────────┐
                           │               │               │
                   ┌───────▼──────┐ ┌──────▼───────┐ ┌────▼────────┐
                   │  ceair-ai    │ │ ceair-tools   │ │ ceair-mesh  │
                   │  AI提供者    │ │ 工具集+权限   │ │ 多Agent协作 │
                   │  Fallback链  │ │ 安全沙箱      │ │ 通信/协商   │
                   └──────────────┘ └──────────────┘ └─────────────┘
                           │               │               │
                   ┌───────▼───────────────▼───────────────▼──────┐
                   │                ceair-config                   │
                   │           配置管理 / JSONC / 加密              │
                   └──────────────────┬────────────────────────────┘
                                      │
              ┌───────────────────────┼───────────────────────┐
              │                       │                       │
      ┌───────▼──────┐      ┌────────▼───────┐      ┌───────▼──────┐
      │ ceair-core   │      │ ceair-session  │      │ ceair-audit  │
      │ 核心类型     │      │ 会话管理       │      │ 审计安全     │
      └──────────────┘      └────────────────┘      └──────────────┘
```

## 各 Crate 职责

### ceair-core — 核心类型库

- `CeairError` — 统一错误类型（thiserror）
- `Event` / `EventBus` — 事件系统
- `Message` — 聊天消息模型
- `AgentId` / `AgentRole` / `AgentStatus` — Agent 标识类型

### ceair-session — 会话管理

- JSONL 树形存储 — 支持多分支对话历史
- Blob 外部存储 — 大型附件分离存储
- 会话恢复 — 崩溃后自动恢复

### ceair-ai — AI 提供者

- 5 个提供者 — OpenAI, Anthropic, DeepSeek, 通义千问, 文心一言
- 流式传输 — SSE 流式响应处理
- 模型角色路由 — Default/Smol/Slow/Plan/Commit
- Fallback 链 — 自动故障转移 + 冷却期管理

### ceair-agent — 代理引擎（最大 crate）

- 11 个专业 Agent — 每个有独立的模型/工具配置
- Category 路由 — 8 种内置类别
- Intent Gate — 意图分类引擎
- 工作流 — Prometheus/Atlas/Boulder/UltraWork
- Hook 系统 — 26 种内置 Hook
- 技能系统 — 6 种内置技能 + 自定义技能
- TTSR — 流式规则注入引擎
- Hashline — SHA-256 锚点编辑
- 上下文压缩 — 自动/手动摘要
- 记忆系统 — 跨会话知识

### ceair-tools — 工具集

- 22+ 内置工具 — 文件/Shell/搜索/网络/LSP 等
- 权限系统 — 5 种权限 × 3 级控制
- 安全沙箱 — FileOperationGuard 路径验证
- 工具注册表 — 动态注册/发现

### ceair-config — 配置管理

- JSONC 解析 — 支持注释和尾逗号
- 配置发现 — 兼容 .ceair/.claude/.codex/.gemini
- 加密存储 — AES-256-GCM 密钥加密
- 模型配置 — 模型别名和参数

### ceair-audit — 审计安全

- 审计链 — SHA-256 链式哈希日志
- 数据脱敏 — 自动检测和替换敏感信息
- 密钥检测 — 正则模式匹配 API 密钥

### ceair-mesh — 多 Agent 协作

- AgentCommBus — 点对点 + 广播消息
- NegotiationProtocol — 任务协商
- HandoffManager — 任务重分配
- MessageBus — 异步消息总线
- SharedState — 线程安全共享状态
- TaskOrchestrator — 任务编排
- ModelRouter — 模型路由

### ceair-mcp — MCP 协议

- JSON-RPC 2.0 — 标准协议实现
- stdio/SSE 传输 — 两种传输模式
- 客户端/服务器 — 双向支持

### ceair-tui — 终端界面

- Markdown 渲染 — 终端 Markdown 支持
- 主题系统 — 可自定义配色
- 会话选择器 — 多会话管理
- 状态栏 — 实时状态显示

### ceair-cli — 命令行

- 斜杠命令 — 26 个命令
- RPC 模式 — JSONL stdio 编程接口
- OAuth 2.1 — MCP 服务器认证
- Zellij/Tmux — 终端复用器集成

## 数据流

```
用户输入 → CLI → Intent Gate → Agent 选择 → Category 路由
    → 模型选择 → AI 提供者 → 流式响应
    → 工具调用 → 权限检查 → 沙箱验证 → 执行
    → Hook 触发 → 审计日志 → 响应渲染
```

## 安全边界

1. **工具层** — FileOperationGuard 包装所有文件工具
2. **权限层** — PermissionChecker 控制工具访问
3. **路径层** — SecurityPolicy 阻止敏感路径
4. **审计层** — AuditChain 记录所有操作
5. **密钥层** — SecretDetector 脱敏敏感数据
