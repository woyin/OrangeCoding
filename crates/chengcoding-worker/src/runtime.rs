use std::sync::Arc;

use chengcoding_control_protocol::{ServerEvent, SessionState};
use chengcoding_core::AgentEvent;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::approval_bridge::ApprovalBridge;
use crate::event_bridge::agent_event_to_server_event;
use crate::session_bridge::SessionSupervisor;

const BROADCAST_CAPACITY: usize = 1024;

/// Trait for executing agent turns. Implemented by the CLI layer which
/// has access to `ceair-agent`, `ceair-ai`, `ceair-tools`, etc.
#[async_trait::async_trait]
pub trait AgentExecutor: Send + Sync + 'static {
    /// Run an agent turn for the given session.
    ///
    /// Implementations should:
    /// 1. Create/reuse an `AgentLoop` for the session
    /// 2. Pipe `AgentEvent`s into the provided `event_tx`
    /// 3. Respect the provided `cancel_token` for task cancellation
    async fn execute_turn(
        &self,
        session_id: String,
        user_message: String,
        event_tx: mpsc::Sender<AgentEvent>,
        cancel_token: CancellationToken,
    ) -> Result<(), String>;
}

pub struct WorkerRuntime {
    pub sessions: Arc<SessionSupervisor>,
    pub approval: Arc<ApprovalBridge>,
    event_tx: broadcast::Sender<ServerEvent>,
    executor: Option<Arc<dyn AgentExecutor>>,
}

impl WorkerRuntime {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            sessions: Arc::new(SessionSupervisor::new()),
            approval: Arc::new(ApprovalBridge::new()),
            event_tx,
            executor: None,
        }
    }

    /// Create a runtime with an agent executor for real agent integration.
    pub fn with_executor(executor: Arc<dyn AgentExecutor>) -> Self {
        let (event_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            sessions: Arc::new(SessionSupervisor::new()),
            approval: Arc::new(ApprovalBridge::new()),
            event_tx,
            executor: Some(executor),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<ServerEvent> {
        self.event_tx.subscribe()
    }

    pub fn publish_event(&self, event: ServerEvent) {
        if let Err(e) = self.event_tx.send(event) {
            debug!("no active subscribers for event: {}", e);
        }
    }

    /// Run an agent turn for the given session. Spawns the agent loop in a
    /// background task, forwarding events through the broadcast channel.
    /// Returns `false` if no executor is configured.
    pub fn run_agent_turn(&self, session_id: String, user_message: String) -> bool {
        let executor = match &self.executor {
            Some(e) => Arc::clone(e),
            None => {
                warn!("no agent executor configured; ignoring user message");
                return false;
            }
        };

        // 获取会话的取消令牌，以便 executor 响应取消请求
        let cancel_token = self
            .sessions
            .get_cancel_token(&session_id)
            .unwrap_or_else(CancellationToken::new);

        let (agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(256);
        self.spawn_event_forwarder(session_id.clone(), agent_rx);

        let sessions = Arc::clone(&self.sessions);
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            match executor
                .execute_turn(session_id.clone(), user_message, agent_tx, cancel_token)
                .await
            {
                Ok(()) => {
                    sessions.update_state(&session_id, SessionState::Completed);
                }
                Err(e) => {
                    sessions.update_state(&session_id, SessionState::Error);
                    let _ = event_tx.send(ServerEvent::Error {
                        session_id: Some(session_id),
                        code: "agent_error".to_string(),
                        message: e,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
        });

        true
    }

    /// Spawn a background task that reads `AgentEvent`s from `agent_rx`,
    /// converts them to `ServerEvent`s, and publishes them. Also updates the
    /// session state on `Completed` and `Error` events.
    pub fn spawn_event_forwarder(
        &self,
        session_id: String,
        mut agent_rx: mpsc::Receiver<AgentEvent>,
    ) {
        let tx = self.event_tx.clone();
        let sessions = Arc::clone(&self.sessions);
        let sid = session_id.clone();

        tokio::spawn(async move {
            while let Some(event) = agent_rx.recv().await {
                // Update session state for terminal events
                match &event {
                    AgentEvent::Completed { .. } => {
                        sessions.update_state(&sid, SessionState::Completed);
                    }
                    AgentEvent::Error { .. } => {
                        sessions.update_state(&sid, SessionState::Error);
                    }
                    _ => {}
                }

                if let Some(server_event) = agent_event_to_server_event(&event, &sid) {
                    if let Err(e) = tx.send(server_event) {
                        warn!("failed to forward event for session {}: {}", sid, e);
                    }
                }
            }
        });
    }
}

impl Default for WorkerRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn create_session_via_runtime() {
        let rt = WorkerRuntime::new();
        let info = rt.sessions.create_session(Some("test".into()), None);
        assert_eq!(rt.sessions.count(), 1);
        assert_eq!(
            rt.sessions.get_session(&info.id).unwrap().title,
            Some("test".into())
        );
    }

    #[tokio::test]
    async fn event_subscription() {
        let rt = WorkerRuntime::new();
        let mut rx = rt.subscribe_events();

        let pong = ServerEvent::Pong {
            timestamp: Utc::now(),
        };
        rt.publish_event(pong.clone());

        let received = rx.recv().await.unwrap();
        match received {
            ServerEvent::Pong { .. } => {} // ok
            other => panic!("expected Pong, got {:?}", other),
        }
    }
}
