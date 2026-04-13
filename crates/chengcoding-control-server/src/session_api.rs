use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chengcoding_worker::WorkerRuntime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub title: Option<String>,
    pub working_directory: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<chengcoding_control_protocol::SessionInfo>,
}

pub async fn create_session(
    State(runtime): State<Arc<WorkerRuntime>>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let info = runtime
        .sessions
        .create_session(req.title, req.working_directory);
    (StatusCode::CREATED, Json(info))
}

pub async fn list_sessions(State(runtime): State<Arc<WorkerRuntime>>) -> impl IntoResponse {
    let sessions = runtime.sessions.list_sessions();
    Json(SessionListResponse { sessions })
}

pub async fn get_session(
    State(runtime): State<Arc<WorkerRuntime>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match runtime.sessions.get_session(&id) {
        Some(info) => Ok(Json(info)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn cancel_session(
    State(runtime): State<Arc<WorkerRuntime>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if runtime.sessions.cancel_task(&id) {
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn close_session(
    State(runtime): State<Arc<WorkerRuntime>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if runtime.sessions.close_session(&id) {
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
