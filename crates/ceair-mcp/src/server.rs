//! MCP 服务器实现
//!
//! 本模块实现了 MCP（Model Context Protocol）服务器端逻辑。
//! 服务器负责注册和暴露工具（Tools），处理客户端的初始化、
//! 工具列表查询、工具调用等请求。
//!
//! MCP 生命周期：
//! 1. 客户端发送 `initialize` 请求
//! 2. 服务器返回能力（capabilities）
//! 3. 客户端发送 `notifications/initialized` 通知
//! 4. 正常操作阶段（工具列表、工具调用等）
//! 5. 客户端发送 `shutdown` 请求
//! 6. 服务器确认关闭

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;

use crate::error::McpError;
use crate::protocol::*;
use crate::transport::Transport;

// ============================================================================
// MCP 能力声明
// ============================================================================

/// MCP 服务器能力声明
///
/// 在 initialize 响应中告知客户端服务器支持哪些功能。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCapabilities {
    /// 是否支持工具功能
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    /// 是否支持资源功能
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    /// 是否支持提示功能
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
}

/// 工具能力配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsCapability {
    /// 当工具列表发生变化时是否发送通知
    #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// 资源能力配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourcesCapability {
    /// 是否支持资源订阅
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    /// 当资源列表发生变化时是否发送通知
    #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// 提示能力配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromptsCapability {
    /// 当提示列表发生变化时是否发送通知
    #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

impl McpCapabilities {
    /// 创建默认能力声明（仅启用工具功能）
    pub fn with_tools() -> Self {
        Self {
            tools: Some(ToolsCapability::default()),
            resources: None,
            prompts: None,
        }
    }

    /// 创建完整能力声明（启用所有功能）
    pub fn all() -> Self {
        Self {
            tools: Some(ToolsCapability::default()),
            resources: Some(ResourcesCapability::default()),
            prompts: Some(PromptsCapability::default()),
        }
    }
}

// ============================================================================
// 工具定义
// ============================================================================

/// MCP 工具定义
///
/// 描述一个可供客户端调用的工具，包括名称、描述和输入参数的 JSON Schema。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具的唯一名称标识
    pub name: String,
    /// 工具的功能描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 工具输入参数的 JSON Schema 定义
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// 创建新的工具定义
    ///
    /// # 参数
    /// - `name`: 工具名称
    /// - `description`: 工具描述
    /// - `input_schema`: 输入参数的 JSON Schema
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: Some(description.into()),
            input_schema,
        }
    }
}

// ============================================================================
// 工具调用结果
// ============================================================================

/// 工具调用结果内容
///
/// MCP 工具调用的返回内容，目前支持文本类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    /// 文本内容
    #[serde(rename = "text")]
    Text {
        /// 文本内容字符串
        text: String,
    },
}

/// 工具调用结果
///
/// 封装工具执行后的返回内容和是否出错的标记。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 返回内容列表
    pub content: Vec<ToolContent>,
    /// 是否为错误结果
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolResult {
    /// 创建成功的文本结果
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text {
                text: content.into(),
            }],
            is_error: None,
        }
    }

    /// 创建错误的文本结果
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text {
                text: message.into(),
            }],
            is_error: Some(true),
        }
    }
}

// ============================================================================
// 工具处理器特征
// ============================================================================

/// 工具处理器特征
///
/// 每个注册的工具都需要实现此特征来处理实际的调用逻辑。
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// 执行工具调用
    ///
    /// # 参数
    /// - `arguments`: 客户端传入的调用参数
    ///
    /// # 返回
    /// 工具执行结果
    async fn call(&self, arguments: serde_json::Value) -> Result<ToolResult, McpError>;
}

// ============================================================================
// 服务器状态
// ============================================================================

/// MCP 服务器运行状态
#[derive(Debug, Clone, PartialEq, Eq)]
enum ServerState {
    /// 未初始化：等待客户端发送 initialize 请求
    Uninitialized,
    /// 已初始化：收到 initialize 请求，等待 initialized 通知
    Initializing,
    /// 正常运行：可以处理工具调用等请求
    Running,
    /// 正在关闭：收到 shutdown 请求
    ShuttingDown,
    /// 已停止
    Stopped,
}

// ============================================================================
// MCP 服务器
// ============================================================================

