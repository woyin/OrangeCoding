use std::sync::Arc;

use chengcoding_control_protocol::{SessionInfo, SessionState};
use chrono::Utc;
use dashmap::DashMap;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct ManagedSession {
    pub info: SessionInfo,
    pub cancel_token: CancellationToken,
}

pub struct SessionSupervisor {
    sessions: Arc<DashMap<String, ManagedSession>>,
}

impl SessionSupervisor {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn create_session(
        &self,
        title: Option<String>,
        _working_directory: Option<String>,
    ) -> SessionInfo {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

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

    pub fn get_session(&self, id: &str) -> Option<SessionInfo> {
        self.sessions.get(id).map(|s| s.info.clone())
    }

    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions.iter().map(|s| s.info.clone()).collect()
    }

    pub fn update_state(&self, id: &str, state: SessionState) -> bool {
        if let Some(mut session) = self.sessions.get_mut(id) {
            session.info.state = state;
            session.info.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Cancel the current task in a session. The existing token is cancelled
    /// but remains in the map so callers can observe the cancellation.
    pub fn cancel_task(&self, id: &str) -> bool {
        if let Some(session) = self.sessions.get(id) {
            session.cancel_token.cancel();
            true
        } else {
            false
        }
    }

    pub fn get_cancel_token(&self, id: &str) -> Option<CancellationToken> {
        self.sessions.get(id).map(|s| s.cancel_token.clone())
    }

    /// Replace the cancellation token with a fresh one so the session can
    /// accept new work after a previous task was cancelled.
    pub fn reset_cancel_token(&self, id: &str) -> bool {
        if let Some(mut session) = self.sessions.get_mut(id) {
            session.cancel_token = CancellationToken::new();
            true
        } else {
            false
        }
    }

    pub fn close_session(&self, id: &str) -> bool {
        self.sessions.remove(id).is_some()
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SessionSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_list_sessions() {
        let sv = SessionSupervisor::new();
        let s1 = sv.create_session(Some("Session A".into()), None);
        let s2 = sv.create_session(Some("Session B".into()), None);

        assert_eq!(sv.count(), 2);

        let list = sv.list_sessions();
        let ids: Vec<&str> = list.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&s1.id.as_str()));
        assert!(ids.contains(&s2.id.as_str()));
    }

    #[test]
    fn update_state() {
        let sv = SessionSupervisor::new();
        let s = sv.create_session(Some("test".into()), None);
        assert_eq!(sv.get_session(&s.id).unwrap().state, SessionState::Idle);

        assert!(sv.update_state(&s.id, SessionState::Running));
        assert_eq!(sv.get_session(&s.id).unwrap().state, SessionState::Running);

        assert!(!sv.update_state("nonexistent", SessionState::Error));
    }

    #[test]
    fn cancel_task_cancels_token() {
        let sv = SessionSupervisor::new();
        let s = sv.create_session(None, None);
        let token = sv.get_cancel_token(&s.id).unwrap();
        assert!(!token.is_cancelled());

        assert!(sv.cancel_task(&s.id));
        assert!(token.is_cancelled());
    }

    #[test]
    fn close_session_removes_it() {
        let sv = SessionSupervisor::new();
        let s = sv.create_session(Some("ephemeral".into()), None);
        assert_eq!(sv.count(), 1);

        assert!(sv.close_session(&s.id));
        assert_eq!(sv.count(), 0);
        assert!(sv.get_session(&s.id).is_none());

        assert!(!sv.close_session(&s.id));
    }
}
