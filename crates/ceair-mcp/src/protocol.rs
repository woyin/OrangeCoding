//! JSON-RPC 2.0 协议类型定义
//!
//! 本模块实现了 MCP 所需的 JSON-RPC 2.0 协议基础类型，
//! 包括请求、响应、通知、错误码等核心结构。

use serde::{Deserialize, Serialize};

// ============================================================================
// 标准 JSON-RPC 2.0 错误码常量
// ============================================================================

/// 解析错误：服务器收到无效的 JSON
pub const PARSE_ERROR: i32 = -32700;
/// 无效请求：发送的 JSON 不是有效的请求对象
pub const INVALID_REQUEST: i32 = -32600;
/// 方法未找到：请求的方法不存在或不可用
pub const METHOD_NOT_FOUND: i32 = -32601;
/// 无效参数：方法参数无效
pub const INVALID_PARAMS: i32 = -32602;
/// 内部错误：JSON-RPC 内部错误
pub const INTERNAL_ERROR: i32 = -32603;

// ============================================================================
// 请求 ID 类型
// ============================================================================

/// JSON-RPC 请求标识符
///
/// 根据 JSON-RPC 2.0 规范，id 可以是数字、字符串或 null。
/// 用于将响应与对应的请求进行关联匹配。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// 数字类型的请求 ID
    Number(i64),
    /// 字符串类型的请求 ID
    String(String),
    /// 空值类型的请求 ID
    Null,
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::Number(n) => write!(f, "{}", n),
            RequestId::String(s) => write!(f, "{}", s),
            RequestId::Null => write!(f, "null"),
        }
    }
}

// ============================================================================
// JSON-RPC 请求
// ============================================================================

/// JSON-RPC 2.0 请求结构
///
/// 表示客户端发送给服务器的一个方法调用请求。
/// 每个请求都包含一个唯一的 id，用于匹配对应的响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC 协议版本，固定为 "2.0"
    pub jsonrpc: String,
    /// 要调用的方法名称
    pub method: String,
    /// 方法参数（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// 请求标识符，用于关联响应
    pub id: RequestId,
}

impl JsonRpcRequest {
    /// 创建新的 JSON-RPC 请求
    ///
    /// # 参数
    /// - `method`: 要调用的方法名称
    /// - `params`: 方法参数（可选的 JSON 值）
    /// - `id`: 请求标识符
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>, id: RequestId) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
            id,
        }
    }
}

// ============================================================================
// JSON-RPC 响应
// ============================================================================

/// JSON-RPC 2.0 响应结构
///
/// 服务器对客户端请求的响应。
/// result 和 error 互斥：成功时包含 result，失败时包含 error。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC 协议版本，固定为 "2.0"
    pub jsonrpc: String,
    /// 方法执行成功时的返回结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// 方法执行失败时的错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// 对应请求的标识符
    pub id: RequestId,
}

impl JsonRpcResponse {
    /// 创建成功响应
    ///
    /// # 参数
    /// - `id`: 对应请求的标识符
    /// - `result`: 方法执行的返回值
    pub fn success(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// 创建错误响应
    ///
    /// # 参数
    /// - `id`: 对应请求的标识符
    /// - `error`: 错误信息
    pub fn error(id: RequestId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

// ============================================================================
// JSON-RPC 错误
// ============================================================================

/// JSON-RPC 2.0 错误对象
///
/// 当方法调用失败时，响应中会包含此错误对象。
/// 包括错误码、错误消息和可选的附加数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// 错误码，标识错误类型
    pub code: i32,
    /// 错误的简短描述
    pub message: String,
    /// 附加的错误数据（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// 创建新的 JSON-RPC 错误
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// 创建带有附加数据的 JSON-RPC 错误
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    /// 创建解析错误（-32700）
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(PARSE_ERROR, message)
    }

    /// 创建无效请求错误（-32600）
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(INVALID_REQUEST, message)
    }

