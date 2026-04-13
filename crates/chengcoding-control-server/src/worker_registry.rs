use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Worker 状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Online,
    Draining,
    Offline,
}

/// 已连接 Worker 的元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub version: String,
    pub connected_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub status: WorkerStatus,
    pub session_count: usize,
    pub capabilities: Vec<String>,
}

/// Worker 注册表 — 管理已连接的 Worker
/// 使用 DashMap 支持并发安全的读写
pub struct WorkerRegistry {
    workers: Arc<DashMap<String, WorkerInfo>>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(DashMap::new()),
        }
    }

    /// 注册一个 Worker，返回 true 表示新注册，false 表示已存在（更新）
    pub fn register(&self, info: WorkerInfo) -> bool {
        use dashmap::mapref::entry::Entry;
        match self.workers.entry(info.worker_id.clone()) {
            Entry::Occupied(mut entry) => {
                entry.insert(info);
                false
            }
            Entry::Vacant(entry) => {
                entry.insert(info);
                true
            }
        }
    }

    /// 注销一个 Worker，返回 true 表示找到并移除
    pub fn unregister(&self, worker_id: &str) -> bool {
        self.workers.remove(worker_id).is_some()
    }

    /// 获取指定 Worker 的信息
    pub fn get(&self, worker_id: &str) -> Option<WorkerInfo> {
        self.workers.get(worker_id).map(|w| w.value().clone())
    }

    /// 列出所有在线的 Worker
    pub fn list_online(&self) -> Vec<WorkerInfo> {
        self.workers
            .iter()
            .filter(|entry| entry.value().status == WorkerStatus::Online)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 更新 Worker 的心跳时间戳
    pub fn update_heartbeat(&self, worker_id: &str) -> bool {
        if let Some(mut entry) = self.workers.get_mut(worker_id) {
            entry.last_heartbeat = Utc::now();
            true
        } else {
            false
        }
    }

    /// 设置 Worker 状态
    pub fn set_status(&self, worker_id: &str, status: WorkerStatus) -> bool {
        if let Some(mut entry) = self.workers.get_mut(worker_id) {
            entry.status = status;
            true
        } else {
            false
        }
    }

    /// 选择负载最低的在线 Worker（按 session_count 排序）
    pub fn select_worker(&self) -> Option<String> {
        self.workers
            .iter()
            .filter(|entry| entry.value().status == WorkerStatus::Online)
            .min_by_key(|entry| entry.value().session_count)
            .map(|entry| entry.value().worker_id.clone())
    }

    /// 返回注册的 Worker 总数
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// 列出所有 Worker（不论状态）
    pub fn list_all(&self) -> Vec<WorkerInfo> {
        self.workers
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 增加 Worker 的 session 计数
    pub fn increment_session_count(&self, worker_id: &str) -> bool {
        if let Some(mut entry) = self.workers.get_mut(worker_id) {
            entry.session_count += 1;
            true
        } else {
            false
        }
    }

    /// 减少 Worker 的 session 计数
    pub fn decrement_session_count(&self, worker_id: &str) -> bool {
        if let Some(mut entry) = self.workers.get_mut(worker_id) {
            if entry.session_count > 0 {
                entry.session_count -= 1;
            }
            true
        } else {
            false
        }
    }
}

impl Default for WorkerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_worker(id: &str) -> WorkerInfo {
        let now = Utc::now();
        WorkerInfo {
            worker_id: id.to_string(),
            version: "1.0.0".to_string(),
            connected_at: now,
            last_heartbeat: now,
            status: WorkerStatus::Online,
            session_count: 0,
            capabilities: vec!["browser".to_string()],
        }
    }

    #[test]
    fn register_and_get_worker() {
        let registry = WorkerRegistry::new();
        let worker = make_worker("w-1");

        assert!(registry.register(worker));
        assert_eq!(registry.worker_count(), 1);

        let info = registry.get("w-1").unwrap();
        assert_eq!(info.worker_id, "w-1");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.status, WorkerStatus::Online);
    }

    #[test]
    fn register_duplicate_returns_false() {
        let registry = WorkerRegistry::new();
        assert!(registry.register(make_worker("w-1")));
        // 再次注册同 ID 的 Worker，返回 false 表示更新
        assert!(!registry.register(make_worker("w-1")));
        assert_eq!(registry.worker_count(), 1);
    }

    #[test]
    fn unregister_worker() {
        let registry = WorkerRegistry::new();
        registry.register(make_worker("w-1"));
        assert!(registry.unregister("w-1"));
        assert_eq!(registry.worker_count(), 0);
        assert!(registry.get("w-1").is_none());

        // 注销不存在的 Worker 返回 false
        assert!(!registry.unregister("nonexistent"));
    }

    #[test]
    fn list_online_filters_correctly() {
        let registry = WorkerRegistry::new();
        registry.register(make_worker("w-online"));

        let mut draining = make_worker("w-draining");
        draining.status = WorkerStatus::Draining;
        registry.register(draining);

        let mut offline = make_worker("w-offline");
        offline.status = WorkerStatus::Offline;
        registry.register(offline);

        let online = registry.list_online();
        assert_eq!(online.len(), 1);
        assert_eq!(online[0].worker_id, "w-online");
    }

    #[test]
    fn update_heartbeat() {
        let registry = WorkerRegistry::new();
        let worker = make_worker("w-1");
        let original_heartbeat = worker.last_heartbeat;
        registry.register(worker);

        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(registry.update_heartbeat("w-1"));

        let info = registry.get("w-1").unwrap();
        assert!(info.last_heartbeat > original_heartbeat);

        assert!(!registry.update_heartbeat("nonexistent"));
    }

    #[test]
    fn set_status() {
        let registry = WorkerRegistry::new();
        registry.register(make_worker("w-1"));

        assert!(registry.set_status("w-1", WorkerStatus::Draining));
        assert_eq!(registry.get("w-1").unwrap().status, WorkerStatus::Draining);

        assert!(registry.set_status("w-1", WorkerStatus::Offline));
        assert_eq!(registry.get("w-1").unwrap().status, WorkerStatus::Offline);

        assert!(!registry.set_status("nonexistent", WorkerStatus::Online));
    }

    #[test]
    fn select_worker_picks_least_loaded() {
        let registry = WorkerRegistry::new();

        let mut w1 = make_worker("w-1");
        w1.session_count = 5;
        registry.register(w1);

        let mut w2 = make_worker("w-2");
        w2.session_count = 2;
        registry.register(w2);

        let mut w3 = make_worker("w-3");
        w3.session_count = 8;
        registry.register(w3);

        // 应选择 session_count 最小的 w-2
        let selected = registry.select_worker().unwrap();
        assert_eq!(selected, "w-2");
    }

    #[test]
    fn select_worker_ignores_non_online() {
        let registry = WorkerRegistry::new();

        let mut w1 = make_worker("w-1");
        w1.session_count = 0;
        w1.status = WorkerStatus::Draining;
        registry.register(w1);

        let mut w2 = make_worker("w-2");
        w2.session_count = 10;
        registry.register(w2);

        // w-1 是 Draining，不可选；只能选 w-2
        let selected = registry.select_worker().unwrap();
        assert_eq!(selected, "w-2");
    }

    #[test]
    fn select_worker_returns_none_when_empty() {
        let registry = WorkerRegistry::new();
        assert!(registry.select_worker().is_none());
    }

    #[test]
    fn increment_and_decrement_session_count() {
        let registry = WorkerRegistry::new();
        registry.register(make_worker("w-1"));

        assert!(registry.increment_session_count("w-1"));
        assert!(registry.increment_session_count("w-1"));
        assert_eq!(registry.get("w-1").unwrap().session_count, 2);

        assert!(registry.decrement_session_count("w-1"));
        assert_eq!(registry.get("w-1").unwrap().session_count, 1);

        assert!(!registry.increment_session_count("nonexistent"));
        assert!(!registry.decrement_session_count("nonexistent"));
    }
}
