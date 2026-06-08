# Phase A: Local Web Control Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `OrangeCoding serve --bind 127.0.0.1:PORT` to provide browser-based local control of OrangeCoding agents via HTTP + WebSocket.

**Architecture:** Three new crates (`orangecoding-control-protocol`, `orangecoding-control-server`, `orangecoding-worker`) sit atop the existing agent runtime. A `serve` CLI subcommand starts an HTTP/WS server on localhost. The browser connects via WebSocket for bidirectional streaming. The worker wraps `AgentLoop` and converts `AgentEvent` into control-protocol events. Approval is upgraded from synchronous CLI prompts to async suspend/resume over WebSocket.

**Tech Stack:** Rust, axum (HTTP/WS), tokio, serde/serde_json, tokio-tungstenite (via axum), uuid, chrono. Existing orangecoding-core/agent/session/audit/tools crates.

---

## Task 1: Create `orangecoding-control-protocol` Crate — Types & Events

**Files:**
- Create: `crates/orangecoding-control-protocol/Cargo.toml`
- Create: `crates/orangecoding-control-protocol/src/lib.rs`
- Create: `crates/orangecoding-control-protocol/src/event.rs`
- Create: `crates/orangecoding-control-protocol/src/command.rs`
- Create: `crates/orangecoding-control-protocol/src/session.rs`
- Create: `crates/orangecoding-control-protocol/src/approval.rs`
- Create: `crates/orangecoding-control-protocol/src/error.rs`
- Modify: `Cargo.toml` (workspace members)
- Test: `crates/orangecoding-control-protocol/src/lib.rs` (inline tests)

This crate defines the shared protocol types used between browser, server, and worker. No I/O, no async — pure data types with serde.

### Step 1: Write failing test for protocol message serialization

Create `crates/orangecoding-control-protocol/Cargo.toml`:

```toml
[package]
name = "orangecoding-control-protocol"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
thiserror = { workspace = true }
orangecoding-core = { workspace = true }
```

Create `crates/orangecoding-control-protocol/src/lib.rs`:

```rust
pub mod approval;
pub mod command;
pub mod error;
pub mod event;
pub mod session;

pub use approval::*;
pub use command::*;
pub use error::*;
pub use event::*;
pub use session::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_command_roundtrip() {
        let cmd = ClientCommand::UserMessage {
            request_id: "req_001".into(),
            session_id: "sess_001".into(),
            content: "Hello agent".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: ClientCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ClientCommand::UserMessage { .. }));
    }

    #[test]
    fn test_server_event_roundtrip() {
        let evt = ServerEvent::AssistantDelta {
            session_id: "sess_001".into(),
            content: "Hello".into(),
            timestamp: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let decoded: ServerEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, ServerEvent::AssistantDelta { .. }));
    }

    #[test]
    fn test_approval_request_roundtrip() {
        let req = ApprovalRequest {
            id: "apr_001".into(),
            session_id: "sess_001".into(),
            tool_name: "bash".into(),
            risk_level: RiskLevel::High,
            summary: "Run cargo test".into(),
            arguments: serde_json::json!({"cmd": "cargo test"}),
            expires_at: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: ApprovalRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "apr_001");
        assert!(matches!(decoded.risk_level, RiskLevel::High));
    }

    #[test]
    fn test_session_info_roundtrip() {
        let info = SessionInfo {
            id: "sess_001".into(),
            title: Some("Test session".into()),
            state: SessionState::Running,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "sess_001");
        assert!(matches!(decoded.state, SessionState::Running));
    }
}
```

### Step 2: Implement `event.rs` — Server-to-browser events

```rust
// crates/orangecoding-control-protocol/src/event.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events sent from the server/worker to the browser client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// Session successfully created
    SessionCreated {
        session_id: String,
        title: Option<String>,
        timestamp: DateTime<Utc>,
    },

    /// Full session state snapshot (sent on attach/reconnect)
    SessionSnapshot {
        session_id: String,
        info: super::SessionInfo,
        history: Vec<HistoryEntry>,
        timestamp: DateTime<Utc>,
    },

    /// Streaming token from assistant
    AssistantDelta {
        session_id: String,
        content: String,
        timestamp: DateTime<Utc>,
    },

    /// Assistant response complete
    AssistantDone {
        session_id: String,
        full_content: String,
        timestamp: DateTime<Utc>,
    },

    /// Tool call execution started
    ToolCallStarted {
        session_id: String,
        tool_call_id: String,
        tool_name: String,
        arguments_preview: serde_json::Value,
        timestamp: DateTime<Utc>,
    },

    /// Tool call execution completed
    ToolCallCompleted {
        session_id: String,
        tool_call_id: String,
        tool_name: String,
        success: bool,
        result_preview: String,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    /// Tool call failed
    ToolCallFailed {
        session_id: String,
        tool_call_id: String,
        tool_name: String,
        error: String,
        timestamp: DateTime<Utc>,
    },

    /// Approval required for a tool call
    ApprovalRequired {
        session_id: String,
        approval: super::ApprovalRequest,
        timestamp: DateTime<Utc>,
    },

    /// Approval was resolved
    ApprovalResolved {
        session_id: String,
        approval_id: String,
        decision: super::ApprovalDecision,
        timestamp: DateTime<Utc>,
    },

    /// Agent status change
    AgentStatus {
        session_id: String,
        status: String,
        message: Option<String>,
        timestamp: DateTime<Utc>,
    },

    /// Token usage update
    UsageDelta {
        session_id: String,
        prompt_tokens: u64,
        completion_tokens: u64,
        total_tokens: u64,
        timestamp: DateTime<Utc>,
    },

    /// Error notification
    Error {
        session_id: Option<String>,
        code: String,
        message: String,
        timestamp: DateTime<Utc>,
    },

    /// Pong response
    Pong {
        timestamp: DateTime<Utc>,
    },
}

/// A historical conversation entry for snapshot replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HistoryEntry {
    UserMessage {
        content: String,
        timestamp: DateTime<Utc>,
    },
    AssistantMessage {
        content: String,
        model: Option<String>,
        timestamp: DateTime<Utc>,
    },
    ToolCall {
        tool_call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
        result: Option<String>,
        success: Option<bool>,
        duration_ms: Option<u64>,
        timestamp: DateTime<Utc>,
    },
}
```

