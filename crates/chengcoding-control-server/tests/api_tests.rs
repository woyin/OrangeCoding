//! Integration tests for the control server HTTP API.
//!
//! These tests start a real HTTP server on a random port and exercise
//! the session, approval, health, and auth endpoints.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chengcoding_control_server::auth::LocalAuth;
use chengcoding_control_server::routes::build_router;
use chengcoding_control_server::worker_registry::{WorkerInfo, WorkerRegistry, WorkerStatus};
use chengcoding_worker::WorkerRuntime;
use chrono::Utc;
use tower::ServiceExt;

fn setup() -> (
    axum::Router,
    LocalAuth,
    Arc<WorkerRuntime>,
    Arc<WorkerRegistry>,
) {
    let runtime = Arc::new(WorkerRuntime::new());
    let auth = LocalAuth::generate();
    let registry = Arc::new(WorkerRegistry::new());
    let router = build_router(runtime.clone(), auth.clone(), registry.clone());
    (router, auth, runtime, registry)
}

fn authed_get(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap()
}

fn authed_post(path: &str, token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn authed_delete(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(path)
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap()
}

fn make_worker(id: &str) -> WorkerInfo {
    let now = Utc::now();
    WorkerInfo {
        worker_id: id.to_string(),
        version: "1.0.0".to_string(),
        connected_at: now,
        last_heartbeat: now,
        status: WorkerStatus::Online,
        session_count: 0,
        capabilities: vec![],
    }
}

// ---- Health endpoint ----

#[tokio::test]
async fn health_returns_ok() {
    let (app, _, _, _) = setup();
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---- Auth middleware ----

#[tokio::test]
async fn unauthenticated_request_returns_401() {
    let (app, _, _, _) = setup();
    let req = Request::builder()
        .uri("/api/v1/sessions")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_token_returns_401() {
    let (app, _, _, _) = setup();
    let req = authed_get("/api/v1/sessions", "wrong-token");

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ---- Session API ----

#[tokio::test]
async fn list_sessions_empty() {
    let (app, auth, _, _) = setup();
    let req = authed_get("/api/v1/sessions", auth.token());

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["sessions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn create_and_get_session() {
    let (_app, auth, runtime, registry) = setup();

    // Create a session directly via runtime (avoids oneshot issues)
    let info = runtime
        .sessions
        .create_session(Some("test session".into()), None);
    let session_id = info.id.clone();

    // Rebuild router for GET (oneshot consumes the service)
    let app = build_router(runtime.clone(), auth.clone(), registry);
    let get_req = authed_get(&format!("/api/v1/sessions/{}", session_id), auth.token());
    let resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let session: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(session["title"].as_str().unwrap(), "test session");

    // List sessions should have 1
    let app = build_router(
        runtime.clone(),
        auth.clone(),
        Arc::new(WorkerRegistry::new()),
    );
    let list_req = authed_get("/api/v1/sessions", auth.token());
    let resp = app.oneshot(list_req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["sessions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn create_session_via_api() {
    let (app, auth, runtime, _) = setup();

    let create_req = authed_post(
        "/api/v1/sessions",
        auth.token(),
        r#"{"title":"api created"}"#,
    );
    let resp = app.oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(created["id"].as_str().is_some());
    assert_eq!(created["title"].as_str().unwrap(), "api created");

    // Verify it was actually stored
    assert_eq!(runtime.sessions.count(), 1);
}

#[tokio::test]
async fn get_nonexistent_session_returns_404() {
    let (app, auth, _, _) = setup();
    let req = authed_get("/api/v1/sessions/nonexistent", auth.token());

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_session() {
    let (_app, auth, runtime, registry) = setup();

    // Create a session directly
    let info = runtime
        .sessions
        .create_session(Some("cancel me".into()), None);
    let session_id = info.id.clone();

    // Cancel it via API
    let app = build_router(runtime.clone(), auth.clone(), registry);
    let cancel_req = authed_post(
        &format!("/api/v1/sessions/{}/cancel", session_id),
        auth.token(),
        "{}",
    );
    let resp = app.oneshot(cancel_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn close_session() {
    let (_app, auth, runtime, registry) = setup();

    // Create a session directly
    let info = runtime
        .sessions
        .create_session(Some("close me".into()), None);
    let session_id = info.id.clone();

    // Close it via API
    let app = build_router(runtime.clone(), auth.clone(), registry);
    let delete_req = authed_delete(&format!("/api/v1/sessions/{}", session_id), auth.token());
    let resp = app.oneshot(delete_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Should be gone
    assert!(runtime.sessions.get_session(&session_id).is_none());
}

// ---- Worker API ----

#[tokio::test]
async fn list_workers_empty() {
    let (app, auth, _, _) = setup();
    let req = authed_get("/api/v1/workers", auth.token());

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["workers"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_workers_with_entries() {
    let (_app, auth, runtime, registry) = setup();
    registry.register(make_worker("w-1"));
    registry.register(make_worker("w-2"));

    let app = build_router(runtime, auth.clone(), registry);
    let req = authed_get("/api/v1/workers", auth.token());
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["workers"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_worker_found() {
    let (_app, auth, runtime, registry) = setup();
    registry.register(make_worker("w-1"));

    let app = build_router(runtime, auth.clone(), registry);
    let req = authed_get("/api/v1/workers/w-1", auth.token());
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["worker_id"].as_str().unwrap(), "w-1");
}

#[tokio::test]
async fn get_worker_not_found() {
    let (app, auth, _, _) = setup();
    let req = authed_get("/api/v1/workers/nonexistent", auth.token());

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn drain_worker_api() {
    let (_app, auth, runtime, registry) = setup();
    registry.register(make_worker("w-1"));

    let app = build_router(runtime, auth.clone(), registry.clone());
    let req = authed_post("/api/v1/workers/w-1/drain", auth.token(), "{}");
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    assert_eq!(registry.get("w-1").unwrap().status, WorkerStatus::Draining);
}

#[tokio::test]
async fn revoke_worker_api() {
    let (_app, auth, runtime, registry) = setup();
    registry.register(make_worker("w-1"));
    assert_eq!(registry.worker_count(), 1);

    let app = build_router(runtime, auth.clone(), registry.clone());
    let req = authed_post("/api/v1/workers/w-1/revoke", auth.token(), "{}");
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    assert_eq!(registry.worker_count(), 0);
}
