use std::sync::Arc;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use ceair_worker::WorkerRuntime;

use crate::approval_api;
use crate::auth::LocalAuth;
use crate::session_api;
use crate::ws::{self, WsState};

pub fn build_router(runtime: Arc<WorkerRuntime>, auth: LocalAuth) -> Router {
    let api_routes = Router::new()
        .route("/sessions", post(session_api::create_session))
        .route("/sessions", get(session_api::list_sessions))
        .route("/sessions/:id", get(session_api::get_session))
        .route("/sessions/:id", delete(session_api::close_session))
        .route("/sessions/:id/cancel", post(session_api::cancel_session))
        .route(
            "/approvals/:id/respond",
            post(approval_api::respond_approval),
        )
        .with_state(runtime.clone());

    let auth_clone = auth.clone();
    let authed_api = api_routes.layer(middleware::from_fn(move |req, next| {
        let auth = auth_clone.clone();
        auth_middleware(auth, req, next)
    }));

    let ws_state = WsState {
        runtime: runtime.clone(),
        auth,
    };

    Router::new()
        .nest("/api/v1", authed_api)
        .route(
            "/api/v1/ws",
            get(ws::ws_handler).with_state(ws_state),
        )
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}

async fn auth_middleware(
    auth: LocalAuth,
    req: Request,
    next: Next,
) -> Response {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if auth.validate(token) {
                next.run(req).await
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        }
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}
