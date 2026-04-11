use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 远程请求审计上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAuditContext {
    pub trace_id: String,
    pub request_id: String,
    pub session_id: Option<String>,
    pub worker_id: Option<String>,
    pub user_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub approval_id: Option<String>,
    pub source_ip: String,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// 审计事件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    LoginSuccess,
    LoginFailure,
    WorkerRegister,
    WorkerRevoke,
    SessionCreate,
    SessionMessage,
    ApprovalTriggered,
    ApprovalApproved,
    ApprovalDenied,
    HighRiskToolExec,
    SessionClose,
}

/// 完整审计记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: String,
    pub event_type: AuditEventType,
    pub context: RemoteAuditContext,
    pub details: String,
    pub created_at: DateTime<Utc>,
}

/// 审计记录器：内存存储，支持按类型/会话/用户查询
pub struct AuditRecorder {
    records: std::sync::Mutex<Vec<AuditRecord>>,
}

impl AuditRecorder {
    pub fn new() -> Self {
        Self {
            records: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// 记录一条审计事件
    pub fn record(
        &self,
        event_type: AuditEventType,
        context: RemoteAuditContext,
        details: &str,
    ) {
        let record = AuditRecord {
            id: Uuid::new_v4().to_string(),
            event_type,
            context,
            details: details.to_string(),
            created_at: Utc::now(),
        };
        self.records.lock().unwrap().push(record);
    }

    /// 按事件类型查询
    pub fn query_by_type(&self, event_type: &AuditEventType) -> Vec<AuditRecord> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|r| &r.event_type == event_type)
            .cloned()
            .collect()
    }

    /// 按会话 ID 查询
    pub fn query_by_session(&self, session_id: &str) -> Vec<AuditRecord> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.context.session_id.as_deref() == Some(session_id))
            .cloned()
            .collect()
    }

    /// 按用户 ID 查询
    pub fn query_by_user(&self, user_id: &str) -> Vec<AuditRecord> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.context.user_id.as_deref() == Some(user_id))
            .cloned()
            .collect()
    }

    /// 返回总记录数
    pub fn total_records(&self) -> usize {
        self.records.lock().unwrap().len()
    }
}

impl Default for AuditRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(
        session_id: Option<&str>,
        user_id: Option<&str>,
    ) -> RemoteAuditContext {
        RemoteAuditContext {
            trace_id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            session_id: session_id.map(String::from),
            worker_id: None,
            user_id: user_id.map(String::from),
            tool_call_id: None,
            approval_id: None,
            source_ip: "127.0.0.1".to_string(),
            user_agent: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn record_and_query_by_type() {
        let recorder = AuditRecorder::new();
        recorder.record(
            AuditEventType::LoginSuccess,
            make_context(None, Some("u1")),
            "user logged in",
        );
        recorder.record(
            AuditEventType::LoginFailure,
            make_context(None, Some("u2")),
            "bad password",
        );
        recorder.record(
            AuditEventType::LoginSuccess,
            make_context(None, Some("u3")),
            "another login",
        );

        let results = recorder.query_by_type(&AuditEventType::LoginSuccess);
        assert_eq!(results.len(), 2);
        let failures = recorder.query_by_type(&AuditEventType::LoginFailure);
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn query_by_session() {
        let recorder = AuditRecorder::new();
        recorder.record(
            AuditEventType::SessionCreate,
            make_context(Some("s1"), None),
            "created",
        );
        recorder.record(
            AuditEventType::SessionMessage,
            make_context(Some("s1"), None),
            "message sent",
        );
        recorder.record(
            AuditEventType::SessionCreate,
            make_context(Some("s2"), None),
            "another session",
        );

        let results = recorder.query_by_session("s1");
        assert_eq!(results.len(), 2);
        let results2 = recorder.query_by_session("s2");
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn query_by_user() {
        let recorder = AuditRecorder::new();
        recorder.record(
            AuditEventType::LoginSuccess,
            make_context(None, Some("alice")),
            "login",
        );
        recorder.record(
            AuditEventType::SessionCreate,
            make_context(Some("s1"), Some("alice")),
            "create session",
        );
        recorder.record(
            AuditEventType::LoginSuccess,
            make_context(None, Some("bob")),
            "login",
        );

        let alice = recorder.query_by_user("alice");
        assert_eq!(alice.len(), 2);
        let bob = recorder.query_by_user("bob");
        assert_eq!(bob.len(), 1);
    }

    #[test]
    fn total_records_count() {
        let recorder = AuditRecorder::new();
        assert_eq!(recorder.total_records(), 0);
        recorder.record(
            AuditEventType::WorkerRegister,
            make_context(None, None),
            "worker joined",
        );
        recorder.record(
            AuditEventType::WorkerRevoke,
            make_context(None, None),
            "worker revoked",
        );
        assert_eq!(recorder.total_records(), 2);
    }

    #[test]
    fn query_empty_returns_nothing() {
        let recorder = AuditRecorder::new();
        assert!(recorder.query_by_type(&AuditEventType::HighRiskToolExec).is_empty());
        assert!(recorder.query_by_session("nonexistent").is_empty());
        assert!(recorder.query_by_user("nobody").is_empty());
    }
}
