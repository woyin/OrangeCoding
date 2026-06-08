# 系统不变量定义

> 本文档定义 OrangeCoding 系统必须始终满足的行为约束（不变量）。
> 任何代码变更都不得违反这些不变量，违反将触发自动回滚。

---

## 不变量索引

| ID | 类别 | 名称 | 严重性 |
|----|------|------|--------|
| INV-AUTH-01 | Auth | WebSocket 连接必须鉴权 | Critical |
| INV-AUTH-02 | Auth | HTTP API 必须通过认证中间件 | Critical |
| INV-AUTH-03 | Auth | Token 不得出现在日志中 | Critical |
| INV-CANCEL-01 | Cancellation | 取消信号必须向下传播 | High |
| INV-CANCEL-02 | Cancellation | 取消后必须可重置 | High |
| INV-SESSION-01 | Session | 会话上下文必须跨 turn 持久化 | High |
| INV-SESSION-02 | Session | 关闭的会话不可继续使用 | High |
| INV-SESSION-03 | Session | 会话 ID 必须全局唯一 | Medium |
| INV-TOOL-01 | Tool Permission | 高危工具执行前必须权限检查 | Critical |
| INV-TOOL-02 | Tool Permission | Deny 决策必须阻止执行 | Critical |
| INV-TOOL-03 | Tool Permission | 输入验证必须在执行前完成 | High |
| INV-CTX-01 | Context | 压缩后系统提示不得丢失 | High |
| INV-CTX-02 | Context | Token 预算不得为负 | Medium |
| INV-AUDIT-01 | Audit | 高危操作必须有审计记录 | High |
| INV-AUDIT-02 | Audit | 审计链哈希必须连续 | Medium |
| INV-APPROVAL-01 | Approval | 审批请求必须可等待 | High |
| INV-APPROVAL-02 | Approval | 审批结果必须送达请求方 | High |
| INV-EVENT-01 | Event | 事件序列必须保持时间顺序 | Medium |

---

## INV-AUTH-01: WebSocket 连接必须鉴权

### 行为规则
每个 WebSocket 升级请求必须在 HTTP 握手阶段完成 token 验证。
未携带 token 或 token 无效的连接必须返回 `401 Unauthorized`，
**不得**升级为 WebSocket 连接。

### 违反示例
```rust
// 错误：跳过鉴权直接升级
pub async fn ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket) // 缺少 token 检查
}
```

### 验证方式
1. 单元测试：无 token → 返回 Err
2. 单元测试：错误 token → 返回 Err
3. 单元测试：空字符串 token → 返回 Err
4. 单元测试：合法 token → 返回 Ok
5. 集成测试：无 token 的 HTTP 请求 → 401 状态码

### 严重性
**Critical** — 违反将导致未经授权的远程控制。

---

## INV-AUTH-02: HTTP API 必须通过认证中间件

### 行为规则
除 `/health` 端点外，所有 HTTP API 端点必须要求
`Authorization: Bearer <token>` 头，且 token 必须通过 `LocalAuth::validate()` 验证。
缺少或无效的 Authorization 头必须返回 `401 Unauthorized`。

### 违反示例
```rust
// 错误：路由未包含 auth 中间件层
Router::new()
    .route("/api/v1/sessions", get(list_sessions))
    // 缺少 .layer(auth_layer)
```

### 验证方式
1. 集成测试：无 Authorization 头 → 401
2. 集成测试：错误 token → 401
3. 集成测试：合法 token → 200
4. 集成测试：/health 端点无需 token → 200

### 严重性
**Critical** — 违反将导致未授权的 API 访问。

---

## INV-AUTH-03: Token 不得出现在日志中

### 行为规则
认证 token、API 密钥等敏感凭证不得以明文形式
出现在日志输出（tracing、println、log 宏）中。
日志中如需引用 token，必须使用脱敏格式（如 `"tok_****1234"`）。

### 违反示例
```rust
// 错误：token 明文出现在日志中
tracing::info!("Auth token: {}", token);
```

