use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::worker_registry::{WorkerRegistry, WorkerStatus};

/// Worker API 的共享状态
pub type WorkerApiState = Arc<WorkerRegistry>;

/// GET /api/v1/workers — 列出所有 Worker
pub async fn list_workers(State(registry): State<WorkerApiState>) -> impl IntoResponse {
    let workers = registry.list_all();
    Json(serde_json::json!({ "workers": workers }))
}

/// GET /api/v1/workers/:id — 获取指定 Worker 信息
pub async fn get_worker(
    State(registry): State<WorkerApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match registry.get(&id) {
        Some(info) => Ok(Json(info)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// POST /api/v1/workers/:id/drain — 设置 Worker 为 Draining 状态
pub async fn drain_worker(
    State(registry): State<WorkerApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if registry.set_status(&id, WorkerStatus::Draining) {
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// POST /api/v1/workers/:id/revoke — 移除 Worker
pub async fn revoke_worker(
    State(registry): State<WorkerApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if registry.unregister(&id) {
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
