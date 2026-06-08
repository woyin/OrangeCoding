# Web / 公网 Agent 控制面完整设计方案

> 目标：为 OrangeCoding 增加一个可从浏览器访问、并可在公网环境中安全接入的 Agent 控制面。本文档面向实现团队，强调可执行性、模块边界、安全模型和分阶段落地。

## 1. 目标

### 1.1 用户目标

用户需要：

1. 从浏览器查看和控制本地或远程运行的 OrangeCoding Agent
2. 在公网环境中安全地连接和控制 Agent，而不是局限于本机 `stdio`
3. 保留现有 Agent、工具、权限和审计体系，而不是重写一套新的运行时
4. 支持流式输出、工具调用可视化、任务取消、权限审批和会话恢复

### 1.2 设计目标

本方案必须满足：

1. **兼容现有架构**：尽量复用 `orangecoding-agent`、`orangecoding-tools`、`orangecoding-mcp`、`orangecoding-session`
2. **本地优先，公网可扩展**：先支持 `localhost` Web 控制，再升级到公网 Gateway
3. **安全默认关闭**：公网接入不允许“零鉴权、零审批”直接操控本地机器
4. **流式交互**：支持 token 流、状态流、工具调用流、审批流
5. **多会话和多 Agent**：支持一个浏览器会话控制多个运行中的 Agent 会话
6. **可分包实施**：可拆给多个 Agent 并行开发

### 1.3 非目标

本方案暂不包含：

1. 多租户 SaaS 计费系统
2. 浏览器端直接运行 Agent
3. 将所有工具完全重写为远程 RPC 工具
4. 完整移动端原生 App

---

## 2. 当前系统约束

基于现有代码库，当前约束如下：

1. `orangecoding-cli` 是主入口，当前没有 `serve` 子命令，只有 `Launch / Config / Status / Version`
2. `orangecoding-mcp` 的传输抽象存在，但当前仅实现 `stdio`
3. `orangecoding-cli::rpc` 提供的是 JSONL over `stdio`，不是 HTTP / WebSocket
4. `orangecoding-agent` 和 `orangecoding-mesh` 以进程内通信为主，`mailbox` 当前是内存实现
5. 权限体系以本地交互式确认为主，适合 CLI，不足以直接暴露公网
6. OAuth 能力主要用于“连接远程 MCP 服务”，不是“把本地 Agent 作为公网服务暴露”

这意味着：

1. **本地 Web 控制面**可以在现有运行时上加一个服务层
2. **公网控制面**必须引入网关、认证和审批边界
3. 不应直接把 `OrangeCoding launch` 或 Agent Worker 对外暴露

---

## 3. 总体方案

采用两层架构：

1. **Control Gateway**：浏览器可连接的 HTTP/WebSocket 服务
2. **Agent Worker Runtime**：实际执行 Agent、工具和会话管理的本地/远程运行时

### 3.1 推荐拓扑

```text
Browser
  │
  │ HTTPS + WebSocket
  ▼
Control Gateway
  │
  ├── AuthN/AuthZ
  ├── Session API
  ├── Approval API
  ├── Stream Fanout
  │
  │ mTLS / signed WebSocket / reverse RPC
  ▼
Agent Worker Runtime
  │
  ├── orangecoding-agent
  ├── orangecoding-tools
  ├── orangecoding-session
  ├── orangecoding-audit
  └── local filesystem / model providers / MCP
```

### 3.2 为什么要分成 Gateway 和 Worker

不推荐把 Worker 直接暴露公网，原因：

1. Worker 具备文件编辑、bash、网络访问能力，攻击面过大
2. Worker 当前权限模型是面向本地交互，不是面向远程调用
3. Gateway 可以承接认证、速率限制、审批、审计聚合、会话路由
4. Worker 可以部署在内网、本地机器、跳板机或单租户主机

### 3.3 分阶段实施

1. **Phase A: Local Web Control**
   - `OrangeCoding serve --bind 127.0.0.1:PORT`
   - 浏览器通过 `localhost` 控制本机 Agent
