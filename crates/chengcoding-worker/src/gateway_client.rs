use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Worker 到 Gateway 的连接客户端
/// 负责建立和维护 Worker 与 Gateway 之间的 WebSocket 通道
pub struct GatewayClient {
    /// Gateway URL（例如 http://127.0.0.1:3200）
    gateway_url: String,
    /// Worker 标识
    worker_id: String,
    /// Worker 版本
    worker_version: String,
    /// 认证 token
    auth_token: String,
    /// 连接状态
    connected: Arc<AtomicBool>,
    /// 重连间隔（秒）
    reconnect_interval: u64,
}

/// 默认重连间隔（秒）
const DEFAULT_RECONNECT_INTERVAL: u64 = 5;

impl GatewayClient {
    /// 创建新的 GatewayClient 实例
    pub fn new(
        gateway_url: String,
        worker_id: String,
        worker_version: String,
        auth_token: String,
    ) -> Self {
        Self {
            gateway_url,
            worker_id,
            worker_version,
            auth_token,
            connected: Arc::new(AtomicBool::new(false)),
            reconnect_interval: DEFAULT_RECONNECT_INTERVAL,
        }
    }

    /// 返回当前连接状态
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// 设置重连间隔
    pub fn set_reconnect_interval(&mut self, seconds: u64) {
        self.reconnect_interval = seconds;
    }

    /// 获取重连间隔
    pub fn reconnect_interval(&self) -> u64 {
        self.reconnect_interval
    }

    /// 获取 Worker ID
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// 获取 Worker 版本
    pub fn worker_version(&self) -> &str {
        &self.worker_version
    }

    /// 构建连接 URL: {gateway_url}/api/v1/worker/connect
    pub fn build_connect_url(&self) -> String {
        let base = self.gateway_url.trim_end_matches('/');
        format!("{}/api/v1/worker/connect", base)
    }

    /// 构建请求头，包含认证和 Worker 身份信息
    pub fn build_headers(&self) -> Vec<(String, String)> {
        vec![
            (
                "Authorization".to_string(),
                format!("Bearer {}", self.auth_token),
            ),
            ("X-Worker-Id".to_string(), self.worker_id.clone()),
            ("X-Worker-Version".to_string(), self.worker_version.clone()),
        ]
    }

    /// 获取连接状态的原子引用（用于跨线程共享）
    pub fn connected_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.connected)
    }

    /// 设置连接状态
    pub fn set_connected(&self, value: bool) {
        self.connected.store(value, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client() -> GatewayClient {
        GatewayClient::new(
            "http://127.0.0.1:3200".to_string(),
            "worker-001".to_string(),
            "0.1.0".to_string(),
            "secret-token".to_string(),
        )
    }

    #[test]
    fn new_client_is_disconnected() {
        let client = make_client();
        assert!(!client.is_connected());
        assert_eq!(client.worker_id(), "worker-001");
        assert_eq!(client.worker_version(), "0.1.0");
    }

    #[test]
    fn build_connect_url_appends_path() {
        let client = make_client();
        assert_eq!(
            client.build_connect_url(),
            "http://127.0.0.1:3200/api/v1/worker/connect"
        );
    }

    #[test]
    fn build_connect_url_trims_trailing_slash() {
        let client = GatewayClient::new(
            "http://gateway.example.com/".to_string(),
            "w-1".to_string(),
            "1.0.0".to_string(),
            "tok".to_string(),
        );
        assert_eq!(
            client.build_connect_url(),
            "http://gateway.example.com/api/v1/worker/connect"
        );
    }

    #[test]
    fn build_headers_contains_auth_and_worker_id() {
        let client = make_client();
        let headers = client.build_headers();

        assert_eq!(headers.len(), 3);

        let auth = headers.iter().find(|(k, _)| k == "Authorization").unwrap();
        assert_eq!(auth.1, "Bearer secret-token");

        let wid = headers.iter().find(|(k, _)| k == "X-Worker-Id").unwrap();
        assert_eq!(wid.1, "worker-001");

        let wver = headers
            .iter()
            .find(|(k, _)| k == "X-Worker-Version")
            .unwrap();
        assert_eq!(wver.1, "0.1.0");
    }

    #[test]
    fn set_reconnect_interval() {
        let mut client = make_client();
        assert_eq!(client.reconnect_interval(), DEFAULT_RECONNECT_INTERVAL);

        client.set_reconnect_interval(30);
        assert_eq!(client.reconnect_interval(), 30);
    }

    #[test]
    fn connected_flag_shared_across_clones() {
        let client = make_client();
        let flag = client.connected_flag();

        assert!(!client.is_connected());
        flag.store(true, Ordering::Relaxed);
        assert!(client.is_connected());
    }

    #[test]
    fn set_connected_changes_state() {
        let client = make_client();
        assert!(!client.is_connected());

        client.set_connected(true);
        assert!(client.is_connected());

        client.set_connected(false);
        assert!(!client.is_connected());
    }
}
