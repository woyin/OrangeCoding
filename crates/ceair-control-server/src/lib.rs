pub mod approval_api;
pub mod audit_ext;
pub mod auth;
pub mod rate_limit;
pub mod rbac;
pub mod routes;
pub mod session_api;
pub mod session_router;
pub mod worker_api;
pub mod worker_auth;
pub mod worker_registry;
pub mod ws;

use std::net::SocketAddr;
use std::sync::Arc;

use ceair_worker::WorkerRuntime;
use tokio::net::TcpListener;

use crate::auth::LocalAuth;
use crate::worker_registry::WorkerRegistry;

pub struct ControlServerConfig {
    pub bind_addr: SocketAddr,
}

impl Default for ControlServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 3200)),
        }
    }
}

/// Start the control server. Returns the generated auth token.
pub async fn start_server(
    config: ControlServerConfig,
    runtime: Arc<WorkerRuntime>,
) -> anyhow::Result<String> {
    let auth = LocalAuth::generate();
    let token = auth.token().to_string();
    let registry = Arc::new(WorkerRegistry::new());
    let app = routes::build_router(runtime, auth, registry);
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!("Control server listening on {}", config.bind_addr);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Server error: {}", e);
        }
    });
    Ok(token)
}