### Step 3: Implement `command.rs` — Browser-to-server commands

```rust
// crates/orangecoding-control-protocol/src/command.rs
use serde::{Deserialize, Serialize};

/// Commands sent from the browser client to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientCommand {
    /// Attach to an existing session
    SessionAttach {
        request_id: String,
        session_id: String,
    },

    /// Create a new session
    SessionCreate {
        request_id: String,
        title: Option<String>,
        working_directory: Option<String>,
    },

    /// Send a user message
    UserMessage {
        request_id: String,
        session_id: String,
        content: String,
    },

    /// Cancel the current task
    TaskCancel {
        request_id: String,
        session_id: String,
    },

    /// Respond to an approval request
    ApprovalRespond {
        request_id: String,
        approval_id: String,
        decision: super::ApprovalDecision,
    },

    /// Rename a session
    SessionRename {
        request_id: String,
        session_id: String,
        title: String,
    },

    /// Close a session
    SessionClose {
        request_id: String,
        session_id: String,
    },

    /// Ping
    Ping {
        request_id: String,
    },
}

impl ClientCommand {
    pub fn request_id(&self) -> &str {
        match self {
            Self::SessionAttach { request_id, .. }
            | Self::SessionCreate { request_id, .. }
            | Self::UserMessage { request_id, .. }
            | Self::TaskCancel { request_id, .. }
            | Self::ApprovalRespond { request_id, .. }
            | Self::SessionRename { request_id, .. }
            | Self::SessionClose { request_id, .. }
            | Self::Ping { request_id, .. } => request_id,
        }
    }

    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::SessionAttach { session_id, .. }
            | Self::UserMessage { session_id, .. }
            | Self::TaskCancel { session_id, .. }
            | Self::SessionRename { session_id, .. }
            | Self::SessionClose { session_id, .. } => Some(session_id),
            Self::SessionCreate { .. }
            | Self::ApprovalRespond { .. }
            | Self::Ping { .. } => None,
        }
    }
}
```

### Step 4: Implement `session.rs` — Session model

```rust
// crates/orangecoding-control-protocol/src/session.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Possible session states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Session created but no agent running
    Idle,
    /// Agent is processing
    Running,
    /// Waiting for approval
    AwaitingApproval,
    /// Session completed
    Completed,
    /// Session errored
    Error,
    /// Session closed by user
    Closed,
}

/// Session metadata visible to the browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: Option<String>,
    pub state: SessionState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Step 5: Implement `approval.rs` — Approval model

```rust
// crates/orangecoding-control-protocol/src/approval.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Risk level for a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// A pending approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub session_id: String,
    pub tool_name: String,
    pub risk_level: RiskLevel,
    pub summary: String,
    pub arguments: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
}

/// User's decision on an approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Denied { reason: Option<String> },
}
```

### Step 6: Implement `error.rs` — Protocol error codes

```rust
// crates/orangecoding-control-protocol/src/error.rs
use serde::{Deserialize, Serialize};

/// Protocol-level error codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    SessionNotFound,
    SessionClosed,
    ApprovalNotFound,
    ApprovalExpired,
    InvalidCommand,
    Unauthorized,
    RateLimited,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionNotFound => "session_not_found",
            Self::SessionClosed => "session_closed",
            Self::ApprovalNotFound => "approval_not_found",
            Self::ApprovalExpired => "approval_expired",
            Self::InvalidCommand => "invalid_command",
            Self::Unauthorized => "unauthorized",
            Self::RateLimited => "rate_limited",
            Self::InternalError => "internal_error",
        }
    }
}
```

### Step 7: Register in workspace and run tests

Add `orangecoding-control-protocol` to workspace `Cargo.toml` members and dependencies.

Run: `cargo test -p orangecoding-control-protocol`
Expected: All 4 tests pass.

### Step 8: Commit

```bash
git add crates/orangecoding-control-protocol/ Cargo.toml Cargo.lock
git commit -m "feat: add orangecoding-control-protocol crate with shared types

Define the browser/server/worker control protocol:
- ClientCommand: browser-to-server commands (attach, create, message, cancel, approve)
- ServerEvent: server-to-browser events (delta, tool calls, approval, usage)
- SessionInfo/SessionState: session metadata
- ApprovalRequest/ApprovalDecision: async approval model
- HistoryEntry: conversation replay
- ErrorCode: protocol error codes"
```

---

## Task 2: Create `orangecoding-worker` Crate — Agent Runtime Adapter

**Files:**
- Create: `crates/orangecoding-worker/Cargo.toml`
- Create: `crates/orangecoding-worker/src/lib.rs`
- Create: `crates/orangecoding-worker/src/runtime.rs`
- Create: `crates/orangecoding-worker/src/session_bridge.rs`
- Create: `crates/orangecoding-worker/src/event_bridge.rs`
- Create: `crates/orangecoding-worker/src/approval_bridge.rs`
- Modify: `Cargo.toml` (workspace members)
- Test: inline unit tests

This crate wraps the existing `AgentLoop` and converts its events into control-protocol events. It manages session lifecycle and provides an async approval bridge.

### Step 1: Write failing test for event bridge

Create `crates/orangecoding-worker/Cargo.toml`:

```toml
[package]
name = "orangecoding-worker"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
orangecoding-core = { workspace = true }
orangecoding-agent = { workspace = true }
orangecoding-tools = { workspace = true }
orangecoding-session = { workspace = true }
orangecoding-audit = { workspace = true }
orangecoding-config = { workspace = true }
orangecoding-control-protocol = { workspace = true }
tokio = { workspace = true }
tokio-util = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
dashmap = { workspace = true }
parking_lot = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros"] }
```

Create `crates/orangecoding-worker/src/event_bridge.rs` with tests:

```rust
//! Converts AgentEvent into ServerEvent for the control protocol.