### 验证方式
1. 静态检查：grep 日志宏中是否包含 token/secret/key 变量
2. 审计测试：AuditLogger 输出不含明文 token

### 严重性
**Critical** — 违反将导致凭证泄露。

---

## INV-CANCEL-01: 取消信号必须向下传播

### 行为规则
当 `SessionSupervisor::cancel_task()` 被调用时，
对应会话的 `CancellationToken` 必须被标记为 `cancelled`，
且所有通过 `token.child()` 创建的子 token 也必须被取消。
AgentLoop 运行中必须检查 cancel_token 状态。

### 违反示例
```rust
// 错误：每次 turn 创建新 token，忽略 supervisor 的 token
fn run_agent_turn(&self, session_id: String, msg: String) -> bool {
    let new_token = CancellationToken::new(); // 脱离了 supervisor 管理
    // ...
}
```

### 验证方式
1. 单元测试：cancel_task 后 token.is_cancelled() == true
2. 单元测试：父 token 取消 → 子 token 也被取消
3. 集成测试：取消正在运行的会话 → AgentLoop 停止

### 严重性
**High** — 违反将导致任务无法取消，资源泄漏。

---

## INV-CANCEL-02: 取消后必须可重置

### 行为规则
取消任务后，调用 `reset_cancel_token()` 必须将会话的
CancellationToken 替换为全新的未取消 token，使得后续任务可以正常运行。

### 违反示例
```rust
// 错误：复用已取消的 token
pub fn reset_cancel_token(&self, id: &str) -> bool {
    // 不做任何操作，旧 token 仍然是 cancelled 状态
    true
}
```

### 验证方式
1. 单元测试：cancel → reset → 新 token 未取消
2. 单元测试：reset 不存在的 session → false

### 严重性
**High** — 违反将导致会话在取消后无法恢复使用。

---

## INV-SESSION-01: 会话上下文必须跨 turn 持久化

### 行为规则
同一个 session_id 的多次 `UserMessage` 调用必须共享相同的
`AgentContext`（包括对话历史）。每个 turn 不得创建全新的 context。

### 违反示例
```rust
// 错误：每次创建新 context
fn run_agent_turn(&self, session_id: String, msg: String) {
    let ctx = AgentContext::new(SessionId::new(), PathBuf::from(".")); // 新 ID!
}
```

### 验证方式
1. 单元测试：两次 turn 后 context 包含两条用户消息
2. 集成测试：发送 "hello" 然后 "world" → Agent 能看到完整上下文

### 严重性
**High** — 违反将导致 Agent 丧失对话连续性。

---

## INV-SESSION-02: 关闭的会话不可继续使用

### 行为规则
调用 `close_session()` 后，该 session_id 必须不可再用于：
发送消息、获取信息、取消任务。相关操作必须返回错误或 false。

### 违反示例
```rust
// 错误：关闭后仍然可以发消息
fn send_message(&self, session_id: &str, msg: &str) -> bool {
    // 不检查会话是否已关闭
    true
}
```

### 验证方式
1. 单元测试：close → get_session → None
2. 单元测试：close → cancel_task → false
3. 单元测试：close → update_state → false

### 严重性
**High** — 违反可能导致资源泄漏或状态不一致。

---

## INV-SESSION-03: 会话 ID 必须全局唯一

### 行为规则
每次 `create_session()` 生成的 session ID 必须是 UUID v4，
不同会话的 ID 不得重复。

### 违反示例
```rust
// 错误：使用自增 ID
static COUNTER: AtomicU64 = AtomicU64::new(0);
fn create_session() -> String {
    format!("sess_{}", COUNTER.fetch_add(1, Ordering::SeqCst))
}
```

### 验证方式
1. 单元测试：创建 1000 个会话 → 所有 ID 唯一
2. 单元测试：ID 格式符合 UUID v4 规范

### 严重性
**Medium** — 重复 ID 将导致会话混淆。