2. **Phase B: Remote Worker Link**
   - Worker 主动连 Gateway，建立反向控制通道
3. **Phase C: Public Control Plane**
   - 多用户登录、审批流、组织级策略、受控公网入口

---

## 4. 组件设计

### 4.1 新增 crate 建议

推荐新增以下 crate：

1. `crates/orangecoding-control-protocol`
   - 定义浏览器/Gateway/Worker 共用协议
   - 包括事件、命令、会话模型、审批模型

2. `crates/orangecoding-control-server`
   - 提供 HTTP API、WebSocket、身份认证、流式分发
   - 初期可被 `orangecoding-cli serve` 调用

3. `crates/orangecoding-worker`
   - 作为 Agent Worker 宿主
   - 封装 `AgentLoop`、会话路由、工具审批桥接

### 4.2 对现有 crate 的修改

1. `orangecoding-cli`
   - 新增 `serve` 子命令
   - 支持 `local` 和 `worker` 两种模式

2. `orangecoding-agent`
   - 提供可嵌入的 `AgentRuntimeService`
   - 把运行时事件结构化导出

3. `orangecoding-session`
   - 增加浏览器可消费的会话索引、运行态和事件回放接口

4. `orangecoding-audit`
   - 增加远程请求上下文、用户 ID、审批记录、连接来源

5. `orangecoding-tools`
   - 将“需要用户确认”的工具调用改造为可挂起/恢复

6. `orangecoding-mcp`
   - 可选：为远程 MCP 打基础，新增 WebSocket/SSE transport

---

## 5. 运行模式

### 5.1 Mode 1: Localhost Web UI

用途：

1. 浏览器控制本机运行的 OrangeCoding
2. 开发和调试阶段最快落地

特点：

1. 服务仅监听 `127.0.0.1`
2. 可使用本地 session cookie 或一次性 token
3. 权限审批仍由同一用户完成

### 5.2 Mode 2: Managed Worker

用途：

1. 远端机器运行 Worker
2. Worker 主动出站连接 Gateway

特点：

1. 无需给 Worker 开公网入站端口
2. 适合 NAT、家庭网络、企业内网
3. Gateway 只做转发和控制，不直接执行工具

### 5.3 Mode 3: Public Control Plane

用途：

1. 多人、多项目、多 Worker 的集中控制

特点：

1. 强制登录
2. 强制 TLS
3. 强制审批与策略约束
4. 所有高危工具默认不自动执行

---

## 6. 协议设计

### 6.1 协议原则

控制面协议统一为两类：

1. **HTTP API**
   - 短请求：登录、会话列表、历史查询、审批提交
2. **WebSocket**
   - 长连接：用户输入、模型流式输出、工具事件、状态更新

### 6.2 浏览器到 Gateway 的 WebSocket

建议路径：

```text
GET /api/v1/ws?session_id=<sid>
```

消息统一结构：

```json
{
  "type": "user_message",
  "request_id": "req_123",
  "session_id": "sess_123",
  "payload": {}
}
```

### 6.3 事件类型

#### 浏览器发往 Gateway

1. `session.attach`
2. `session.create`
3. `user.message`
4. `task.cancel`
5. `approval.respond`
6. `session.rename`
7. `session.close`
8. `ping`

#### Gateway/Worker 发往浏览器

1. `session.created`
2. `session.snapshot`
3. `assistant.delta`
4. `assistant.done`
5. `tool.call.started`
6. `tool.call.completed`
7. `tool.call.failed`
8. `approval.required`
9. `approval.resolved`
10. `agent.status`
11. `usage.delta`
12. `audit.notice`
13. `error`
14. `pong`

### 6.4 Worker 连接协议

推荐 Worker 主动连接 Gateway：

```text
GET /api/v1/worker/connect
Authorization: Bearer <worker_token>
X-Worker-Id: worker_xxx
X-Worker-Version: 0.1.0
```

Worker 通道上的消息：

