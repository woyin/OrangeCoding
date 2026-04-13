use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use chengcoding_control_protocol::{ClientCommand, ErrorCode, ServerEvent, SessionState};
use chengcoding_worker::WorkerRuntime;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;

use crate::auth::LocalAuth;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

#[derive(Clone)]
pub struct WsState {
    pub runtime: Arc<WorkerRuntime>,
    pub auth: LocalAuth,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<WsState>,
) -> axum::response::Response {
    // 鉴权必须在 WebSocket 升级之前完成，防止未认证连接进入 handle_socket
    if check_ws_auth(&query, &state.auth).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state.runtime))
        .into_response()
}

async fn handle_socket(socket: WebSocket, runtime: Arc<WorkerRuntime>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = runtime.subscribe_events();

    // Forward server events to the WebSocket client
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to serialize event: {}", e);
                }
            }
        }
    });

    // Receive client commands from the WebSocket
    let recv_runtime = runtime.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    handle_client_message(&recv_runtime, &text);
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // When one task finishes (e.g. client disconnect), abort the other so
    // the spawned task does not keep running after the socket is gone.
    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
        },
        _ = &mut recv_task => {
            send_task.abort();
        },
    }
}

fn handle_client_message(runtime: &WorkerRuntime, text: &str) {
    let command: ClientCommand = match serde_json::from_str(text) {
        Ok(cmd) => cmd,
        Err(e) => {
            let event = ServerEvent::Error {
                session_id: None,
                code: ErrorCode::InvalidCommand.as_str().to_string(),
                message: format!("Failed to parse command: {}", e),
                timestamp: chrono::Utc::now(),
            };
            runtime.publish_event(event);
            return;
        }
    };

    match command {
        ClientCommand::Ping { .. } => {
            runtime.publish_event(ServerEvent::Pong {
                timestamp: chrono::Utc::now(),
            });
        }
        ClientCommand::SessionCreate {
            title,
            working_directory,
            ..
        } => {
            let info = runtime.sessions.create_session(title, working_directory);
            runtime.publish_event(ServerEvent::SessionCreated {
                session_id: info.id.clone(),
                title: info.title.clone(),
                timestamp: chrono::Utc::now(),
            });
        }
        ClientCommand::TaskCancel { session_id, .. } => {
            runtime.sessions.cancel_task(&session_id);
            runtime.publish_event(ServerEvent::AgentStatus {
                session_id,
                status: "cancelled".to_string(),
                message: None,
                timestamp: chrono::Utc::now(),
            });
        }
        ClientCommand::ApprovalRespond {
            approval_id,
            decision,
            ..
        } => {
            runtime.approval.resolve(&approval_id, decision.clone());
            runtime.publish_event(ServerEvent::ApprovalResolved {
                session_id: String::new(),
                approval_id,
                decision,
                timestamp: chrono::Utc::now(),
            });
        }
        ClientCommand::SessionClose { session_id, .. } => {
            runtime.sessions.close_session(&session_id);
        }
        ClientCommand::UserMessage {
            session_id,
            content,
            ..
        } => {
            runtime
                .sessions
                .update_state(&session_id, SessionState::Running);
            runtime.publish_event(ServerEvent::AgentStatus {
                session_id: session_id.clone(),
                status: "running".to_string(),
                message: Some(content.clone()),
                timestamp: chrono::Utc::now(),
            });
            if !runtime.run_agent_turn(session_id, content) {
                tracing::warn!("No agent executor configured; message not processed");
            }
        }
        ClientCommand::SessionAttach { session_id, .. } => {
            if let Some(info) = runtime.sessions.get_session(&session_id) {
                runtime.publish_event(ServerEvent::SessionSnapshot {
                    session_id: info.id.clone(),
                    info,
                    history: vec![],
                    timestamp: chrono::Utc::now(),
                });
            }
        }
        ClientCommand::SessionRename {
            session_id, title, ..
        } => {
            tracing::info!("Session rename requested: {} -> {}", session_id, title);
        }
    }
}

/// 鉴权检查逻辑，独立于 WebSocket 提取器以便测试。
/// 返回 Ok(()) 表示鉴权通过，Err 表示应返回 401。
fn check_ws_auth(query: &WsQuery, auth: &LocalAuth) -> Result<(), ()> {
    match &query.token {
        Some(token) if auth.validate(token) => Ok(()),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_auth() -> LocalAuth {
        LocalAuth::from_token("test-token-123".to_string())
    }

    // ── 鉴权逻辑单元测试 ──

    #[test]
    fn test_ws_auth_no_token_rejected() {
        // 无 token → 鉴权失败
        let auth = make_auth();
        let query = WsQuery { token: None };
        assert!(check_ws_auth(&query, &auth).is_err(), "无 token 应被拒绝");
    }

    #[test]
    fn test_ws_auth_wrong_token_rejected() {
        // 非法 token → 鉴权失败
        let auth = make_auth();
        let query = WsQuery {
            token: Some("wrong-token".into()),
        };
        assert!(check_ws_auth(&query, &auth).is_err(), "非法 token 应被拒绝");
    }

    #[test]
    fn test_ws_auth_valid_token_accepted() {
        // 合法 token → 鉴权通过
        let auth = make_auth();
        let query = WsQuery {
            token: Some("test-token-123".into()),
        };
        assert!(
            check_ws_auth(&query, &auth).is_ok(),
            "合法 token 应通过鉴权"
        );
    }

    #[test]
    fn test_ws_auth_empty_token_rejected() {
        // 空字符串 token → 鉴权失败
        let auth = make_auth();
        let query = WsQuery {
            token: Some(String::new()),
        };
        assert!(check_ws_auth(&query, &auth).is_err(), "空 token 应被拒绝");
    }

    // ── WsState 构造测试 ──

    #[test]
    fn test_ws_state_clonable() {
        // WsState 必须实现 Clone（axum State 要求）
        let runtime = Arc::new(WorkerRuntime::new());
        let auth = make_auth();
        let state = WsState {
            runtime,
            auth: auth.clone(),
        };
        let _cloned = state.clone();
    }
}