/// MCP 服务器
///
/// 管理工具注册、请求分发和 MCP 生命周期。
/// 通过传输层与客户端通信，支持工具列表查询和工具调用。
pub struct McpServer {
    /// 服务器名称
    name: String,
    /// 服务器版本
    version: String,
    /// 服务器能力声明
    capabilities: McpCapabilities,
    /// 已注册的工具定义
    tools: Arc<RwLock<HashMap<String, ToolDefinition>>>,
    /// 已注册的工具处理器
    handlers: Arc<RwLock<HashMap<String, Arc<dyn ToolHandler>>>>,
    /// 服务器当前状态
    state: Arc<RwLock<ServerState>>,
}

impl McpServer {
    /// 创建新的 MCP 服务器实例
    ///
    /// # 参数
    /// - `name`: 服务器名称（在 initialize 响应中返回）
    /// - `version`: 服务器版本号
    /// - `capabilities`: 服务器能力声明
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        capabilities: McpCapabilities,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            capabilities,
            tools: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(ServerState::Uninitialized)),
        }
    }

    /// 注册一个工具
    ///
    /// 同时注册工具定义（用于 tools/list 响应）和工具处理器（用于实际调用）。
    ///
    /// # 参数
    /// - `definition`: 工具的定义信息
    /// - `handler`: 工具的调用处理器
    pub async fn register_tool(
        &self,
        definition: ToolDefinition,
        handler: Arc<dyn ToolHandler>,
    ) {
        let name = definition.name.clone();
        self.tools.write().await.insert(name.clone(), definition);
        self.handlers.write().await.insert(name.clone(), handler);
        tracing::info!("已注册工具: {}", name);
    }

    /// 启动服务器，开始处理消息循环
    ///
    /// 持续从传输层读取消息并进行处理，直到收到 shutdown 请求
    /// 或传输层断开连接。
    ///
    /// # 参数
    /// - `transport`: 传输层实例
    pub async fn start(&self, transport: Arc<dyn Transport>) -> Result<(), McpError> {
        tracing::info!("MCP 服务器 '{}' 启动中...", self.name);

        loop {
            // 检查服务器状态
            let state = self.state.read().await.clone();
            if state == ServerState::Stopped {
                tracing::info!("服务器已停止，退出消息循环");
                break;
            }

            // 从传输层接收消息
            let message = match transport.receive().await {
                Ok(msg) => msg,
                Err(McpError::Transport(ref e)) if e.contains("EOF") || e.contains("关闭") => {
                    tracing::info!("传输层已关闭，服务器停止");
                    break;
                }
                Err(e) => {
                    tracing::error!("接收消息失败: {}", e);
                    continue;
                }
            };

            // 根据消息类型进行分发处理
            match message {
                JsonRpcMessage::Request(request) => {
                    let response = self.handle_request(&request).await;
                    if let Err(e) = transport
                        .send(&JsonRpcMessage::Response(response))
                        .await
                    {
                        tracing::error!("发送响应失败: {}", e);
                    }
                }
                JsonRpcMessage::Notification(notification) => {
                    self.handle_notification(&notification).await;
                }
                JsonRpcMessage::Response(_) => {
                    // 服务器通常不处理响应消息
                    tracing::warn!("收到意外的响应消息，忽略");
                }
            }

            // 如果处于关闭中状态，在处理完当前消息后退出
            let state = self.state.read().await.clone();
            if state == ServerState::ShuttingDown {
                *self.state.write().await = ServerState::Stopped;
                break;
            }
        }

        tracing::info!("MCP 服务器 '{}' 已停止", self.name);
        Ok(())
    }

    /// 停止服务器
    ///
    /// 将服务器状态设置为已停止，消息循环将在下次检查时退出。
    pub async fn stop(&self) {
        *self.state.write().await = ServerState::Stopped;
        tracing::info!("MCP 服务器停止信号已发出");
    }

    /// 处理 JSON-RPC 请求
    ///
    /// 根据方法名称路由到对应的处理函数。
    /// 如果服务器未完成初始化，除 initialize 外的请求都会被拒绝。
    pub async fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        tracing::debug!("处理请求: method={}, id={}", request.method, request.id);

        let state = self.state.read().await.clone();

        // 初始化前只允许 initialize 请求
        if state == ServerState::Uninitialized && request.method != "initialize" {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_request("服务器未初始化，请先发送 initialize 请求"),
            );
        }

        // 根据方法名分发请求
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "shutdown" => self.handle_shutdown(request).await,
            "ping" => self.handle_ping(request).await,
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tool_call(request).await,
            _ => {
                // 未知方法
                JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::method_not_found(format!("未知方法: {}", request.method)),
                )
            }
        }
    }

    /// 处理 initialize 请求
    ///
    /// 返回服务器信息和能力声明。
    /// 服务器进入 Initializing 状态，等待 initialized 通知。
    async fn handle_initialize(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        tracing::info!("处理 initialize 请求");

        // 更新状态为初始化中
        *self.state.write().await = ServerState::Initializing;

        // 构建初始化响应
        let result = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": self.capabilities,
            "serverInfo": {
                "name": self.name,
                "version": self.version
            }
        });

        JsonRpcResponse::success(request.id.clone(), result)
    }

    /// 处理 shutdown 请求
    ///
    /// 标记服务器为关闭中状态，返回 null 结果。
    async fn handle_shutdown(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        tracing::info!("处理 shutdown 请求");
        *self.state.write().await = ServerState::ShuttingDown;
        JsonRpcResponse::success(request.id.clone(), json!(null))
    }

    /// 处理 ping 请求
    ///
    /// 简单的心跳检测，返回空对象。
    async fn handle_ping(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        JsonRpcResponse::success(request.id.clone(), json!({}))
    }

    /// 处理 tools/list 请求
    ///
    /// 返回所有已注册工具的定义列表。
    async fn handle_tools_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let tools = self.tools.read().await;
        let tool_list: Vec<&ToolDefinition> = tools.values().collect();

        let result = json!({
            "tools": tool_list
        });

        JsonRpcResponse::success(request.id.clone(), result)
    }

    /// 处理 tools/call 请求
    ///
    /// 查找并执行指定名称的工具，返回执行结果。
    ///
    /// 期望的参数格式：
    /// ```json
    /// {
    ///     "name": "工具名称",
    ///     "arguments": { ... }
    /// }
    /// ```
    async fn handle_tool_call(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        // 解析请求参数
        let params = match &request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::invalid_params("缺少工具调用参数"),
                );
            }
        };

        // 提取工具名称
        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(name) => name.to_string(),
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::invalid_params("缺少工具名称参数 'name'"),
                );
            }
        };

        // 提取工具调用参数（默认为空对象）
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        tracing::info!("调用工具: {}", tool_name);

        // 查找工具处理器
        let handlers = self.handlers.read().await;
        let handler = match handlers.get(&tool_name) {
            Some(h) => Arc::clone(h),
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    JsonRpcError::method_not_found(format!("未找到工具: {}", tool_name)),
                );
            }
        };
        // 释放读锁，避免在工具执行期间持有锁
        drop(handlers);

        // 执行工具调用
        match handler.call(arguments).await {
            Ok(result) => {
                let result_json = serde_json::to_value(&result).unwrap_or_else(|e| {
                    json!({"content": [{"type": "text", "text": format!("结果序列化失败: {}", e)}], "isError": true})
                });
                JsonRpcResponse::success(request.id.clone(), result_json)
            }
            Err(e) => {
                // 工具执行出错时返回错误结果（不是 JSON-RPC 层面的错误）
                let error_result = ToolResult::error(format!("工具执行失败: {}", e));
                let result_json = serde_json::to_value(&error_result).unwrap_or_else(|_| {
                    json!({"content": [{"type": "text", "text": "未知错误"}], "isError": true})
                });
                JsonRpcResponse::success(request.id.clone(), result_json)
            }
        }
    }

    /// 处理 JSON-RPC 通知
    ///
    /// 通知不需要返回响应。主要处理 initialized 通知来完成初始化流程。
    async fn handle_notification(&self, notification: &JsonRpcNotification) {
        tracing::debug!("处理通知: method={}", notification.method);

        match notification.method.as_str() {
            "notifications/initialized" => {
                // 客户端确认初始化完成，服务器进入正常运行状态
                let mut state = self.state.write().await;
                if *state == ServerState::Initializing {
                    *state = ServerState::Running;
                    tracing::info!("MCP 服务器初始化完成，进入运行状态");
                }
            }
            "notifications/cancelled" => {
                // 客户端取消了一个正在执行的请求
                tracing::info!("收到取消通知: {:?}", notification.params);
            }
            _ => {
                tracing::debug!("收到未处理的通知: {}", notification.method);
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
    use crate::protocol::RequestId;
    use serde_json::json;

    /// 用于测试的简单工具处理器
    struct EchoHandler;

    #[async_trait]
    impl ToolHandler for EchoHandler {
        async fn call(&self, arguments: serde_json::Value) -> Result<ToolResult, McpError> {
            let text = arguments
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("无输入");
            Ok(ToolResult::text(format!("回声: {}", text)))
        }
    }

    /// 用于测试的会失败的工具处理器
    struct FailingHandler;

    #[async_trait]
    impl ToolHandler for FailingHandler {
        async fn call(&self, _arguments: serde_json::Value) -> Result<ToolResult, McpError> {
            Err(McpError::Internal("模拟工具执行失败".to_string()))
        }
    }

    /// 创建测试用的服务器实例
    fn create_test_server() -> McpServer {
        McpServer::new("test-server", "0.1.0", McpCapabilities::with_tools())
    }

    /// 测试 initialize 请求处理
    #[tokio::test]
    async fn test_handle_initialize() {
        let server = create_test_server();

        let request = JsonRpcRequest::new(
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "0.1.0"
                }
            })),
            RequestId::Number(1),
        );

        let response = server.handle_request(&request).await;

        // 验证响应包含正确的服务器信息
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "test-server");
        assert_eq!(result["serverInfo"]["version"], "0.1.0");
    }

    /// 测试未初始化时拒绝非 initialize 请求
    #[tokio::test]
    async fn test_reject_before_initialize() {
        let server = create_test_server();

        let request = JsonRpcRequest::new("tools/list", None, RequestId::Number(1));

        let response = server.handle_request(&request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, INVALID_REQUEST);
    }

    /// 测试 tools/list 请求处理
    #[tokio::test]
    async fn test_handle_tools_list() {
        let server = create_test_server();

        // 先初始化服务器
        let init_req = JsonRpcRequest::new("initialize", None, RequestId::Number(1));
        server.handle_request(&init_req).await;

        // 模拟收到 initialized 通知
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        server.handle_notification(&notification).await;

        // 注册一个测试工具
        let tool_def = ToolDefinition::new(
            "echo",
            "回声工具：返回输入的文本",
            json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "要回声的文本"
                    }
                },
                "required": ["text"]
            }),
        );
        server.register_tool(tool_def, Arc::new(EchoHandler)).await;

        // 查询工具列表
        let list_req = JsonRpcRequest::new("tools/list", None, RequestId::Number(2));
        let response = server.handle_request(&list_req).await;

        assert!(response.error.is_none());
        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "echo");
    }

    /// 测试 tools/call 请求处理
    #[tokio::test]
    async fn test_handle_tool_call() {
        let server = create_test_server();

        // 初始化服务器
        let init_req = JsonRpcRequest::new("initialize", None, RequestId::Number(1));
        server.handle_request(&init_req).await;
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        server.handle_notification(&notification).await;

        // 注册回声工具
        let tool_def = ToolDefinition::new(
            "echo",
            "回声工具",
            json!({"type": "object", "properties": {"text": {"type": "string"}}}),
        );
        server.register_tool(tool_def, Arc::new(EchoHandler)).await;

        // 调用工具
        let call_req = JsonRpcRequest::new(
            "tools/call",
            Some(json!({
                "name": "echo",
                "arguments": {"text": "你好世界"}
            })),
            RequestId::Number(3),
        );

        let response = server.handle_request(&call_req).await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        let content = result["content"].as_array().unwrap();
        assert!(content[0]["text"].as_str().unwrap().contains("你好世界"));
    }

    /// 测试调用不存在的工具
    #[tokio::test]
    async fn test_call_unknown_tool() {
        let server = create_test_server();

        // 初始化
        let init_req = JsonRpcRequest::new("initialize", None, RequestId::Number(1));
        server.handle_request(&init_req).await;
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        server.handle_notification(&notification).await;

        // 调用不存在的工具
        let call_req = JsonRpcRequest::new(
            "tools/call",
            Some(json!({"name": "不存在的工具"})),
            RequestId::Number(2),
        );

        let response = server.handle_request(&call_req).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, METHOD_NOT_FOUND);
    }

    /// 测试工具调用缺少参数
    #[tokio::test]
    async fn test_tool_call_missing_params() {
        let server = create_test_server();

        // 初始化
        let init_req = JsonRpcRequest::new("initialize", None, RequestId::Number(1));
        server.handle_request(&init_req).await;
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        server.handle_notification(&notification).await;

        // 不带参数调用工具
        let call_req = JsonRpcRequest::new("tools/call", None, RequestId::Number(2));
        let response = server.handle_request(&call_req).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, INVALID_PARAMS);
    }

    /// 测试工具执行失败的处理
    #[tokio::test]
    async fn test_tool_call_handler_error() {
        let server = create_test_server();

        // 初始化
        let init_req = JsonRpcRequest::new("initialize", None, RequestId::Number(1));
        server.handle_request(&init_req).await;
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        server.handle_notification(&notification).await;

        // 注册会失败的工具
        let tool_def = ToolDefinition::new("fail", "失败工具", json!({"type": "object"}));
        server
            .register_tool(tool_def, Arc::new(FailingHandler))
            .await;

        // 调用工具
        let call_req = JsonRpcRequest::new(
            "tools/call",
            Some(json!({"name": "fail"})),
            RequestId::Number(2),
        );
        let response = server.handle_request(&call_req).await;

        // 工具执行失败应返回成功的 JSON-RPC 响应，但结果中标记 isError
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], true);
    }

    /// 测试 shutdown 请求
    #[tokio::test]
    async fn test_handle_shutdown() {
        let server = create_test_server();

        // 初始化
        let init_req = JsonRpcRequest::new("initialize", None, RequestId::Number(1));
        server.handle_request(&init_req).await;
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        server.handle_notification(&notification).await;

        // 关闭
        let shutdown_req = JsonRpcRequest::new("shutdown", None, RequestId::Number(2));
        let response = server.handle_request(&shutdown_req).await;
        assert!(response.error.is_none());
    }

    /// 测试 ping 请求
    #[tokio::test]
    async fn test_handle_ping() {
        let server = create_test_server();

        // 初始化
        let init_req = JsonRpcRequest::new("initialize", None, RequestId::Number(1));
        server.handle_request(&init_req).await;
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        server.handle_notification(&notification).await;

        let ping_req = JsonRpcRequest::new("ping", None, RequestId::Number(2));
        let response = server.handle_request(&ping_req).await;
        assert!(response.error.is_none());
        assert_eq!(response.result.unwrap(), json!({}));
    }

    /// 测试未知方法的处理
    #[tokio::test]
    async fn test_unknown_method() {
        let server = create_test_server();

        // 初始化
        let init_req = JsonRpcRequest::new("initialize", None, RequestId::Number(1));
        server.handle_request(&init_req).await;
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        server.handle_notification(&notification).await;

        let req = JsonRpcRequest::new("nonexistent/method", None, RequestId::Number(2));
        let response = server.handle_request(&req).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, METHOD_NOT_FOUND);
    }

    /// 测试 McpCapabilities 的构建
    #[test]
    fn test_capabilities_builder() {
        let tools_only = McpCapabilities::with_tools();
        assert!(tools_only.tools.is_some());
        assert!(tools_only.resources.is_none());
        assert!(tools_only.prompts.is_none());

        let all = McpCapabilities::all();
        assert!(all.tools.is_some());
        assert!(all.resources.is_some());
        assert!(all.prompts.is_some());
    }

    /// 测试 ToolResult 的创建
    #[test]
    fn test_tool_result() {
        let success = ToolResult::text("成功");
        assert!(success.is_error.is_none());
        assert_eq!(success.content.len(), 1);

        let error = ToolResult::error("失败");
        assert_eq!(error.is_error, Some(true));
    }

    /// 测试 ToolDefinition 的序列化
    #[test]
    fn test_tool_definition_serialization() {
        let tool = ToolDefinition::new(
            "test_tool",
            "测试工具",
            json!({"type": "object"}),
        );
        let json = serde_json::to_value(&tool).expect("序列化失败");
        assert_eq!(json["name"], "test_tool");
        assert_eq!(json["description"], "测试工具");
        assert_eq!(json["inputSchema"]["type"], "object");
    }
}