1. `worker.hello`
2. `worker.capabilities`
3. `worker.heartbeat`
4. `worker.session.attach`
5. `worker.session.detach`
6. `worker.command`
7. `worker.event`
8. `worker.result`

### 6.5 是否复用 MCP

不建议一开始直接把浏览器控制协议设计成 MCP。

原因：

1. MCP 当前更适合工具发现和调用
2. 浏览器控制面需要会话、审批、流式 UI 状态，这超出 MCP 当前抽象
3. 现有仓库中的 MCP 仍主要是 `stdio`

建议：

1. **浏览器控制面使用自定义控制协议**
2. **Worker 内部继续复用 MCP 连接外部工具**
3. 后续如需跨实现互通，再考虑把 Worker 控制协议映射到 MCP 扩展

---

## 7. API 设计

### 7.1 HTTP API

#### 会话 API

1. `POST /api/v1/sessions`
   - 创建会话
2. `GET /api/v1/sessions`
   - 列出会话
3. `GET /api/v1/sessions/:id`
   - 获取会话元信息
4. `GET /api/v1/sessions/:id/history`
   - 获取历史消息
5. `POST /api/v1/sessions/:id/cancel`
   - 取消当前任务
6. `DELETE /api/v1/sessions/:id`
   - 关闭会话

#### 审批 API

1. `GET /api/v1/approvals`
2. `POST /api/v1/approvals/:id/approve`
3. `POST /api/v1/approvals/:id/deny`

#### Worker API

1. `GET /api/v1/workers`
2. `GET /api/v1/workers/:id`
3. `POST /api/v1/workers/:id/drain`
4. `POST /api/v1/workers/:id/revoke`

### 7.2 数据结构

#### Session

```json
{
  "id": "sess_01",
  "title": "Investigate build failure",
  "state": "running",
  "worker_id": "worker_local",
  "created_at": "2026-04-11T10:00:00Z",
  "updated_at": "2026-04-11T10:01:10Z"
}
```

#### ApprovalRequest

```json
{
  "id": "apr_01",
  "session_id": "sess_01",
  "tool_name": "bash",
  "risk_level": "high",
  "summary": "Run cargo test in workspace",
  "arguments": {
    "cmd": "cargo test"
  },
  "expires_at": "2026-04-11T10:05:00Z"
}
```

#### ToolEvent

```json
{
  "type": "tool.call.started",
  "session_id": "sess_01",
  "tool_call_id": "tool_01",
  "tool_name": "grep",
  "arguments_preview": {
    "pattern": "serve",
    "path": "crates/"
  }
}
```

---

## 8. Worker 设计

### 8.1 Worker 职责

Worker 负责：

1. 承载一个或多个 Agent 会话
2. 执行工具
3. 管理本地权限和安全策略
4. 把运行事件上报 Gateway
5. 对需要审批的动作进入挂起态

### 8.2 Worker 内部模块

1. `WorkerConnectionManager`
   - 维持到 Gateway 的连接
2. `SessionSupervisor`
   - 管理会话生命周期
3. `AgentRuntimeAdapter`
   - 包装 `orangecoding-agent`
4. `ApprovalBridge`
   - 处理工具审批挂起和恢复
5. `EventPublisher`
   - 结构化上报事件
6. `RuntimePolicy`
   - 执行本地策略和沙箱约束

### 8.3 Worker 生命周期

```text
Boot
  -> load config
  -> register worker
  -> connect gateway
  -> advertise capabilities
  -> accept session assignment
  -> run session
  -> drain / shutdown
```

### 8.4 会话调度

初期建议：

1. 一个 Worker 支持多个会话
2. 每个会话串行处理用户输入
3. 工具执行可保留当前并发模型

后续可扩展：

1. 基于模型资源和工具压力做队列调度
2. 支持优先级和会话抢占

---

## 9. 浏览器端设计

### 9.1 页面结构

最小可用 UI：

1. 登录页
2. 会话列表页
3. 会话详情页
4. 审批中心页
5. Worker 状态页

### 9.2 会话详情页功能

必须包括：

