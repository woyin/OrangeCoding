# P1 级 BUG 分析报告

> 生成时间：2026-04-11
> 状态：阶段 1 — 问题建模（禁止修改代码）

---

## BUG-WS-AUTH：WebSocket 控制通道未鉴权

优先级：**P1**

问题本质：
控制面的安全边界存在缺口。HTTP API 路由通过 `auth_middleware` 强制 Bearer token 验证，但 WebSocket 路由（`/api/v1/ws`）直接注册在 Router 根层级，绕过了 `authed_api` 的 middleware 层。这意味着 WebSocket 连接根本不经过任何认证逻辑，任何人都可以连接并发送 `UserMessage`、`TaskCancel` 等控制命令。

攻击路径 / 失败路径：
1. 攻击者打开浏览器控制台
2. 执行 `new WebSocket("ws://127.0.0.1:3200/api/v1/ws")`
3. 连接立即成功（无 auth 握手）
4. 发送 `{"type":"user_message","session_id":"...","content":"rm -rf /"}` 
5. 服务端直接执行——`ws.rs:128-144` 的 `UserMessage` handler 调用 `runtime.run_agent_turn()`

根因（代码层面）：
`routes.rs:40-48` 中的路由构建：

```rust
Router::new()
    .nest("/api/v1", authed_api)         // ← HTTP API 走 auth middleware
    .route("/api/v1/ws",                  // ← WebSocket 不走 authed_api
        get(ws::ws_handler).with_state(runtime.clone()),
    )
```

WebSocket 路由直接挂在根 Router 上，而非嵌套在 `authed_api` 内。`ws_handler` 接收 `Query(_query): Query<WsQuery>` 但完全忽略 `token` 字段（`_query` 前缀 `_` 表示未使用）。`handle_socket` 无任何鉴权检查。

影响范围：
- `crates/orangecoding-control-server/src/routes.rs` — 路由配置
- `crates/orangecoding-control-server/src/ws.rs` — WebSocket handler
- `crates/orangecoding-control-server/src/auth.rs` — LocalAuth（已存在但未用于 WS）
- 所有通过 WebSocket 传递的命令：`UserMessage`, `TaskCancel`, `ApprovalRespond`, `SessionCreate`, `SessionClose`

修复策略：
1. **在 `ws_handler` 中强制验证 token**：检查 `WsQuery.token` 是否存在且 `auth.validate()` 通过
2. **无 token 或非法 token → 拒绝 WebSocket 升级**（返回 401，不建立连接）
3. **将 `LocalAuth` 通过 State 传递给 ws_handler**（需要修改路由注册和 State 类型）
4. 可选：将 WS 路由也纳入 middleware 层（但 WebSocket upgrade 与 axum middleware 有兼容性问题，直接在 handler 内验证更可靠）

验证方法：
1. 测试无 token 连接 → 连接被拒（HTTP 401）
2. 测试非法 token 连接 → 连接被拒
3. 测试合法 token 连接 → 正常建立 WebSocket
4. 模拟浏览器直接连接 ws://127.0.0.1:3200/api/v1/ws → 确认无法发送 user_message
5. 确认 HTTP API 仍正常工作（回归测试）

回归风险：
- 前端客户端需要传递 token（可能需要更新 WS 连接逻辑）
- 如果 token 通过 query param 传递，存在 server log 泄露风险（应考虑通过 header 或 sub-protocol 传递）

---

## BUG-CANCEL：任务取消不传播到 AgentLoop

优先级：**P1**

问题本质：
取消信号在"控制面→会话管理"层面被正确触发，但没有传播到实际执行层。`SessionSupervisor.cancel_task()` 取消的是会话级的 `CancellationToken`，但 `LocalAgentExecutor::execute_turn()` 每次都创建一个**全新的** `CancellationToken`，与 session 的 token 无关。因此取消操作只改变了 UI 状态（`AgentStatus: cancelled`），AgentLoop 继续运行直到自然完成。

攻击路径 / 失败路径：
1. 用户通过 WS 或 HTTP API 创建 session，发送长任务
2. 用户发送 `TaskCancel { session_id }`
3. `ws.rs:103-111` → `runtime.sessions.cancel_task(&session_id)` 取消 session 的 token
4. `ws.rs:105-110` → 发布 `AgentStatus { status: "cancelled" }`（UI 显示已取消）
5. 但 `serve.rs:73` 的 `execute_turn` 中 `let cancel_token = CancellationToken::new()` 是独立的新 token
6. AgentLoop 继续运行——它监听的是新 token，不是 session 的 token

根因（代码层面）：
`serve.rs:48-78` 的 `LocalAgentExecutor::execute_turn()`：

```rust
async fn execute_turn(&self, session_id: String, ...) -> Result<(), String> {
    let sid = SessionId::new();                          // ← 新 SessionId，忽略了传入的 session_id
    let mut context = AgentContext::new(sid, working_dir); // ← 新上下文，无历史
    let cancel_token = CancellationToken::new();          // ← 新 token，不关联 session token
    agent_loop.run(&self.chat_options, cancel_token, event_tx).await
}
```

