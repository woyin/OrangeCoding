use std::sync::Arc;

use dashmap::DashMap;

use crate::worker_registry::WorkerRegistry;

/// Session 到 Worker 的路由表
/// 创建 session 时选择负载最低的 Worker 并记录映射关系
pub struct SessionRouter {
    /// session_id → worker_id 映射
    routes: Arc<DashMap<String, String>>,
    /// Worker 注册表引用
    registry: Arc<WorkerRegistry>,
}

impl SessionRouter {
    pub fn new(registry: Arc<WorkerRegistry>) -> Self {
        Self {
            routes: Arc::new(DashMap::new()),
            registry,
        }
    }

    /// 为指定 session 分配 Worker，返回选中的 worker_id
    /// 如果没有可用 Worker 则返回 None
    pub fn assign(&self, session_id: &str) -> Option<String> {
        if let Some(worker_id) = self.registry.select_worker() {
            self.routes
                .insert(session_id.to_string(), worker_id.clone());
            // Worker may have been removed between select and increment;
            // roll back the route to avoid routing to a dead worker.
            if !self.registry.increment_session_count(&worker_id) {
                self.routes.remove(session_id);
                return None;
            }
            Some(worker_id)
        } else {
            None
        }
    }

    /// 查询 session 被路由到了哪个 Worker
    pub fn get_worker_for_session(&self, session_id: &str) -> Option<String> {
        self.routes.get(session_id).map(|v| v.value().clone())
    }

    /// 移除 session 路由并递减 Worker 的 session 计数
    pub fn remove(&self, session_id: &str) -> Option<String> {
        if let Some((_, worker_id)) = self.routes.remove(session_id) {
            self.registry.decrement_session_count(&worker_id);
            Some(worker_id)
        } else {
            None
        }
    }

    /// 返回当前路由数量
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker_registry::{WorkerInfo, WorkerStatus};
    use chrono::Utc;

    fn make_worker(id: &str, sessions: usize) -> WorkerInfo {
        let now = Utc::now();
        WorkerInfo {
            worker_id: id.to_string(),
            version: "1.0.0".to_string(),
            connected_at: now,
            last_heartbeat: now,
            status: WorkerStatus::Online,
            session_count: sessions,
            capabilities: vec![],
        }
    }

    #[test]
    fn assign_session_to_least_loaded_worker() {
        let registry = Arc::new(WorkerRegistry::new());
        registry.register(make_worker("w-1", 5));
        registry.register(make_worker("w-2", 1));

        let router = SessionRouter::new(registry.clone());
        let assigned = router.assign("sess-1").unwrap();
        assert_eq!(assigned, "w-2");

        // session 计数应递增
        assert_eq!(registry.get("w-2").unwrap().session_count, 2);
    }

    #[test]
    fn assign_returns_none_when_no_workers() {
        let registry = Arc::new(WorkerRegistry::new());
        let router = SessionRouter::new(registry);
        assert!(router.assign("sess-1").is_none());
    }

    #[test]
    fn get_and_remove_route() {
        let registry = Arc::new(WorkerRegistry::new());
        registry.register(make_worker("w-1", 0));

        let router = SessionRouter::new(registry.clone());
        router.assign("sess-1");

        assert_eq!(router.get_worker_for_session("sess-1").unwrap(), "w-1");
        assert_eq!(router.route_count(), 1);

        let removed = router.remove("sess-1").unwrap();
        assert_eq!(removed, "w-1");
        assert_eq!(router.route_count(), 0);
        assert_eq!(registry.get("w-1").unwrap().session_count, 0);
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let registry = Arc::new(WorkerRegistry::new());
        let router = SessionRouter::new(registry);
        assert!(router.remove("nonexistent").is_none());
    }
}