1. 用户输入框
2. Assistant 流式输出
3. Tool timeline
4. 当前状态栏
5. Token / cost 展示
6. 取消按钮
7. 审批弹窗
8. 错误和断线重连提示

### 9.3 状态模型

前端状态建议分层：

1. `connection_state`
2. `session_state`
3. `stream_buffer`
4. `tool_events`
5. `approval_queue`
6. `worker_status`

---

## 10. 认证与授权

### 10.1 身份模型

区分三类主体：

1. `human_user`
2. `worker`
3. `service_admin`

### 10.2 推荐认证方案

#### Phase A

1. 本地模式使用一次性本地 token
2. 可选 OS 用户绑定

#### Phase B/C

1. 浏览器用户：OIDC / OAuth 2.1 登录
2. Worker：短期 JWT 或 mTLS client cert
3. 管理接口：RBAC + 审计

### 10.3 授权模型

RBAC 最小角色：

1. `viewer`
   - 可查看会话
2. `operator`
   - 可发送消息、取消任务、审批低风险动作
3. `admin`
   - 可管理 worker、策略、令牌

### 10.4 审批模型

不能继续沿用“CLI 弹框式 ask”语义，必须升级为：

1. Worker 把待审批动作挂起
2. Gateway 生成审批记录
3. 浏览器用户审批
4. Gateway 把结果发回 Worker
5. Worker 恢复或拒绝执行

审批对象至少包括：

1. `bash`
2. `edit`
3. `external_directory`
4. `web_fetch`
5. `ssh`

---

## 11. 安全设计

### 11.1 总原则

公网模式下默认假设：

1. 浏览器和 Gateway 所在网络不可信
2. Gateway 和 Worker 之间链路不可信
3. 浏览器用户身份可能被盗用
4. Worker 所在机器权限极高，必须最小暴露

### 11.2 核心安全规则

1. Worker 不直接暴露公网入站端口
2. 所有外部连接强制 TLS
3. 所有连接都有短期凭证和可撤销机制
4. 高危工具默认需审批
5. 会话和工具事件全部落审计
6. 每个 Worker 必须绑定明确的策略集

### 11.3 策略分层

#### Gateway Policy

1. 用户认证
2. 角色授权
3. 速率限制
4. Worker 分配
5. 事件脱敏

#### Worker Policy

1. 文件系统 allowed/blocklist
2. bash allow/deny/ask
3. 网络 egress 限制
4. 模型 provider allowlist
5. 本地 secret scan

### 11.4 公网模式新增风险

1. 会话劫持
2. 重放攻击
3. 伪造审批结果
4. Worker 冒充
5. Prompt injection 导致远程危险工具调用
6. 内网探测和 SSRF

### 11.5 对应防护

1. WebSocket 握手附带 access token，服务端二次校验 session 归属
2. 所有命令带 `request_id` 和时间窗口，服务端去重
3. 审批结果使用带签名的 server-side 状态，不信任前端裸 payload
4. Worker 使用短期 token 或 mTLS 证书
5. 高危工具双重校验：模型意图不等于执行授权
6. 网络请求延续当前的 localhost / 内网阻止策略，并增强 CIDR 检查

### 11.6 密钥和凭证

1. Gateway 不应持有 Worker 机器上的本地凭证
2. Worker 端本地模型/API 密钥继续由现有加密配置管理
3. Worker 注册令牌必须支持吊销和轮换

---

## 12. 会话与状态持久化

### 12.1 会话持久化

复用 `orangecoding-session`，新增索引能力：

1. `session_metadata`
2. `latest_state`
3. `assigned_worker`
4. `last_event_offset`

### 12.2 事件存储

建议引入 event log：

1. 所有流式事件都可重放
2. 浏览器断线后可恢复
3. 调试和审计更容易

事件存储可以先用：

1. 本地 JSONL
2. 内存 + 定期刷盘

后续可扩展到：

1. SQLite
2. Postgres

### 12.3 审批持久化

审批记录必须可持久化，字段至少包括：