use orangecoding_control_protocol::ServerEvent;
use orangecoding_core::event::AgentEvent;
use chrono::Utc;

/// Convert an internal AgentEvent to a control-protocol ServerEvent.
pub fn agent_event_to_server_event(event: &AgentEvent, session_id: &str) -> Option<ServerEvent> {
    match event {
        AgentEvent::Started { .. } => Some(ServerEvent::AgentStatus {
            session_id: session_id.to_string(),
            status: "started".into(),
            message: None,
            timestamp: Utc::now(),
        }),

        AgentEvent::StreamChunk { content, .. } => Some(ServerEvent::AssistantDelta {
            session_id: session_id.to_string(),
            content: content.clone(),
            timestamp: Utc::now(),
        }),

        AgentEvent::ToolCallRequested { tool_call, .. } => Some(ServerEvent::ToolCallStarted {
            session_id: session_id.to_string(),
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.function_name.clone(),
            arguments_preview: tool_call.arguments.clone(),
            timestamp: Utc::now(),
        }),

        AgentEvent::ToolCallCompleted {
            tool_name,
            success,
            duration_ms,
            ..
        } => Some(ServerEvent::ToolCallCompleted {
            session_id: session_id.to_string(),
            tool_call_id: String::new(), // Not available in AgentEvent
            tool_name: tool_name.clone(),
            success: *success,
            result_preview: String::new(),
            duration_ms: *duration_ms,
            timestamp: Utc::now(),
        }),

        AgentEvent::TokenUsageUpdated { usage, .. } => Some(ServerEvent::UsageDelta {
            session_id: session_id.to_string(),
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            timestamp: Utc::now(),
        }),

        AgentEvent::Completed { summary, .. } => Some(ServerEvent::AssistantDone {
            session_id: session_id.to_string(),
            full_content: summary.clone(),
            timestamp: Utc::now(),
        }),

        AgentEvent::Error { error_message, .. } => Some(ServerEvent::Error {
            session_id: Some(session_id.to_string()),
            code: "agent_error".into(),
            message: error_message.clone(),
            timestamp: Utc::now(),
        }),

        AgentEvent::MessageReceived { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orangecoding_core::{AgentId, SessionId, TokenUsage};

    #[test]
    fn test_stream_chunk_converts_to_delta() {
        let event = AgentEvent::stream_chunk(
            AgentId::new(),
            SessionId::new(),
            "Hello world".to_string(),
        );
        let result = agent_event_to_server_event(&event, "sess_001");
        assert!(matches!(result, Some(ServerEvent::AssistantDelta { .. })));
        if let Some(ServerEvent::AssistantDelta { content, .. }) = result {
            assert_eq!(content, "Hello world");
        }
    }

    #[test]
    fn test_completed_converts_to_done() {
        let event = AgentEvent::completed(
            AgentId::new(),
            SessionId::new(),
            "Task finished".to_string(),
        );
        let result = agent_event_to_server_event(&event, "sess_001");
        assert!(matches!(result, Some(ServerEvent::AssistantDone { .. })));
    }

    #[test]
    fn test_usage_converts_to_usage_delta() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        let event = AgentEvent::token_usage_updated(
            AgentId::new(),
            SessionId::new(),
            usage,
        );
        let result = agent_event_to_server_event(&event, "sess_001");
        assert!(matches!(result, Some(ServerEvent::UsageDelta { .. })));
        if let Some(ServerEvent::UsageDelta { total_tokens, .. }) = result {
            assert_eq!(total_tokens, 150);
        }
    }

    #[test]
    fn test_message_received_returns_none() {
        let event = AgentEvent::message_received(
            AgentId::new(),
            SessionId::new(),
            "preview".to_string(),
        );
        let result = agent_event_to_server_event(&event, "sess_001");
        assert!(result.is_none());
    }
}
```

### Step 2: Implement `approval_bridge.rs`

```rust
//! Async approval bridge: suspends tool execution pending user decision.

use orangecoding_control_protocol::{ApprovalDecision, ApprovalRequest, RiskLevel};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

/// Manages pending approval requests with async suspend/resume.
pub struct ApprovalBridge {
    pending: Arc<DashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

impl ApprovalBridge {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(DashMap::new()),
        }
    }

    /// Create an approval request and return a future that resolves when the user decides.
    pub async fn request_approval(
        &self,
        session_id: &str,
        tool_name: &str,
        risk_level: RiskLevel,
        summary: &str,
        arguments: serde_json::Value,
    ) -> (ApprovalRequest, tokio::sync::oneshot::Receiver<ApprovalDecision>) {
        let id = format!("apr_{}", Uuid::new_v4().simple());
        let (tx, rx) = oneshot::channel();

        let request = ApprovalRequest {
            id: id.clone(),
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            risk_level,
            summary: summary.to_string(),
            arguments,
            expires_at: None,
        };

        self.pending.insert(id, tx);
        (request, rx)
    }

    /// Resolve a pending approval.
    pub fn resolve(&self, approval_id: &str, decision: ApprovalDecision) -> bool {
        if let Some((_, tx)) = self.pending.remove(approval_id) {
            tx.send(decision).is_ok()
        } else {
            false
        }
    }

    /// Number of pending approvals.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_approval_approve_flow() {
        let bridge = ApprovalBridge::new();
        let (req, rx) = bridge
            .request_approval(
                "sess_001",
                "bash",
                RiskLevel::High,
                "Run cargo test",
                serde_json::json!({"cmd": "cargo test"}),
            )
            .await;

        assert_eq!(bridge.pending_count(), 1);

        let resolved = bridge.resolve(&req.id, ApprovalDecision::Approved);
        assert!(resolved);

        let decision = rx.await.unwrap();
        assert!(matches!(decision, ApprovalDecision::Approved));
        assert_eq!(bridge.pending_count(), 0);
    }

    #[tokio::test]
    async fn test_approval_deny_flow() {
        let bridge = ApprovalBridge::new();
        let (req, rx) = bridge
            .request_approval(
                "sess_001",
                "bash",
                RiskLevel::High,
                "rm -rf /",
                serde_json::json!({"cmd": "rm -rf /"}),
            )
            .await;

        let resolved = bridge.resolve(
            &req.id,
            ApprovalDecision::Denied {
                reason: Some("Too dangerous".into()),
            },
        );
        assert!(resolved);

        let decision = rx.await.unwrap();
        assert!(matches!(decision, ApprovalDecision::Denied { .. }));
    }

    #[tokio::test]
    async fn test_resolve_nonexistent_returns_false() {
        let bridge = ApprovalBridge::new();
        let resolved = bridge.resolve("apr_nonexistent", ApprovalDecision::Approved);
        assert!(!resolved);
    }
}
```

### Step 3: Implement `session_bridge.rs`

```rust
//! Manages active sessions and their associated agent loops.