三个关键错误：
1. `CancellationToken::new()` 与 session 的 cancel_token 无关
2. `SessionId::new()` 忽略了传入的 `session_id`（这也导致了 BUG-SESSION-STATE）
3. `AgentContext::new(sid, working_dir)` 每次创建全新上下文

影响范围：
- `crates/orangecoding-cli/src/commands/serve.rs` — LocalAgentExecutor
- `crates/orangecoding-worker/src/session_bridge.rs` — SessionSupervisor（token 管理正确）
- `crates/orangecoding-agent/src/agent_loop.rs` — AgentLoop::run（已支持 CancellationToken）
- `crates/orangecoding-worker/src/runtime.rs` — AgentExecutor trait

修复策略：
1. **`AgentExecutor::execute_turn` 签名增加 `CancellationToken` 参数**：将 session 的 token 传入
2. **`runtime.rs::run_agent_turn` 传递 session 的 cancel_token**：`sessions.get_cancel_token(&session_id)`
3. **`LocalAgentExecutor::execute_turn` 使用传入的 token** 而非 `CancellationToken::new()`
4. 确保 `AgentLoop::run` 中的所有执行点（AI 调用、工具执行）都检查该 token

验证方法：
1. 启动长任务（sleep 10s 的 bash 工具）
2. 调用 cancel API
3. 断言 AgentLoop 在 <2s 内退出（而非等待 10s）
4. 断言无后续工具调用
5. 断言 event channel 收到 cancel 事件

回归风险：
- `AgentExecutor` trait 签名变更影响所有实现者
- 需要确保 token reset（`reset_cancel_token`）在 cancel 后正确调用，否则后续 turn 会立即被取消

---

## BUG-SESSION-STATE：多轮对话会话状态丢失

优先级：**P1**

问题本质：
`LocalAgentExecutor::execute_turn()` 每次被调用时都创建全新的 `SessionId` 和 `AgentContext`，完全忽略传入的 `session_id` 参数。这意味着：
- 第一轮：创建 session A，设置上下文，执行任务
- 第二轮：传入 session A 的 ID，但代码创建全新的 session B + 空 context
- 结果：上下文、对话历史、工具结果、working_directory 全部丢失

根因（代码层面）：
`serve.rs:48-52`：

```rust
async fn execute_turn(&self, session_id: String, ...) -> Result<(), String> {
    let sid = SessionId::new();                              // ← BUG：忽略传入的 session_id
    let working_dir = std::env::current_dir()...;            // ← BUG：不恢复 session 的工作目录
    let mut context = AgentContext::new(sid, working_dir);   // ← BUG：新建空 context
    context.add_user_message(&user_message);
```

应改为：
```rust
let sid = SessionId::from(session_id.clone());              // 使用传入的 session_id
// 从 session 存储中恢复 AgentContext（或从持久化中加载）
```

此外，当前没有 session 级别的 `AgentContext` 持久化机制。`SessionSupervisor` 只存 `SessionInfo`（元数据），不存对话历史和上下文。

影响范围：
- `crates/orangecoding-cli/src/commands/serve.rs` — LocalAgentExecutor（根因）
- `crates/orangecoding-agent/src/context.rs` — AgentContext
- `crates/orangecoding-worker/src/session_bridge.rs` — SessionSupervisor（需扩展存储 context）
- 所有依赖多轮对话的功能：follow-up 消息、工具结果引用、上下文连续性

修复策略：
1. **`AgentContext` 按 session_id 存储**：在 `WorkerRuntime` 或 `SessionSupervisor` 中维护 `HashMap<SessionId, AgentContext>`
2. **`execute_turn` 从存储中恢复 context**：如果有现有 context，恢复它并追加新消息
3. **使用传入的 `session_id`**：不再 `SessionId::new()`
4. **保存 context 到 session 存储中**：每次 turn 结束后更新

验证方法：
1. 创建 session，发送 "请记住我的名字是 Alice"
2. 发送 follow-up "我叫什么名字？"
3. 断言 context 包含第一轮的对话历史
4. 断言 working_directory 正确
5. 断言 session_id 一致

回归风险：
- context 存储需要线程安全（DashMap 或 Mutex）
- context 可能增长过大（需要清理/压缩策略）
- 并发 turn 对同一 session 的 context 竞争

---

## 依赖关系

```
BUG-SESSION-STATE ← 基础设施（session context 存储）
    ↑
BUG-CANCEL ← 依赖 session context 存储（需要从 session 获取 cancel_token）
    ↑
BUG-WS-AUTH ← 独立（路由层问题）
```

建议修复顺序：
1. **BUG-WS-AUTH**（独立，最简单）
2. **BUG-SESSION-STATE**（基础设施）
3. **BUG-CANCEL**（依赖 SESSION-STATE 的 context 存储）