1. `approval_id`
2. `session_id`
3. `worker_id`
4. `tool_name`
5. `risk_level`
6. `request_payload`
7. `decision`
8. `decision_by`
9. `decision_at`

---

## 13. 观测与审计

### 13.1 日志

建议统一结构化日志字段：

1. `trace_id`
2. `request_id`
3. `session_id`
4. `worker_id`
5. `user_id`
6. `tool_call_id`
7. `approval_id`

### 13.2 Metrics

至少暴露：

1. 活跃 WebSocket 数
2. 活跃会话数
3. Worker 在线数
4. 会话创建速率
5. 工具审批等待时长
6. 模型响应时延
7. 工具错误率
8. 会话取消率

### 13.3 审计

所有以下事件必须进入 `orangecoding-audit`：

1. 登录成功/失败
2. Worker 注册/撤销
3. 创建会话
4. 发送消息
5. 触发审批
6. 审批通过/拒绝
7. 高危工具执行
8. 会话关闭

---

## 14. 实施计划

## Phase A: Local Web Control

### 范围

1. 新增 `OrangeCoding serve`
2. 本地监听 `127.0.0.1`
3. 提供 HTTP + WebSocket
4. 可创建会话、发送消息、看流式输出、取消任务
5. 支持本地审批弹窗

### 不包含

1. 公网登录
2. 多用户
3. 远程 Worker

### 验收标准

1. 本机浏览器可打开会话页面
2. 能发消息并收到流式输出
3. 能看到工具调用事件
4. 高危工具能进入审批挂起
5. 会话关闭后可恢复历史

## Phase B: Remote Worker Link

### 范围

1. Worker 主动连接 Gateway
2. Gateway 可把会话分配到远程 Worker
3. 浏览器可看到 Worker 在线状态
4. 审批可穿透到远程 Worker

### 验收标准

1. Worker 离线/重连可感知
2. 会话能路由到指定 Worker
3. 远程工具调用和审批闭环可用
4. 断线后会话和事件可恢复

## Phase C: Public Control Plane

### 范围

1. OIDC 登录
2. RBAC
3. Worker token / mTLS
4. 审计增强
5. 速率限制
6. 管理后台

### 验收标准

1. 未授权用户无法访问任何会话
2. Worker 凭证可轮换和吊销
3. 高危动作全量审计
4. 公网暴露不需要开放 Worker 入站端口

---

## 15. 面向其他 Agent 的任务拆分

以下拆分按“可并行开发、写集尽量分离”原则设计。

### Agent 1: 控制协议与类型系统

负责：

1. 新建 `orangecoding-control-protocol`
2. 定义 HTTP/WS/Worker 消息结构
3. 定义会话、审批、事件、错误码
4. 提供 serde 测试和兼容性测试

交付物：

1. 协议 crate
2. JSON schema 或示例 payload
3. 单测

### Agent 2: Local Control Server

负责：

1. 新建 `orangecoding-control-server`
2. 提供 HTTP 路由和 WebSocket
3. 本地 token 鉴权
4. Session API 和流式事件 fanout

交付物：

1. `serve` 所需 server 代码
2. WebSocket 会话附着逻辑
3. 集成测试

### Agent 3: Worker Runtime Adapter

负责：

1. 新建 `orangecoding-worker`
2. 对接 `orangecoding-agent`
3. 会话生命周期管理
4. 将 Agent 运行事件转换为控制协议事件

交付物：

1. Worker 宿主
2. 会话 supervisor
3. 运行态事件桥接

### Agent 4: Approval Bridge

负责：

1. 改造工具调用审批为挂起/恢复模型
2. 定义 `approval.required` / `approval.resolved`
3. 打通 UI -> Gateway -> Worker -> ToolExecutor

交付物：

1. 可恢复的审批桥
2. 高危工具策略映射
3. 审批状态测试

### Agent 5: Session Persistence & Replay

负责：

1. 会话索引
2. 事件日志
3. 断线恢复与历史重放

交付物：

1. 历史查询 API 支撑
2. 重放逻辑
3. 兼容现有 `orangecoding-session`

### Agent 6: Security & Auth