---

## INV-TOOL-01: 高危工具执行前必须权限检查

### 行为规则
所有标记为 `is_destructive: true` 的工具，在 `execute()` 调用前
必须先调用 `check_permissions()`。权限检查不得被绕过。

### 违反示例
```rust
// 错误：直接调用 execute，跳过权限检查
let result = tool.execute(params).await;
```

### 验证方式
1. 单元测试：destructive 工具 → 执行前 check_permissions 被调用
2. 单元测试：ToolExecutor 流程验证（validate → permissions → execute）
3. 审计测试：高危工具调用有审计记录

### 严重性
**Critical** — 违反将导致危险操作无审批执行。

---

## INV-TOOL-02: Deny 决策必须阻止执行

### 行为规则
当 `check_permissions()` 返回 `PermissionDecision::Deny(reason)` 时，
工具的 `execute()` 方法**不得被调用**。必须返回包含拒绝原因的错误。

### 违反示例
```rust
// 错误：忽略 Deny 决策
match tool.check_permissions(&params, &ctx) {
    PermissionDecision::Deny(_) => { /* 继续执行 */ }
    _ => {}
}
let result = tool.execute(params).await; // 即使 Deny 也执行
```

### 验证方式
1. 单元测试：Deny → execute 不被调用
2. 单元测试：Deny → 返回 SecurityViolation 错误
3. 集成测试：被 deny 的路径不产生文件系统变更

### 严重性
**Critical** — 违反将使权限系统失效。

---

## INV-TOOL-03: 输入验证必须在执行前完成

### 行为规则
`validate_input()` 必须在 `execute()` 之前被调用。
如果 validate_input 返回错误，execute 不得被调用。

### 违反示例
```rust
// 错误：先执行后验证
let result = tool.execute(params.clone()).await;
let _ = tool.validate_input(&params); // 太迟了
```

### 验证方式
1. 单元测试：无效输入 → validate_input 返回 Err → execute 不被调用
2. 单元测试：有效输入 → validate_input Ok → execute 被调用

### 严重性
**High** — 违反可能导致工具接收畸形输入。

---

## INV-CTX-01: 压缩后系统提示不得丢失

### 行为规则
Context 压缩（MicroCompact / AutoCompact）操作后，
系统提示（`Role::System` 消息）必须保留在压缩后的消息列表中。

### 违反示例
```rust
// 错误：压缩时移除了系统提示
fn compact(messages: &[Message]) -> Vec<Message> {
    messages.iter().skip(1).cloned().collect() // 跳过第一条系统提示
}
```

### 验证方式
1. 单元测试：压缩后第一条消息 role == System
2. 单元测试：多轮压缩后系统提示内容不变

### 严重性
**High** — 违反将导致 Agent 丧失行为指令。

---

## INV-CTX-02: Token 预算不得为负

### 行为规则
`TokenBudget` 的 `remaining` 值在任何操作后不得变为负数。
如果扣减操作会导致负数，应返回错误或截止到零。

### 违反示例
```rust
// 错误：允许负数
fn deduct(&mut self, tokens: u64) {
    self.remaining -= tokens as i64; // 可能为负
}
```

### 验证方式
1. 单元测试：扣减超过剩余 → 返回错误或 remaining == 0
2. 单元测试：正常扣减 → remaining 正确减少

### 严重性
**Medium** — 负预算可能导致无限压缩循环。

---

## INV-AUDIT-01: 高危操作必须有审计记录

### 行为规则
以下操作必须在 `AuditLogger` 中记录审计条目：
- bash 工具执行
- 文件编辑/删除操作
- 外部网络请求
- 审批决策（approve/deny）
- 会话创建/关闭
- 认证成功/失败

### 违反示例
```rust
// 错误：执行 bash 但不记录审计
async fn execute_bash(cmd: &str) -> Result<String> {
    Command::new("sh").arg("-c").arg(cmd).output().await // 无审计
}
```