use orangecoding_control_protocol::{SessionInfo, SessionState};
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Metadata for a managed session.
pub struct ManagedSession {
    pub info: SessionInfo,
    pub cancel_token: CancellationToken,
}

/// Session supervisor managing lifecycle of agent sessions.
pub struct SessionSupervisor {
    sessions: Arc<DashMap<String, ManagedSession>>,
}

impl SessionSupervisor {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// Create a new session and return its info.
    pub fn create_session(&self, title: Option<String>, _working_directory: Option<String>) -> SessionInfo {
        let id = format!("sess_{}", Uuid::new_v4().simple());
        let now = Utc::now();
        let info = SessionInfo {
            id: id.clone(),
            title,
            state: SessionState::Idle,
            created_at: now,
            updated_at: now,
        };

        let managed = ManagedSession {
            info: info.clone(),
            cancel_token: CancellationToken::new(),
        };

        self.sessions.insert(id, managed);
        info
    }

    /// Get session info by ID.
    pub fn get_session(&self, session_id: &str) -> Option<SessionInfo> {
        self.sessions.get(session_id).map(|s| s.info.clone())
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions.iter().map(|s| s.info.clone()).collect()
    }

    /// Update session state.
    pub fn update_state(&self, session_id: &str, state: SessionState) -> bool {
        if let Some(mut s) = self.sessions.get_mut(session_id) {
            s.info.state = state;
            s.info.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Cancel the running task in a session.
    pub fn cancel_task(&self, session_id: &str) -> bool {
        if let Some(s) = self.sessions.get(session_id) {
            s.cancel_token.cancel();
            true
        } else {
            false
        }
    }

    /// Get a cloned cancellation token for a session.
    pub fn get_cancel_token(&self, session_id: &str) -> Option<CancellationToken> {
        self.sessions.get(session_id).map(|s| s.cancel_token.clone())
    }

    /// Replace the cancellation token (e.g. after a cancel, for next run).
    pub fn reset_cancel_token(&self, session_id: &str) -> bool {
        if let Some(mut s) = self.sessions.get_mut(session_id) {
            s.cancel_token = CancellationToken::new();
            true
        } else {
            false
        }
    }

    /// Close and remove a session.
    pub fn close_session(&self, session_id: &str) -> bool {
        if let Some((_, s)) = self.sessions.remove(session_id) {
            s.cancel_token.cancel();
            true
        } else {
            false
        }
    }

    /// Count of active sessions.
    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_list_sessions() {
        let supervisor = SessionSupervisor::new();
        let s1 = supervisor.create_session(Some("Session 1".into()), None);
        let s2 = supervisor.create_session(Some("Session 2".into()), None);

        assert_eq!(supervisor.count(), 2);
        let list = supervisor.list_sessions();
        assert_eq!(list.len(), 2);

        let found = supervisor.get_session(&s1.id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().title.as_deref(), Some("Session 1"));
    }

    #[test]
    fn test_update_state() {
        let supervisor = SessionSupervisor::new();
        let s = supervisor.create_session(None, None);

        assert!(supervisor.update_state(&s.id, SessionState::Running));
        let info = supervisor.get_session(&s.id).unwrap();
        assert!(matches!(info.state, SessionState::Running));
    }

    #[test]
    fn test_cancel_task() {
        let supervisor = SessionSupervisor::new();
        let s = supervisor.create_session(None, None);
        let token = supervisor.get_cancel_token(&s.id).unwrap();

        assert!(!token.is_cancelled());
        assert!(supervisor.cancel_task(&s.id));
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_close_session() {
        let supervisor = SessionSupervisor::new();
        let s = supervisor.create_session(None, None);
        assert!(supervisor.close_session(&s.id));
        assert_eq!(supervisor.count(), 0);
        assert!(supervisor.get_session(&s.id).is_none());
    }
}
```

### Step 4: Implement `runtime.rs` — Worker orchestrator

```rust
//! Worker runtime: orchestrates agent execution for the control server.

use crate::approval_bridge::ApprovalBridge;
use crate::event_bridge::agent_event_to_server_event;
use crate::session_bridge::SessionSupervisor;
use orangecoding_control_protocol::{ServerEvent, SessionState};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing;

/// Worker runtime manages sessions, agent execution, and event distribution.
pub struct WorkerRuntime {
    pub sessions: Arc<SessionSupervisor>,
    pub approval: Arc<ApprovalBridge>,
    event_tx: broadcast::Sender<ServerEvent>,
}

impl WorkerRuntime {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            sessions: Arc::new(SessionSupervisor::new()),
            approval: Arc::new(ApprovalBridge::new()),
            event_tx,
        }
    }

    /// Subscribe to server events (for WebSocket fanout).
    pub fn subscribe_events(&self) -> broadcast::Receiver<ServerEvent> {
        self.event_tx.subscribe()
    }

    /// Publish a server event to all subscribers.
    pub fn publish_event(&self, event: ServerEvent) {
        if let Err(e) = self.event_tx.send(event) {
            tracing::warn!("No event subscribers: {}", e);
        }
    }

    /// Start consuming agent events from an mpsc channel and converting to ServerEvents.
    pub fn spawn_event_forwarder(
        &self,
        session_id: String,
        mut agent_rx: mpsc::Receiver<orangecoding_core::event::AgentEvent>,
    ) {
        let event_tx = self.event_tx.clone();
        let sessions = self.sessions.clone();

        tokio::spawn(async move {
            while let Some(agent_event) = agent_rx.recv().await {
                if let Some(server_event) =
                    agent_event_to_server_event(&agent_event, &session_id)
                {
                    let _ = event_tx.send(server_event);
                }

                // Update session state on completion/error
                match &agent_event {
                    orangecoding_core::event::AgentEvent::Completed { .. } => {
                        sessions.update_state(&session_id, SessionState::Idle);
                    }
                    orangecoding_core::event::AgentEvent::Error { .. } => {
                        sessions.update_state(&session_id, SessionState::Error);
                    }
                    _ => {}
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_runtime_create_session() {
        let runtime = WorkerRuntime::new();
        let info = runtime.sessions.create_session(Some("Test".into()), None);
        assert_eq!(runtime.sessions.count(), 1);
        assert!(matches!(info.state, SessionState::Idle));
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let runtime = WorkerRuntime::new();
        let mut rx = runtime.subscribe_events();

        runtime.publish_event(ServerEvent::Pong {
            timestamp: chrono::Utc::now(),
        });

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, ServerEvent::Pong { .. }));
    }
}
```

### Step 5: Implement `lib.rs`

```rust
// crates/orangecoding-worker/src/lib.rs
pub mod approval_bridge;
pub mod event_bridge;
pub mod runtime;
pub mod session_bridge;

pub use approval_bridge::ApprovalBridge;
pub use event_bridge::agent_event_to_server_event;
pub use runtime::WorkerRuntime;
pub use session_bridge::SessionSupervisor;
```

### Step 6: Register in workspace and run tests

Add `orangecoding-worker` to workspace `Cargo.toml` members and dependencies.

Run: `cargo test -p orangecoding-worker`
Expected: All tests pass.

### Step 7: Commit

```bash
git add crates/orangecoding-worker/ Cargo.toml Cargo.lock
git commit -m "feat: add orangecoding-worker crate with runtime adapter

- EventBridge: converts AgentEvent to ServerEvent
- ApprovalBridge: async suspend/resume for tool approvals
- SessionSupervisor: manages session lifecycle with cancel tokens
- WorkerRuntime: orchestrates sessions, events, approvals"
```

---

## Task 3: Create `orangecoding-control-server` Crate — HTTP + WebSocket Server

**Files:**
- Create: `crates/orangecoding-control-server/Cargo.toml`
- Create: `crates/orangecoding-control-server/src/lib.rs`
- Create: `crates/orangecoding-control-server/src/auth.rs`
- Create: `crates/orangecoding-control-server/src/routes.rs`
- Create: `crates/orangecoding-control-server/src/ws.rs`
- Create: `crates/orangecoding-control-server/src/session_api.rs`
- Create: `crates/orangecoding-control-server/src/approval_api.rs`
- Modify: `Cargo.toml` (workspace members and deps)
- Test: inline tests + integration test

### Step 1: Create Cargo.toml with axum dependencies

Add to workspace `Cargo.toml` dependencies:
```toml
axum = { version = "0.7", features = ["ws"] }
axum-extra = { version = "0.9", features = ["typed-header"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
hyper = { version = "1.0", features = ["full"] }
```

Create `crates/orangecoding-control-server/Cargo.toml`:

```toml
[package]
name = "orangecoding-control-server"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
orangecoding-core = { workspace = true }
orangecoding-control-protocol = { workspace = true }
orangecoding-worker = { workspace = true }
axum = { workspace = true }
axum-extra = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
futures = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros"] }
reqwest = { workspace = true }
```

### Step 2: Implement `auth.rs` — Local token auth

```rust
//! Simple local-mode authentication.
//! Phase A uses a random one-time token generated at server startup.

use std::sync::Arc;
use uuid::Uuid;

/// Local auth token manager.
#[derive(Clone)]
pub struct LocalAuth {
    token: Arc<String>,
}

impl LocalAuth {
    /// Generate a new random token.
    pub fn generate() -> Self {
        let token = Uuid::new_v4().to_string();
        Self {
            token: Arc::new(token),
        }
    }

    /// Create from a known token (for testing).
    pub fn from_token(token: String) -> Self {
        Self {
            token: Arc::new(token),
        }
    }

    /// Get the token value (for display to user on startup).
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Validate a provided token.
    pub fn validate(&self, provided: &str) -> bool {
        provided == self.token.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_auth() {
        let auth = LocalAuth::generate();
        let token = auth.token().to_string();
        assert!(auth.validate(&token));
        assert!(!auth.validate("wrong_token"));
    }

    #[test]
    fn test_from_token() {
        let auth = LocalAuth::from_token("test_token_123".into());
        assert!(auth.validate("test_token_123"));
    }
}
```

### Step 3: Implement `session_api.rs` — REST session endpoints

```rust
//! HTTP handlers for session management.

use axum::{
    extract::State,
    extract::Path,
    http::StatusCode,
    Json,
};
use orangecoding_control_protocol::SessionInfo;
use orangecoding_worker::WorkerRuntime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub title: Option<String>,
    pub working_directory: Option<String>,
}

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub session: SessionInfo,
}

#[derive(Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionInfo>,
}

/// POST /api/v1/sessions
pub async fn create_session(
    State(runtime): State<Arc<WorkerRuntime>>,
    Json(req): Json<CreateSessionRequest>,
) -> (StatusCode, Json<CreateSessionResponse>) {
    let info = runtime
        .sessions
        .create_session(req.title, req.working_directory);
    (
        StatusCode::CREATED,
        Json(CreateSessionResponse { session: info }),
    )
}

/// GET /api/v1/sessions
pub async fn list_sessions(
    State(runtime): State<Arc<WorkerRuntime>>,
) -> Json<ListSessionsResponse> {
    let sessions = runtime.sessions.list_sessions();
    Json(ListSessionsResponse { sessions })
}

/// GET /api/v1/sessions/:id
pub async fn get_session(
    State(runtime): State<Arc<WorkerRuntime>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionInfo>, StatusCode> {
    runtime
        .sessions
        .get_session(&session_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// POST /api/v1/sessions/:id/cancel
pub async fn cancel_session(
    State(runtime): State<Arc<WorkerRuntime>>,
    Path(session_id): Path<String>,
) -> StatusCode {
    if runtime.sessions.cancel_task(&session_id) {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// DELETE /api/v1/sessions/:id
pub async fn close_session(
    State(runtime): State<Arc<WorkerRuntime>>,
    Path(session_id): Path<String>,
) -> StatusCode {
    if runtime.sessions.close_session(&session_id) {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}
```

### Step 4: Implement `approval_api.rs` — REST approval endpoints

```rust
//! HTTP handlers for approval management.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use orangecoding_control_protocol::ApprovalDecision;
use orangecoding_worker::WorkerRuntime;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ApprovalResponse {
    pub decision: ApprovalDecision,
}

/// POST /api/v1/approvals/:id/respond
pub async fn respond_to_approval(
    State(runtime): State<Arc<WorkerRuntime>>,
    Path(approval_id): Path<String>,
    Json(body): Json<ApprovalResponse>,
) -> StatusCode {
    if runtime.approval.resolve(&approval_id, body.decision) {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}
```

### Step 5: Implement `ws.rs` — WebSocket handler

```rust
//! WebSocket handler for real-time bidirectional communication.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use orangecoding_control_protocol::{ClientCommand, ServerEvent};
use orangecoding_worker::WorkerRuntime;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tracing;

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(_query): Query<WsQuery>,
    State(runtime): State<Arc<WorkerRuntime>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, runtime))
}

async fn handle_socket(socket: WebSocket, runtime: Arc<WorkerRuntime>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let mut event_rx = runtime.subscribe_events();

    // Forward server events to WebSocket
    let forward_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!("Failed to serialize event: {}", e);
                    continue;
                }
            };
            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive client commands from WebSocket
    let runtime_clone = runtime.clone();
    let receive_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
                    match serde_json::from_str::<ClientCommand>(&text) {
                        Ok(cmd) => handle_command(cmd, &runtime_clone).await,
                        Err(e) => {
                            tracing::warn!("Invalid command: {}", e);
                            let error_event = ServerEvent::Error {
                                session_id: None,
                                code: "invalid_command".into(),
                                message: format!("Failed to parse command: {}", e),
                                timestamp: chrono::Utc::now(),
                            };
                            runtime_clone.publish_event(error_event);
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = forward_task => {}
        _ = receive_task => {}
    }
}

async fn handle_command(cmd: ClientCommand, runtime: &WorkerRuntime) {
    match cmd {
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
            let info = runtime
                .sessions
                .create_session(title, working_directory);
            runtime.publish_event(ServerEvent::SessionCreated {
                session_id: info.id,
                title: info.title,
                timestamp: chrono::Utc::now(),
            });
        }
        ClientCommand::TaskCancel { session_id, .. } => {
            runtime.sessions.cancel_task(&session_id);
            runtime.publish_event(ServerEvent::AgentStatus {
                session_id,
                status: "cancelled".into(),
                message: None,
                timestamp: chrono::Utc::now(),
            });
        }
        ClientCommand::ApprovalRespond {
            approval_id,
            decision,
            ..
        } => {
            let resolved = runtime.approval.resolve(&approval_id, decision.clone());
            if resolved {
                runtime.publish_event(ServerEvent::ApprovalResolved {
                    session_id: String::new(),
                    approval_id,
                    decision,
                    timestamp: chrono::Utc::now(),
                });
            }
        }
        ClientCommand::SessionClose { session_id, .. } => {
            runtime.sessions.close_session(&session_id);
        }
        ClientCommand::UserMessage {
            session_id,
            content,
            ..
        } => {
            // Mark session as running
            runtime.sessions.update_state(
                &session_id,
                orangecoding_control_protocol::SessionState::Running,
            );
            runtime.publish_event(ServerEvent::AgentStatus {
                session_id: session_id.clone(),
                status: "running".into(),
                message: Some(format!("Processing: {}", &content[..content.len().min(100)])),
                timestamp: chrono::Utc::now(),
            });
            // Note: Actual agent invocation will be connected in Task 5.
            // For now, this sets up the plumbing.
            tracing::info!(
                session_id = %session_id,
                "User message received, agent invocation pending integration"
            );
        }
        _ => {
            tracing::debug!("Unhandled command type");
        }
    }
}
```

### Step 6: Implement `routes.rs` — Router setup

```rust
//! HTTP router combining all API routes.

use crate::{approval_api, auth::LocalAuth, session_api, ws};
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post},
    Router,
};
use orangecoding_worker::WorkerRuntime;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub runtime: Arc<WorkerRuntime>,
    pub auth: LocalAuth,
}

/// Build the full router with all routes.
pub fn build_router(runtime: Arc<WorkerRuntime>, auth: LocalAuth) -> Router {
    let state = AppState {
        runtime: runtime.clone(),
        auth,
    };

    let api_routes = Router::new()
        // Session endpoints
        .route("/sessions", post(session_api::create_session))
        .route("/sessions", get(session_api::list_sessions))
        .route("/sessions/{id}", get(session_api::get_session))
        .route("/sessions/{id}/cancel", post(session_api::cancel_session))
        .route("/sessions/{id}", delete(session_api::close_session))
        // Approval endpoints
        .route(
            "/approvals/{id}/respond",
            post(approval_api::respond_to_approval),
        )
        .with_state(runtime);

    let ws_route = Router::new()
        .route("/ws", get(ws::ws_handler))
        .with_state(state.runtime.clone());

    // Health check
    let health = Router::new().route("/health", get(health_check));

    Router::new()
        .nest("/api/v1", api_routes)
        .nest("/api/v1", ws_route)
        .merge(health)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
}

async fn health_check() -> &'static str {
    "ok"
}

async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for health check and WebSocket upgrade (WS does its own auth via query param)
    let path = req.uri().path();
    if path == "/health" || path.contains("/ws") {
        return Ok(next.run(req).await);
    }

    // Check Authorization header
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if state.auth.validate(token) {
                Ok(next.run(req).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
```

### Step 7: Implement `lib.rs` — Server entrypoint

```rust
// crates/orangecoding-control-server/src/lib.rs
pub mod approval_api;
pub mod auth;
pub mod routes;
pub mod session_api;
pub mod ws;

use auth::LocalAuth;
use orangecoding_worker::WorkerRuntime;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing;

/// Configuration for the control server.
pub struct ControlServerConfig {
    pub bind_addr: SocketAddr,
}

impl Default for ControlServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 3200)),
        }
    }
}