负责：

1. 本地 token
2. Worker token 设计
3. 公网模式 OIDC/RBAC 方案落代码骨架
4. 审计字段扩展

交付物：

1. 身份模型
2. 中间件
3. 审计与安全测试

### Agent 7: Frontend UI

负责：

1. 会话列表
2. 会话详情页
3. Tool timeline
4. Approval UI
5. Worker 状态页

交付物：

1. Web UI
2. WebSocket 客户端
3. E2E 测试

---

## 16. 文件与目录建议

```text
crates/
  orangecoding-control-protocol/
    src/
      browser.rs
      worker.rs
      session.rs
      approval.rs
      event.rs
      error.rs

  orangecoding-control-server/
    src/
      lib.rs
      auth.rs
      routes.rs
      ws.rs
      session_api.rs
      approval_api.rs
      worker_registry.rs
      event_hub.rs

  orangecoding-worker/
    src/
      lib.rs
      runtime.rs
      supervisor.rs
      gateway_client.rs
      approval_bridge.rs
      session_bridge.rs

crates/orangecoding-cli/src/commands/
  serve.rs
```

可选前端：

```text
web/
  src/
    pages/
    components/
    hooks/
    api/
```

---

## 17. 关键技术决策

### 决策 1：浏览器协议使用 WebSocket，不使用纯 SSE

原因：

1. 浏览器需要双向通信
2. 审批、取消、attach 都需要客户端主动发送命令
3. 一个连接可承载双向事件

### 决策 2：Worker 主动连 Gateway，不做公网入站

原因：

1. 更安全
2. 更易穿透 NAT
3. 部署更简单

### 决策 3：审批改造为挂起/恢复，不保留同步 ask

原因：

1. 公网控制一定是异步
2. 浏览器审批不是终端阻塞输入

### 决策 4：不把浏览器控制面直接建在 MCP 之上

原因：

1. 当前 MCP 主要用于工具协议，不适合作为完整会话控制协议

---

## 18. 风险与应对

### 风险 1：工具审批改造会牵动 `orangecoding-agent` 执行链

应对：

1. 先实现少量高危工具的挂起/恢复
2. 用 trait 或 channel 注入审批桥

### 风险 2：现有会话存储偏离事件流模型

应对：

1. 保留 `orangecoding-session` 作为历史真相源
2. 新增事件日志层而不是硬改原存储

### 风险 3：Gateway 与 Worker 协议过早复杂化

应对：

1. Phase A 不引入远程 Worker
2. Phase B 先支持单 Gateway 单 Worker

### 风险 4：公网模式安全范围失控

应对：

1. 公网模式单独 feature gate
2. 默认只发布 localhost 模式

---

## 19. 最小可用版本定义

### MVP 定义

MVP 指 **Local Web Control**，不是公网 SaaS。

必须具备：

1. `OrangeCoding serve --bind 127.0.0.1:PORT`
2. 浏览器打开会话页面
3. 创建会话
4. 收发消息
5. 流式输出
6. 工具调用展示
7. 高危工具审批
8. 任务取消
9. 会话历史恢复

不要求：

1. OIDC
2. 多租户
3. 远程 Worker 集群

---

## 20. 交付顺序建议

建议顺序：

1. `orangecoding-control-protocol`
2. `orangecoding-control-server` 本地模式
3. `orangecoding-worker` 本地嵌入模式
4. 审批桥改造
5. UI
6. 会话回放
7. 远程 Worker 链路
8. 公网认证与 RBAC

---

## 21. 最终建议

建议立项方式：

1. 把本项目拆成 **“本地 Web 控制面”** 与 **“公网控制面”** 两个里程碑
2. 第一个里程碑只解决本机浏览器控制，不提前引入公网复杂度
3. 第二个里程碑再引入 Gateway/Worker 分离、OIDC、RBAC 和 Worker 凭证体系

一句话总结：

**不要把现有 CLI 直接暴露到公网；要在现有 Agent Runtime 之上增加一个受控的 Gateway + Worker 控制平面。**
