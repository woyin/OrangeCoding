//! MCP 客户端实现
//!
//! 本模块实现了 MCP（Model Context Protocol）客户端。
//! 客户端负责连接 MCP 服务器、执行初始化握手、
//! 查询可用工具列表、调用工具等操作。
//!
//! 主要功能：
//! - 通过传输层连接服务器并完成初始化
//! - 请求/响应的 ID 关联匹配
//! - 请求超时处理
//! - 断线重连（指数退避策略）

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{Mutex, RwLock};

use crate::error::McpError;
use crate::protocol::*;
use crate::server::ToolDefinition;
use crate::transport::Transport;

// ============================================================================
// 客户端配置
// ============================================================================

/// MCP 客户端配置
///
/// 控制超时、重连策略等客户端行为参数。
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// 请求超时时间
    pub request_timeout: Duration,
    /// 最大重连尝试次数
    pub max_reconnect_attempts: u32,
    /// 重连初始退避时间
    pub reconnect_base_delay: Duration,
    /// 重连最大退避时间
    pub reconnect_max_delay: Duration,
}

impl Default for ClientConfig {
    /// 创建默认配置
    ///
    /// - 请求超时：30 秒
    /// - 最大重连次数：5 次
    /// - 初始退避：1 秒
    /// - 最大退避：30 秒
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            max_reconnect_attempts: 5,
            reconnect_base_delay: Duration::from_secs(1),
            reconnect_max_delay: Duration::from_secs(30),
        }
    }
}

// ============================================================================
// 服务器信息
// ============================================================================

/// MCP 服务器信息
///
/// 在初始化握手过程中从服务器获取的信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// 服务器名称
    pub name: String,
    /// 服务器版本
    pub version: String,
}

// ============================================================================
// MCP 客户端
// ============================================================================

/// MCP 客户端
///
/// 通过传输层与 MCP 服务器通信，支持工具发现和调用。
/// 内部维护请求 ID 计数器用于请求/响应关联。
pub struct McpClient {
    /// 客户端名称
    name: String,
    /// 客户端版本
    version: String,
    /// 客户端配置
    config: ClientConfig,
    /// 传输层实例（连接后设置）
    transport: RwLock<Option<Arc<dyn Transport>>>,
    /// 连接状态标记
    connected: AtomicBool,
    /// 请求 ID 计数器（原子递增，确保线程安全）
    next_request_id: AtomicI64,
    /// 服务器信息（初始化后设置）
    server_info: RwLock<Option<ServerInfo>>,
    /// 服务器协议版本
    protocol_version: RwLock<Option<String>>,
    /// 消息接收互斥锁（确保单一消费者）
    receive_lock: Mutex<()>,
}

