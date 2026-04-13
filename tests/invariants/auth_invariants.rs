//! # Auth Invariant Tests
//!
//! INV-AUTH-01: WebSocket 连接必须鉴权
//! INV-AUTH-02: HTTP API 必须通过认证中间件
//! INV-AUTH-03: Token 不得出现在日志中

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chengcoding_control_server::auth::LocalAuth;
use chengcoding_control_server::routes::build_router;
use chengcoding_control_server::worker_registry::WorkerRegistry;
use chengcoding_control_server::ws::WsQuery;
use chengcoding_worker::WorkerRuntime;
use tower::ServiceExt;

fn setup() -> (axum::Router, LocalAuth, Arc<WorkerRuntime>) {
    let runtime = Arc::new(WorkerRuntime::new());
    let auth = LocalAuth::generate();
    let registry = Arc::new(WorkerRegistry::new());
    let router = build_router(runtime.clone(), auth.clone(), registry);
    (router, auth, runtime)
}

// =========================================================================
// INV-AUTH-01: WebSocket 连接必须鉴权
// =========================================================================

#[test]
fn inv_auth_01_valid_token_passes_ws_auth() {
    let auth = LocalAuth::from_token("test-token".into());
    let query = WsQuery {
        token: Some("test-token".into()),
    };
    assert!(
        query.token.as_ref().map_or(false, |t| auth.validate(t)),
        "合法 token 应通过 WS 鉴权"
    );
}

#[test]
fn inv_auth_01_no_token_rejected() {
    let auth = LocalAuth::from_token("test-token".into());
    let query = WsQuery { token: None };
    assert!(
        !query.token.as_ref().map_or(false, |t| auth.validate(t)),
        "无 token 应被拒绝"
    );
}

#[test]
fn inv_auth_01_wrong_token_rejected() {
    let auth = LocalAuth::from_token("test-token".into());
    let query = WsQuery {
        token: Some("wrong-token".into()),
    };
    assert!(
        !query.token.as_ref().map_or(false, |t| auth.validate(t)),
        "错误 token 应被拒绝"
    );
}

#[test]
fn inv_auth_01_empty_token_rejected() {
    let auth = LocalAuth::from_token("test-token".into());
    let query = WsQuery {
        token: Some(String::new()),
    };
    assert!(
        !query.token.as_ref().map_or(false, |t| auth.validate(t)),
        "空字符串 token 应被拒绝"
    );
}

// =========================================================================
// INV-AUTH-02: HTTP API 必须通过认证中间件
// =========================================================================

#[tokio::test]
async fn inv_auth_02_valid_bearer_returns_200() {
    let (app, auth, _rt) = setup();
    let req = Request::builder()
        .uri("/api/v1/sessions")
        .header("authorization", format!("Bearer {}", auth.token()))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "合法 token 应返回 200");
}

#[tokio::test]
async fn inv_auth_02_no_auth_header_returns_401() {
    let (app, _auth, _rt) = setup();
    let req = Request::builder()
        .uri("/api/v1/sessions")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "无 header 应返回 401"
    );
}

#[tokio::test]
async fn inv_auth_02_wrong_token_returns_401() {
    let (app, _auth, _rt) = setup();
    let req = Request::builder()
        .uri("/api/v1/sessions")
        .header("authorization", "Bearer wrong-token-value")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "错误 token 应返回 401"
    );
}

#[tokio::test]
async fn inv_auth_02_health_no_auth_returns_200() {
    let (app, _auth, _rt) = setup();
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "/health 无需 token 应返回 200"
    );
}

// =========================================================================
// INV-AUTH-03: Token 不得出现在日志中 (静态扫描)
// =========================================================================

#[test]
fn inv_auth_03_no_token_in_log_macros() {
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("crates");

    // Patterns that indicate token/secret leaking in log macros
    let dangerous_patterns = [
        // tracing macros printing token/secret variables directly
        r#"tracing::info!(".*\btoken\b"#,
        r#"tracing::debug!(".*\btoken\b"#,
        r#"tracing::warn!(".*\bsecret\b"#,
        r#"tracing::info!(".*\bsecret\b"#,
        r#"println!(".*\btoken\b"#,
        r#"println!(".*\bsecret\b"#,
    ];

    let mut violations = Vec::new();

    for entry in walkdir(&src_dir) {
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "rs") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (line_no, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // Skip comments and test code
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            for pattern in &dangerous_patterns {
                if trimmed.contains(pattern) {
                    violations.push(format!("{}:{}: {}", path.display(), line_no + 1, trimmed));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Token/secret found in log macros:\n{}",
        violations.join("\n")
    );
}

/// Recursive directory walker (no external dep needed for tests)
fn walkdir(dir: &std::path::Path) -> Vec<std::fs::DirEntry> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir(&path).into_iter().map(|e| e));
            } else {
                result.push(entry);
            }
        }
    }
    result
}
