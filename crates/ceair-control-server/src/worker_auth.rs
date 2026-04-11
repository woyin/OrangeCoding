use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// Worker 凭证
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerToken {
    pub token: String,
    pub worker_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked: bool,
}

/// Worker 凭证管理器：签发、验证、撤销、轮转
pub struct WorkerTokenManager {
    tokens: Mutex<HashMap<String, WorkerToken>>,
    default_ttl: Duration,
}

impl WorkerTokenManager {
    pub fn new(default_ttl_hours: i64) -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            default_ttl: Duration::hours(default_ttl_hours),
        }
    }

    /// 为指定 worker 签发新令牌
    pub fn issue(&self, worker_id: &str) -> WorkerToken {
        let now = Utc::now();
        let token = WorkerToken {
            token: Uuid::new_v4().to_string(),
            worker_id: worker_id.to_string(),
            issued_at: now,
            expires_at: now + self.default_ttl,
            revoked: false,
        };
        self.tokens
            .lock()
            .unwrap()
            .insert(token.token.clone(), token.clone());
        token
    }

    /// 验证令牌：存在、未过期、未撤销
    pub fn validate(&self, token: &str) -> Option<WorkerToken> {
        let tokens = self.tokens.lock().unwrap();
        tokens.get(token).and_then(|t| {
            if !t.revoked && Utc::now() < t.expires_at {
                Some(t.clone())
            } else {
                None
            }
        })
    }

    /// 撤销指定令牌
    pub fn revoke(&self, token: &str) -> bool {
        let mut tokens = self.tokens.lock().unwrap();
        if let Some(t) = tokens.get_mut(token) {
            t.revoked = true;
            true
        } else {
            false
        }
    }

    /// 撤销某 worker 的所有令牌，返回撤销数量
    pub fn revoke_all_for_worker(&self, worker_id: &str) -> usize {
        let mut tokens = self.tokens.lock().unwrap();
        let mut count = 0;
        for t in tokens.values_mut() {
            if t.worker_id == worker_id && !t.revoked {
                t.revoked = true;
                count += 1;
            }
        }
        count
    }

    /// 轮转令牌：撤销旧令牌并签发新令牌
    pub fn rotate(&self, old_token: &str) -> Option<WorkerToken> {
        let worker_id = {
            let mut tokens = self.tokens.lock().unwrap();
            let old = tokens.get_mut(old_token)?;
            if old.revoked || Utc::now() >= old.expires_at {
                return None;
            }
            old.revoked = true;
            old.worker_id.clone()
        };
        // 签发新令牌（会重新获取锁）
        Some(self.issue(&worker_id))
    }

    /// 清理过期令牌，返回清理数量
    pub fn cleanup_expired(&self) -> usize {
        let mut tokens = self.tokens.lock().unwrap();
        let now = Utc::now();
        let before = tokens.len();
        tokens.retain(|_, t| now < t.expires_at);
        before - tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_and_validate() {
        let mgr = WorkerTokenManager::new(24);
        let token = mgr.issue("worker-1");
        assert_eq!(token.worker_id, "worker-1");
        assert!(!token.revoked);

        let validated = mgr.validate(&token.token);
        assert!(validated.is_some());
        assert_eq!(validated.unwrap().worker_id, "worker-1");
    }

    #[test]
    fn expired_token_rejected() {
        // TTL 为 0 小时，令牌立即过期
        let mgr = WorkerTokenManager::new(0);
        let token = mgr.issue("worker-1");
        // 0 小时 TTL 意味着 expires_at == issued_at，立刻过期
        let validated = mgr.validate(&token.token);
        assert!(validated.is_none());
    }

    #[test]
    fn revoked_token_rejected() {
        let mgr = WorkerTokenManager::new(24);
        let token = mgr.issue("worker-1");
        assert!(mgr.revoke(&token.token));
        assert!(mgr.validate(&token.token).is_none());
    }

    #[test]
    fn rotate_revokes_old_issues_new() {
        let mgr = WorkerTokenManager::new(24);
        let old = mgr.issue("worker-1");
        let new = mgr.rotate(&old.token).expect("rotate should succeed");

        // 旧令牌已失效
        assert!(mgr.validate(&old.token).is_none());
        // 新令牌有效
        assert!(mgr.validate(&new.token).is_some());
        assert_eq!(new.worker_id, "worker-1");
        assert_ne!(old.token, new.token);
    }

    #[test]
    fn cleanup_expired() {
        let mgr = WorkerTokenManager::new(0);
        mgr.issue("w1");
        mgr.issue("w2");
        // TTL=0 意味着都已过期
        let cleaned = mgr.cleanup_expired();
        assert_eq!(cleaned, 2);
    }

    #[test]
    fn revoke_all_for_worker() {
        let mgr = WorkerTokenManager::new(24);
        mgr.issue("worker-1");
        mgr.issue("worker-1");
        mgr.issue("worker-2");
        let count = mgr.revoke_all_for_worker("worker-1");
        assert_eq!(count, 2);
    }

    #[test]
    fn rotate_fails_on_revoked_token() {
        let mgr = WorkerTokenManager::new(24);
        let token = mgr.issue("worker-1");
        mgr.revoke(&token.token);
        assert!(mgr.rotate(&token.token).is_none());
    }
}