/// Start the control server. Returns the generated auth token.
pub async fn start_server(
    config: ControlServerConfig,
    runtime: Arc<WorkerRuntime>,
) -> anyhow::Result<String> {
    let auth = LocalAuth::generate();
    let token = auth.token().to_string();

    let app = routes::build_router(runtime, auth);

    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!("Control server listening on {}", config.bind_addr);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Server error: {}", e);
        }
    });

    Ok(token)
}
```

### Step 8: Register in workspace and run tests

Add `orangecoding-control-server` to workspace `Cargo.toml` members and dependencies.

Run: `cargo test -p orangecoding-control-server && cargo check -p orangecoding-control-server`
Expected: All tests pass, crate compiles.

### Step 9: Commit

```bash
git add crates/orangecoding-control-server/ Cargo.toml Cargo.lock
git commit -m "feat: add orangecoding-control-server with HTTP/WebSocket endpoints

- LocalAuth: one-time token authentication for localhost mode
- Session API: create, list, get, cancel, close sessions
- Approval API: respond to pending approvals
- WebSocket handler: bidirectional event/command streaming
- Router: axum-based routing with auth middleware and CORS
- start_server(): launches the control server on a given address"
```

---

## Task 4: Add `serve` Subcommand to `orangecoding-cli`

**Files:**
- Create: `crates/orangecoding-cli/src/commands/serve.rs`
- Modify: `crates/orangecoding-cli/src/main.rs` (add Serve variant)
- Modify: `crates/orangecoding-cli/src/commands/mod.rs` (if exists, add module)
- Modify: `crates/orangecoding-cli/Cargo.toml` (add dependencies)

### Step 1: Write the `serve.rs` command

```rust
// crates/orangecoding-cli/src/commands/serve.rs
use orangecoding_control_server::ControlServerConfig;
use orangecoding_worker::WorkerRuntime;
use clap::Args;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Address to bind the control server to
    #[arg(long, default_value = "127.0.0.1:3200")]
    pub bind: SocketAddr,
}