impl McpClient {
    /// 创建新的 MCP 客户端实例
    ///
    /// # 参数
    /// - `name`: 客户端名称（在 initialize 请求中发送给服务器）
    /// - `version`: 客户端版本号
    /// - `config`: 客户端配置
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        config: ClientConfig,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            config,
            transport: RwLock::new(None),
            connected: AtomicBool::new(false),
            next_request_id: AtomicI64::new(1),
            server_info: RwLock::new(None),
            protocol_version: RwLock::new(None),
            receive_lock: Mutex::new(()),
        }
    }

    /// 使用默认配置创建客户端
    pub fn with_defaults(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self::new(name, version, ClientConfig::default())
    }

    /// 连接到 MCP 服务器
    ///
    /// 设置传输层并执行初始化握手流程：
    /// 1. 发送 initialize 请求
    /// 2. 接收服务器能力和信息
    /// 3. 发送 initialized 通知
    ///
    /// # 参数
    /// - `transport`: 传输层实例
    pub async fn connect(&self, transport: Arc<dyn Transport>) -> Result<(), McpError> {
        tracing::info!("MCP 客户端连接中...");

        // 保存传输层引用
        *self.transport.write().await = Some(Arc::clone(&transport));

        // 执行初始化握手
        self.initialize().await?;

        // 标记为已连接
        self.connected.store(true, Ordering::SeqCst);
        tracing::info!("MCP 客户端已连接");

        Ok(())
    }

    /// 断开与服务器的连接
    ///
    /// 发送 shutdown 请求，然后关闭传输层。
    pub async fn disconnect(&self) -> Result<(), McpError> {
        if !self.is_connected() {
            return Ok(());
        }

        tracing::info!("MCP 客户端断开连接中...");

        // 发送 shutdown 请求
        if let Err(e) = self.send_request("shutdown", None).await {
            tracing::warn!("发送 shutdown 请求失败: {}", e);
        }

        // 关闭传输层
        let transport = self.transport.read().await;
        if let Some(ref t) = *transport {
            if let Err(e) = t.close().await {
                tracing::warn!("关闭传输层失败: {}", e);
            }
        }
        drop(transport);

        // 清理状态
        *self.transport.write().await = None;
        self.connected.store(false, Ordering::SeqCst);
        *self.server_info.write().await = None;
        *self.protocol_version.write().await = None;

        tracing::info!("MCP 客户端已断开连接");
        Ok(())
    }

    /// 检查客户端是否已连接
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// 执行 MCP 初始化握手
    ///
    /// 发送 initialize 请求并处理响应，然后发送 initialized 通知。
    async fn initialize(&self) -> Result<(), McpError> {
        tracing::info!("执行 MCP 初始化握手...");

        // 构建 initialize 请求参数
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": self.name,
                "version": self.version
            }
        });

        // 发送 initialize 请求
        let response = self.send_request("initialize", Some(params)).await?;

        // 解析服务器响应
        if let Some(result) = response.result {
            // 保存协议版本
            if let Some(version) = result.get("protocolVersion").and_then(|v| v.as_str()) {
                *self.protocol_version.write().await = Some(version.to_string());
            }

            // 保存服务器信息
            if let Some(info) = result.get("serverInfo") {
                let server_info = ServerInfo {
                    name: info
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    version: info
                        .get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                };
                tracing::info!(
                    "已连接到服务器: {} v{}",
                    server_info.name,
                    server_info.version
                );
                *self.server_info.write().await = Some(server_info);
            }
        } else if let Some(error) = response.error {
            return Err(McpError::Protocol(format!(
                "初始化失败: [{}] {}",
                error.code, error.message
            )));
        }

        // 发送 initialized 通知
        self.send_notification("notifications/initialized", None)
            .await?;

        tracing::info!("MCP 初始化握手完成");
        Ok(())
    }

    /// 获取服务器可用的工具列表
    ///
    /// 发送 tools/list 请求并解析返回的工具定义列表。
    ///
    /// # 返回
    /// 工具定义列表
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>, McpError> {
        self.ensure_connected()?;

        let response = self.send_request("tools/list", None).await?;

        if let Some(error) = response.error {
            return Err(McpError::Protocol(format!(
                "获取工具列表失败: [{}] {}",
                error.code, error.message
            )));
        }

        let result = response.result.ok_or_else(|| {
            McpError::Protocol("tools/list 响应缺少 result 字段".to_string())
        })?;

        // 从 result 中解析工具列表
        let tools_value = result.get("tools").ok_or_else(|| {
            McpError::Protocol("tools/list 响应缺少 tools 字段".to_string())
        })?;

        let tools: Vec<ToolDefinition> = serde_json::from_value(tools_value.clone()).map_err(|e| {
            McpError::Protocol(format!("解析工具列表失败: {}", e))
        })?;

        tracing::info!("获取到 {} 个可用工具", tools.len());
        Ok(tools)
    }

    /// 调用服务器上的工具
    ///
    /// 发送 tools/call 请求并返回工具执行结果。
    ///
    /// # 参数
    /// - `name`: 工具名称
    /// - `arguments`: 工具调用参数
    ///
    /// # 返回
    /// 工具调用的原始 JSON 结果
    pub async fn call_tool(
        &self,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        self.ensure_connected()?;

        let tool_name = name.into();
        tracing::info!("调用工具: {}", tool_name);

        let params = json!({
            "name": tool_name,
            "arguments": arguments
        });

        let response = self.send_request("tools/call", Some(params)).await?;

        if let Some(error) = response.error {
            return Err(McpError::Protocol(format!(
                "工具调用失败: [{}] {}",
                error.code, error.message
            )));
        }

        response.result.ok_or_else(|| {
            McpError::Protocol("tools/call 响应缺少 result 字段".to_string())
        })
    }

    /// 获取服务器信息
    pub async fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }

    /// 获取协商的协议版本
    pub async fn protocol_version(&self) -> Option<String> {
        self.protocol_version.read().await.clone()
    }

    // ========================================================================
    // 内部辅助方法
    // ========================================================================

    /// 确保客户端已连接
    fn ensure_connected(&self) -> Result<(), McpError> {
        if !self.is_connected() {
            return Err(McpError::Transport("客户端未连接".to_string()));
        }
        Ok(())
    }

    /// 生成下一个请求 ID
    fn next_id(&self) -> RequestId {
        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        RequestId::Number(id)
    }

    /// 发送请求并等待响应
    ///
    /// 使用超时控制，如果在指定时间内没有收到响应则返回错误。
    ///
    /// # 参数
    /// - `method`: 方法名称
    /// - `params`: 方法参数
    ///
    /// # 返回
    /// 服务器的响应
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, McpError> {
        let transport_guard = self.transport.read().await;
        let transport = transport_guard.as_ref().ok_or_else(|| {
            McpError::Transport("传输层未初始化".to_string())
        })?;

        let request_id = self.next_id();
        let request = JsonRpcRequest::new(method, params, request_id.clone());

        tracing::debug!("发送请求: method={}, id={}", method, request_id);

        // 发送请求
        transport
            .send(&JsonRpcMessage::Request(request))
            .await?;

        // 释放传输层的读锁，避免在等待响应时持有
        drop(transport_guard);

        // 带超时的等待响应
        let response = tokio::time::timeout(self.config.request_timeout, async {
            // 获取消息接收锁，确保同一时间只有一个协程在读取
            let _receive_guard = self.receive_lock.lock().await;
            let transport_guard = self.transport.read().await;
            let transport = transport_guard.as_ref().ok_or_else(|| {
                McpError::Transport("传输层已断开".to_string())
            })?;

            // 持续读取消息，直到收到匹配 ID 的响应
            loop {
                let message = transport.receive().await?;
                match message {
                    JsonRpcMessage::Response(resp) if resp.id == request_id => {
                        return Ok(resp);
                    }
                    JsonRpcMessage::Response(_) => {
                        // 收到不匹配的响应，继续等待
                        tracing::warn!("收到不匹配的响应 ID，继续等待");
                    }
                    JsonRpcMessage::Notification(notif) => {
                        // 收到通知消息，记录后继续等待
                        tracing::debug!("等待响应期间收到通知: {}", notif.method);
                    }
                    JsonRpcMessage::Request(req) => {
                        // 服务器发来的请求（如 ping），暂时忽略
                        tracing::debug!("等待响应期间收到服务器请求: {}", req.method);
                    }
                }
            }
        })
        .await
        .map_err(|_| McpError::Timeout(format!("请求超时: method={}", method)))?;

        response
    }

    /// 发送通知（不等待响应）
    ///
    /// # 参数
    /// - `method`: 通知方法名称
    /// - `params`: 通知参数
    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), McpError> {
        let transport_guard = self.transport.read().await;
        let transport = transport_guard.as_ref().ok_or_else(|| {
            McpError::Transport("传输层未初始化".to_string())
        })?;

        let notification = JsonRpcNotification::new(method, params);

        tracing::debug!("发送通知: method={}", method);

        transport
            .send(&JsonRpcMessage::Notification(notification))
            .await
    }

    /// 尝试重新连接服务器
    ///
    /// 使用指数退避策略进行重连尝试。
    /// 退避时间从 `reconnect_base_delay` 开始，每次翻倍，
    /// 最大不超过 `reconnect_max_delay`。
    ///
    /// # 参数
    /// - `transport`: 新的传输层实例
    pub async fn reconnect(&self, transport: Arc<dyn Transport>) -> Result<(), McpError> {
        tracing::info!("开始重连...");

        let mut attempt = 0u32;
        let mut delay = self.config.reconnect_base_delay;

        loop {
            attempt += 1;
            if attempt > self.config.max_reconnect_attempts {
                return Err(McpError::Transport(format!(
                    "重连失败：已达到最大重试次数 ({})",
                    self.config.max_reconnect_attempts
                )));
            }

            tracing::info!("重连尝试 {}/{}", attempt, self.config.max_reconnect_attempts);

            // 尝试连接
            match self.connect(Arc::clone(&transport)).await {
                Ok(()) => {
                    tracing::info!("重连成功（第 {} 次尝试）", attempt);
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("重连尝试 {} 失败: {}", attempt, e);

                    if attempt >= self.config.max_reconnect_attempts {
                        return Err(McpError::Transport(format!(
                            "重连失败：所有尝试均失败，最后错误: {}",
                            e
                        )));
                    }

                    // 指数退避等待
                    tracing::info!("等待 {:?} 后重试...", delay);
                    tokio::time::sleep(delay).await;

                    // 计算下次退避时间（翻倍但不超过最大值）
                    delay = std::cmp::min(delay * 2, self.config.reconnect_max_delay);
                }
            }
        }
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MemoryTransport;
    use std::sync::Arc;

    /// 测试客户端的创建和默认状态
    #[test]
    fn test_client_creation() {
        let client = McpClient::with_defaults("test-client", "0.1.0");
        assert!(!client.is_connected());
    }

    /// 测试自定义配置的创建
    #[test]
    fn test_client_config() {
        let config = ClientConfig {
            request_timeout: Duration::from_secs(60),
            max_reconnect_attempts: 10,
            reconnect_base_delay: Duration::from_secs(2),
            reconnect_max_delay: Duration::from_millis(60_000),
        };
        let client = McpClient::new("test", "0.1.0", config);
        assert!(!client.is_connected());
    }

    /// 测试通过内存传输进行初始化握手
    #[tokio::test]
    async fn test_client_connect_and_initialize() {
        let (client_transport, server_transport) = MemoryTransport::pair();
        let client_transport = Arc::new(client_transport);
        let server_transport = Arc::new(server_transport);

        let client = Arc::new(McpClient::with_defaults("test-client", "0.1.0"));

        // 在后台模拟服务器响应
        let server_t = Arc::clone(&server_transport);
        let server_handle = tokio::spawn(async move {
            // 接收 initialize 请求
            let message = server_t.receive().await.expect("接收 initialize 失败");
            match message {
                JsonRpcMessage::Request(req) => {
                    assert_eq!(req.method, "initialize");

                    // 发送初始化响应
                    let response = JsonRpcResponse::success(
                        req.id,
                        json!({
                            "protocolVersion": "2024-11-05",
                            "capabilities": {"tools": {}},
                            "serverInfo": {
                                "name": "mock-server",
                                "version": "1.0.0"
                            }
                        }),
                    );
                    server_t
                        .send(&JsonRpcMessage::Response(response))
                        .await
                        .expect("发送响应失败");
                }
                _ => panic!("期望收到请求消息"),
            }

            // 接收 initialized 通知
            let message = server_t.receive().await.expect("接收 initialized 通知失败");
            match message {
                JsonRpcMessage::Notification(notif) => {
                    assert_eq!(notif.method, "notifications/initialized");
                }
                _ => panic!("期望收到通知消息"),
            }
        });

        // 客户端连接
        client.connect(client_transport).await.expect("连接失败");
        server_handle.await.expect("服务器任务失败");

        // 验证连接状态
        assert!(client.is_connected());

        // 验证服务器信息
        let info = client.server_info().await.expect("未获取到服务器信息");
        assert_eq!(info.name, "mock-server");
        assert_eq!(info.version, "1.0.0");

        // 验证协议版本
        let version = client.protocol_version().await.expect("未获取到协议版本");
        assert_eq!(version, "2024-11-05");
    }

    /// 测试未连接时调用方法应报错
    #[tokio::test]
    async fn test_operations_before_connect() {
        let client = McpClient::with_defaults("test", "0.1.0");

        // 未连接时查询工具列表应失败
        let result = client.list_tools().await;
        assert!(result.is_err());

        // 未连接时调用工具应失败
        let result = client.call_tool("test", json!({})).await;
        assert!(result.is_err());
    }

    /// 测试客户端断开连接
    #[tokio::test]
    async fn test_client_disconnect() {
        let (client_transport, server_transport) = MemoryTransport::pair();
        let client_transport = Arc::new(client_transport);
        let server_transport = Arc::new(server_transport);

        let client = Arc::new(McpClient::with_defaults("test", "0.1.0"));

        // 模拟服务器处理初始化
        let server_t = Arc::clone(&server_transport);
        let server_handle = tokio::spawn(async move {
            // 处理 initialize
            let msg = server_t.receive().await.unwrap();
            if let JsonRpcMessage::Request(req) = msg {
                let resp = JsonRpcResponse::success(
                    req.id,
                    json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "serverInfo": {"name": "mock", "version": "1.0"}
                    }),
                );
                server_t.send(&JsonRpcMessage::Response(resp)).await.unwrap();
            }

            // 处理 initialized 通知
            let _msg = server_t.receive().await.unwrap();

            // 处理 shutdown 请求
            if let Ok(msg) = server_t.receive().await {
                if let JsonRpcMessage::Request(req) = msg {
                    let resp = JsonRpcResponse::success(req.id, json!(null));
                    let _ = server_t.send(&JsonRpcMessage::Response(resp)).await;
                }
            }
        });

        // 连接然后断开
        client.connect(client_transport).await.expect("连接失败");
        assert!(client.is_connected());

        client.disconnect().await.expect("断开失败");
        assert!(!client.is_connected());

        let _ = server_handle.await;
    }

    /// 测试请求 ID 自增
    #[test]
    fn test_request_id_generation() {
        let client = McpClient::with_defaults("test", "0.1.0");

        let id1 = client.next_id();
        let id2 = client.next_id();
        let id3 = client.next_id();

        assert_eq!(id1, RequestId::Number(1));
        assert_eq!(id2, RequestId::Number(2));
        assert_eq!(id3, RequestId::Number(3));
    }

    /// 测试默认配置值
    #[test]
    fn test_default_config() {
        let config = ClientConfig::default();
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert_eq!(config.max_reconnect_attempts, 5);
        assert_eq!(config.reconnect_base_delay, Duration::from_secs(1));
        assert_eq!(config.reconnect_max_delay, Duration::from_secs(30));
    }

    /// 测试 ServerInfo 的序列化
    #[test]
    fn test_server_info_serialization() {
        let info = ServerInfo {
            name: "test-server".to_string(),
            version: "1.0.0".to_string(),
        };
        let json = serde_json::to_value(&info).expect("序列化失败");
        assert_eq!(json["name"], "test-server");
        assert_eq!(json["version"], "1.0.0");
    }
}
