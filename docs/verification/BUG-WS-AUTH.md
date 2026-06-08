# BUG-WS-AUTH 验证报告：WebSocket 控制通道鉴权

Result: **PASS**

## 修复是否完整
✅ 完整。`ws_handler` 在 WebSocket 升级之前执行 `check_ws_auth`，未认证请求返回 401，已认证请求正常升级。

## 是否存在遗漏路径
✅ 无遗漏。检查了所有 WebSocket 入口：
- `ws_handler` — **已覆盖**（新增鉴权检查）
- `routes.rs` 中 WS 路由注册 — **已覆盖**（使用 `WsState` 包含 `LocalAuth`）
- `build_router` 中 HTTP API 路由 — 不受影响（已有 `auth_middleware`）

## 修改文件
1. `crates/orangecoding-control-server/src/ws.rs` — 新增 `check_ws_auth` 函数、`WsState` 结构体
2. `crates/orangecoding-control-server/src/routes.rs` — 使用 `WsState` 替代裸 `Arc<WorkerRuntime>`

## 测试覆盖
- `test_ws_auth_no_token_rejected` — 无 token 被拒绝
- `test_ws_auth_wrong_token_rejected` — 错误 token 被拒绝
- `test_ws_auth_valid_token_accepted` — 正确 token 通过
- `test_ws_auth_empty_token_rejected` — 空字符串 token 被拒绝
- `test_ws_state_clonable` — WsState 可 Clone
- 全量测试 0 失败