pub async fn execute(args: ServeArgs) -> anyhow::Result<()> {
    let runtime = Arc::new(WorkerRuntime::new());

    let config = ControlServerConfig {
        bind_addr: args.bind,
    };

    let token = orangecoding_control_server::start_server(config, runtime).await?;

    println!("\n╔══════════════════════════════════════════════════╗");
    println!("║          OrangeCoding Control Server Started            ║");
    println!("╠══════════════════════════════════════════════════╣");
    println!("║                                                  ║");
    println!("║  URL:   http://{}           ║", args.bind);
    println!("║  Token: {}  ║", &token[..36]);
    println!("║                                                  ║");
    println!("║  Health: http://{}/health     ║", args.bind);
    println!("║  WS:     ws://{}/api/v1/ws    ║", args.bind);
    println!("║                                                  ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    // Wait indefinitely (server runs in background task)
    tokio::signal::ctrl_c().await?;
    println!("\nShutting down...");

    Ok(())
}
```

### Step 2: Add `Serve` variant to CLI command enum

In `crates/orangecoding-cli/src/main.rs`, add:
- `Serve(serve::ServeArgs)` to the `Commands` enum
- `Commands::Serve(args) => serve::execute(args).await` to the match

### Step 3: Add dependencies to `orangecoding-cli/Cargo.toml`

```toml
orangecoding-control-server = { workspace = true }
orangecoding-worker = { workspace = true }
orangecoding-control-protocol = { workspace = true }
```

### Step 4: Run build and manual smoke test

Run: `cargo build -p orangecoding-cli`
Expected: Compiles successfully.

Run: `cargo run -p orangecoding-cli -- serve --bind 127.0.0.1:3200`
Expected: Server starts, prints token and URL.

Test health: `curl http://127.0.0.1:3200/health`
Expected: `ok`

### Step 5: Commit

```bash
git add crates/orangecoding-cli/src/commands/serve.rs crates/orangecoding-cli/src/main.rs crates/orangecoding-cli/Cargo.toml Cargo.toml Cargo.lock
git commit -m "feat: add 'OrangeCoding serve' subcommand for local web control

Starts HTTP + WebSocket server on localhost:3200 (configurable via --bind).
Displays auth token on startup. Ctrl+C for graceful shutdown."
```

---

## Task 5: Connect Agent Execution to WebSocket Flow

**Files:**
- Modify: `crates/orangecoding-control-server/src/ws.rs` (wire up agent invocation)
- Modify: `crates/orangecoding-worker/src/runtime.rs` (add run_agent method)
- Modify: `crates/orangecoding-cli/src/commands/serve.rs` (pass config for AI provider)

This task connects the `UserMessage` WebSocket command to actual `AgentLoop` execution.

### Step 1: Add `run_agent_turn` to WorkerRuntime

In `crates/orangecoding-worker/src/runtime.rs`, add a method that:
1. Creates an `AgentContext` for the session
2. Spins up `AgentLoop` with an `mpsc` channel for events
3. Calls `spawn_event_forwarder` to pipe events to WebSocket
4. Runs the agent loop and returns result

```rust
/// Run a single agent turn for a given session with user input.
pub async fn run_agent_turn(
    &self,
    session_id: &str,
    user_content: &str,
    ai_provider: Arc<dyn orangecoding_ai::AiProvider>,
    tool_registry: Arc<orangecoding_tools::ToolRegistry>,
) -> anyhow::Result<()> {
    use orangecoding_agent::{AgentLoop, AgentLoopConfig, AgentContext};
    use orangecoding_core::{AgentId, SessionId};
    use tokio::sync::mpsc;

    let sid = session_id.to_string();

    // Get or create cancel token
    self.sessions.reset_cancel_token(&sid);
    let cancel_token = self.sessions.get_cancel_token(&sid)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}", sid))?;

    self.sessions.update_state(&sid, orangecoding_control_protocol::SessionState::Running);

    // Create agent context
    let mut context = AgentContext::new(
        SessionId::new(),
        std::env::current_dir().unwrap_or_default(),
    );
    context.add_user_message(user_content);

    // Create agent loop
    let executor = orangecoding_agent::ToolExecutor::new(tool_registry);
    let config = AgentLoopConfig::default();
    let mut agent = AgentLoop::new(
        AgentId::new(),
        ai_provider,
        executor,
        context,
        config,
    );

    // Event channel
    let (event_tx, event_rx) = mpsc::channel(256);
    self.spawn_event_forwarder(sid.clone(), event_rx);

    // Run agent in background
    let sessions = self.sessions.clone();
    let sid_clone = sid.clone();
    tokio::spawn(async move {
        let chat_options = orangecoding_ai::ChatOptions::default();
        match agent.run(&chat_options, cancel_token, event_tx).await {
            Ok(_result) => {
                sessions.update_state(&sid_clone, orangecoding_control_protocol::SessionState::Idle);
            }
            Err(e) => {
                tracing::error!(session_id = %sid_clone, "Agent error: {}", e);
                sessions.update_state(&sid_clone, orangecoding_control_protocol::SessionState::Error);
            }
        }
    });

    Ok(())
}
```

### Step 2: Wire up in ws.rs `handle_command`

Update `ClientCommand::UserMessage` handler to call `runtime.run_agent_turn()`.
This requires passing AI provider and tool registry through the runtime or server state.

### Step 3: Run integration test

Create a simple integration test that:
1. Starts the server
2. Connects via WebSocket
3. Sends a `Ping` command
4. Receives a `Pong` response

Run: `cargo test -p orangecoding-control-server`
Expected: All tests pass.

### Step 4: Commit

```bash
git add crates/orangecoding-worker/ crates/orangecoding-control-server/ crates/orangecoding-cli/
git commit -m "feat: connect agent execution to WebSocket flow

UserMessage commands now trigger AgentLoop execution.
Events are streamed back to WebSocket clients in real-time."
```

---

## Task 6: Integration Test — End-to-End Flow

**Files:**
- Create: `tests/integration/control_server_test.rs` or inline in `orangecoding-control-server`

### Step 1: Write integration test

Test the full flow:
1. Create `WorkerRuntime`
2. Start server on a random port
3. HTTP: Create session
4. HTTP: List sessions (verify it appears)
5. HTTP: Get session by ID
6. HTTP: Close session
7. WebSocket: Connect, send Ping, receive Pong

### Step 2: Run all tests

Run: `cargo test --workspace`
Expected: All tests pass, including new integration tests.

### Step 3: Commit

```bash
git add tests/ crates/
git commit -m "test: add integration tests for control server

Tests cover session CRUD, WebSocket ping/pong, and health check."
```

---

## Task 7: Documentation

**Files:**
- Modify: `README.md` (add serve command docs)
- Modify: `docs/architecture/overview.md` (add control plane section)

### Step 1: Update README with `serve` usage

Add a section documenting:
- `OrangeCoding serve --bind 127.0.0.1:3200`
- How to use the auth token
- Available HTTP API endpoints
- WebSocket protocol basics

### Step 2: Update architecture docs

Add a section to `docs/architecture/overview.md` describing the control plane architecture.

### Step 3: Commit

```bash
git add README.md docs/
git commit -m "docs: add control plane documentation

Document the 'OrangeCoding serve' command, HTTP API, WebSocket protocol,
and control plane architecture."
```

---

## Dependency Graph

```
Task 1: orangecoding-control-protocol  (no deps)
    ↓
Task 2: orangecoding-worker            (depends on Task 1)
    ↓
Task 3: orangecoding-control-server    (depends on Task 1, 2)
    ↓
Task 4: CLI serve command       (depends on Task 3)
    ↓
Task 5: Agent execution wiring  (depends on Task 2, 3, 4)
    ↓
Task 6: Integration tests       (depends on Task 5)
    ↓
Task 7: Documentation           (depends on Task 4)
```

## Notes

- **Phase A only**: This plan targets localhost-only web control. No public gateway, no OIDC, no remote workers.
- **axum version**: Uses axum 0.7 which is the latest stable. If the workspace already has a different HTTP framework, adapt accordingly.
- **Agent integration caveat**: Task 5 depends on the exact `AgentLoop::run` API. The code sketches assume the API matches the exploration findings. Verify against actual signatures before implementing.
- **Frontend**: This plan does NOT include the browser UI (Task 7 in the design doc's agent split). A separate plan should cover the React/frontend implementation.
- **Approval bridge**: The current design uses oneshot channels. In production, you may want to add timeout support and persistent approval records.