    /// 创建方法未找到错误（-32601）
    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self::new(METHOD_NOT_FOUND, message)
    }

    /// 创建无效参数错误（-32602）
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, message)
    }

    /// 创建内部错误（-32603）
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(INTERNAL_ERROR, message)
    }
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC 错误 [{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

// ============================================================================
// JSON-RPC 通知
// ============================================================================

/// JSON-RPC 2.0 通知结构
///
/// 通知是没有 id 字段的请求，不期望收到响应。
/// 常用于单向事件通知，例如 MCP 的 "initialized" 通知。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    /// JSON-RPC 协议版本，固定为 "2.0"
    pub jsonrpc: String,
    /// 通知的方法名称
    pub method: String,
    /// 通知参数（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    /// 创建新的 JSON-RPC 通知
    ///
    /// # 参数
    /// - `method`: 通知方法名称
    /// - `params`: 通知参数（可选）
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

// ============================================================================
// 消息枚举：统一表示所有 JSON-RPC 消息类型
// ============================================================================

/// JSON-RPC 消息类型枚举
///
/// 统一表示请求、响应和通知三种消息类型。
/// 用于传输层的消息收发和路由分发。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    /// 请求消息（包含 method 和 id）
    Request(JsonRpcRequest),
    /// 响应消息（包含 result/error 和 id）
    Response(JsonRpcResponse),
    /// 通知消息（包含 method，无 id）
    Notification(JsonRpcNotification),
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试请求的序列化与反序列化
    #[test]
    fn test_request_serialization() {
        let request = JsonRpcRequest::new(
            "initialize",
            Some(json!({"protocolVersion": "2024-11-05"})),
            RequestId::Number(1),
        );

        let serialized = serde_json::to_string(&request).expect("序列化失败");
        let deserialized: JsonRpcRequest =
            serde_json::from_str(&serialized).expect("反序列化失败");

        assert_eq!(deserialized.jsonrpc, "2.0");
        assert_eq!(deserialized.method, "initialize");
        assert_eq!(deserialized.id, RequestId::Number(1));
        assert!(deserialized.params.is_some());
    }

    /// 测试成功响应的创建
    #[test]
    fn test_success_response() {
        let response = JsonRpcResponse::success(
            RequestId::Number(1),
            json!({"protocolVersion": "2024-11-05"}),
        );

        assert!(response.result.is_some());
        assert!(response.error.is_none());
        assert_eq!(response.id, RequestId::Number(1));
    }

    /// 测试错误响应的创建
    #[test]
    fn test_error_response() {
        let error = JsonRpcError::method_not_found("未找到方法: foo");
        let response = JsonRpcResponse::error(RequestId::String("abc".to_string()), error);

        assert!(response.result.is_none());
        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, METHOD_NOT_FOUND);
    }

    /// 测试通知的序列化
    #[test]
    fn test_notification_serialization() {
        let notification = JsonRpcNotification::new("notifications/initialized", None);

        let serialized = serde_json::to_string(&notification).expect("序列化失败");
        // 通知不应包含 id 字段
        assert!(!serialized.contains("\"id\""));
        assert!(serialized.contains("\"jsonrpc\":\"2.0\""));
    }

    /// 测试各种错误码的工厂方法
    #[test]
    fn test_error_codes() {
        assert_eq!(JsonRpcError::parse_error("test").code, PARSE_ERROR);
        assert_eq!(JsonRpcError::invalid_request("test").code, INVALID_REQUEST);
        assert_eq!(JsonRpcError::method_not_found("test").code, METHOD_NOT_FOUND);
        assert_eq!(JsonRpcError::invalid_params("test").code, INVALID_PARAMS);
        assert_eq!(JsonRpcError::internal_error("test").code, INTERNAL_ERROR);
    }

    /// 测试请求 ID 的多种类型
    #[test]
    fn test_request_id_types() {
        // 数字 ID
        let num_id = RequestId::Number(42);
        let json = serde_json::to_value(&num_id).expect("序列化失败");
        assert_eq!(json, json!(42));

        // 字符串 ID
        let str_id = RequestId::String("req-001".to_string());
        let json = serde_json::to_value(&str_id).expect("序列化失败");
        assert_eq!(json, json!("req-001"));

        // 空值 ID
        let null_id = RequestId::Null;
        let json = serde_json::to_value(&null_id).expect("序列化失败");
        assert_eq!(json, json!(null));
    }

    /// 测试无参数请求的序列化（params 字段应被省略）
    #[test]
    fn test_request_without_params() {
        let request = JsonRpcRequest::new("shutdown", None, RequestId::Number(99));
        let serialized = serde_json::to_string(&request).expect("序列化失败");
        // 当 params 为 None 时，序列化后不应包含 params 字段
        assert!(!serialized.contains("\"params\""));
    }

    /// 测试带附加数据的错误
    #[test]
    fn test_error_with_data() {
        let error = JsonRpcError::with_data(
            INTERNAL_ERROR,
            "处理失败",
            json!({"detail": "堆栈溢出"}),
        );
        assert_eq!(error.code, INTERNAL_ERROR);
        assert!(error.data.is_some());
    }

    /// 测试 RequestId 的 Display 实现
    #[test]
    fn test_request_id_display() {
        assert_eq!(format!("{}", RequestId::Number(1)), "1");
        assert_eq!(format!("{}", RequestId::String("abc".into())), "abc");
        assert_eq!(format!("{}", RequestId::Null), "null");
    }

    /// 测试 JsonRpcError 的 Display 实现
    #[test]
    fn test_error_display() {
        let error = JsonRpcError::parse_error("无效 JSON");
        let display = format!("{}", error);
        assert!(display.contains("-32700"));
        assert!(display.contains("无效 JSON"));
    }
}