### 验证方式
1. 集成测试：执行高危工具 → 审计日志包含对应条目
2. 集成测试：审批操作 → 审计日志包含决策记录

### 严重性
**High** — 违反将导致操作不可追溯。

---

## INV-AUDIT-02: 审计链哈希必须连续

### 行为规则
审计日志中每条记录的 `hash` 字段必须基于 `previous_hash` 计算。
中间不得有断链。篡改任何一条记录会导致后续哈希不匹配。

### 违反示例
```rust
// 错误：hash 不依赖前一条记录
fn create_entry(action: &str) -> AuditEntry {
    AuditEntry {
        hash: sha256(action),           // 不包含 previous_hash
        previous_hash: String::new(),   // 空值
    }
}
```

### 验证方式
1. 单元测试：连续写入 3 条 → 每条的 previous_hash == 前一条的 hash
2. 单元测试：修改中间记录 → 链验证失败

### 严重性
**Medium** — 违反将使审计日志无法验证完整性。

---

## INV-APPROVAL-01: 审批请求必须可等待

### 行为规则
调用 `request_approval()` 必须返回一个可 await 的 Receiver，
该 Receiver 在对应的 `resolve()` 被调用后产出 `ApprovalDecision`。
在 resolve 之前，await 必须阻塞。

### 违反示例
```rust
// 错误：立即返回结果，不等待
pub async fn request_approval(...) -> ApprovalDecision {
    ApprovalDecision::Approved // 自动批准，跳过人工审批
}
```

### 验证方式
1. 单元测试：request → resolve(Approved) → receiver 收到 Approved
2. 单元测试：request → resolve(Denied) → receiver 收到 Denied
3. 单元测试：多个并发请求 → 各自独立等待和解决

### 严重性
**High** — 违反将导致审批流程被绕过。

---

## INV-APPROVAL-02: 审批结果必须送达请求方

### 行为规则
`resolve()` 调用后，通过 `oneshot::channel` 发送的决策
必须被等待该审批的 Receiver 端收到。如果 Receiver 已 drop，
resolve 应返回 false 表示送达失败。

### 违反示例
```rust
// 错误：resolve 不通过 channel 发送
pub fn resolve(&self, id: &str, decision: ApprovalDecision) -> bool {
    self.pending.remove(id); // 移除但不发送
    true // 错误地报告成功
}
```

### 验证方式
1. 单元测试：resolve → receiver 收到值
2. 单元测试：receiver 先 drop → resolve 返回 false

### 严重性
**High** — 违反将导致工具调用永久挂起。

---

## INV-EVENT-01: 事件序列必须保持时间顺序

### 行为规则
通过 `WorkerRuntime::publish_event()` 发布的事件，
在每个 subscriber 的接收端必须保持发布顺序。
事件的 `timestamp` 字段必须单调递增。

### 违反示例
```rust
// 错误：乱序发布
runtime.publish_event(event_b); // 时间戳 T+2
runtime.publish_event(event_a); // 时间戳 T+1 但后发
```

### 验证方式
1. 单元测试：发布 A, B, C → 接收顺序为 A, B, C
2. 单元测试：多个 subscriber 接收到相同顺序

### 严重性
**Medium** — 乱序事件导致 UI 显示错误和回放失败。

---

## 应用指南

### 新代码提交前检查清单

1. ☐ 是否影响认证路径？→ 检查 INV-AUTH-*
2. ☐ 是否涉及取消逻辑？→ 检查 INV-CANCEL-*
3. ☐ 是否修改会话管理？→ 检查 INV-SESSION-*
4. ☐ 是否修改工具执行链？→ 检查 INV-TOOL-*
5. ☐ 是否涉及上下文压缩？→ 检查 INV-CTX-*
6. ☐ 是否涉及审计日志？→ 检查 INV-AUDIT-*
7. ☐ 是否涉及审批流程？→ 检查 INV-APPROVAL-*
8. ☐ 是否涉及事件发布？→ 检查 INV-EVENT-*
