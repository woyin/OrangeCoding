//! MCP 传输层实现
//!
//! 本模块定义了 MCP 协议的传输层抽象和具体实现。
//! 目前实现了基于标准输入/输出（stdio）的传输方式，
//! 使用 Content-Length 头部进行消息帧界定。

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;

use crate::error::McpError;
use crate::protocol::JsonRpcMessage;

// ============================================================================
// 传输层特征定义
// ============================================================================

/// MCP 传输层特征
///
/// 定义了消息发送和接收的异步接口。
/// 所有传输实现（stdio、HTTP SSE 等）都需要实现此特征。
#[async_trait]
pub trait Transport: Send + Sync {
    /// 发送一条 JSON-RPC 消息
    ///
    /// # 参数
    /// - `message`: 要发送的 JSON-RPC 消息
    ///
    /// # 错误
    /// 当消息序列化或写入失败时返回错误
    async fn send(&self, message: &JsonRpcMessage) -> Result<(), McpError>;

    /// 接收一条 JSON-RPC 消息
    ///
    /// 阻塞等待直到收到完整的消息。
    ///
    /// # 返回
    /// 接收到的 JSON-RPC 消息
    ///
    /// # 错误
    /// 当读取或反序列化失败时返回错误
    async fn receive(&self) -> Result<JsonRpcMessage, McpError>;

    /// 关闭传输连接
    ///
    /// 清理资源并关闭底层 IO 通道。
    async fn close(&self) -> Result<(), McpError>;
}

// ============================================================================
// 标准 IO 传输实现
// ============================================================================

/// 基于标准输入/输出的传输实现
///
/// 使用 Content-Length 头部帧协议在 stdin/stdout 上传输 JSON-RPC 消息。
/// 消息格式为：
/// ```text
/// Content-Length: <字节数>\r\n
/// \r\n
/// <JSON 正文>
/// ```
///
/// 这是 MCP 规范中推荐的本地进程间通信方式。
pub struct StdioTransport {
    /// 带缓冲的标准输入读取器（使用互斥锁保护并发访问）
    reader: Mutex<BufReader<tokio::io::Stdin>>,
    /// 带缓冲的标准输出写入器（使用互斥锁保护并发访问）
    writer: Mutex<BufWriter<tokio::io::Stdout>>,
    /// 传输连接是否已关闭
    closed: Mutex<bool>,
}

impl StdioTransport {
    /// 创建新的标准 IO 传输实例
    ///
    /// 初始化带缓冲的 stdin 读取器和 stdout 写入器。
    pub fn new() -> Self {
        Self {
            reader: Mutex::new(BufReader::new(tokio::io::stdin())),
            writer: Mutex::new(BufWriter::new(tokio::io::stdout())),
            closed: Mutex::new(false),
        }
    }

    /// 从输入流读取一条完整的消息
    ///
    /// 解析流程：
    /// 1. 逐行读取头部，查找 Content-Length
    /// 2. 遇到空行表示头部结束
    /// 3. 根据 Content-Length 读取指定字节数的 JSON 正文
    /// 4. 反序列化为 JsonRpcMessage
    async fn read_message(
        reader: &mut BufReader<tokio::io::Stdin>,
    ) -> Result<JsonRpcMessage, McpError> {
        let mut content_length: Option<usize> = None;

        // 逐行读取 HTTP 风格的头部
        loop {
            let mut header_line = String::new();
            let bytes_read = reader
                .read_line(&mut header_line)
                .await
                .map_err(|e| McpError::Transport(format!("读取头部失败: {}", e)))?;

            // 如果读取到 0 字节，表示输入流已关闭（EOF）
            if bytes_read == 0 {
                return Err(McpError::Transport("输入流已关闭（EOF）".to_string()));
            }

            // 去除行尾的换行符
            let trimmed = header_line.trim();

            // 空行表示头部结束
            if trimmed.is_empty() {
                break;
            }

            // 解析 Content-Length 头部
            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                content_length = Some(value.trim().parse::<usize>().map_err(|e| {
                    McpError::Transport(format!("无效的 Content-Length 值: {}", e))
                })?);
            }
            // 忽略其他未知头部字段
        }

        // 验证 Content-Length 是否存在
        let length = content_length
            .ok_or_else(|| McpError::Transport("缺少 Content-Length 头部".to_string()))?;

        // 根据 Content-Length 读取 JSON 正文
        let mut body_buffer = vec![0u8; length];
        reader
            .read_exact(&mut body_buffer)
            .await
            .map_err(|e| McpError::Transport(format!("读取消息正文失败: {}", e)))?;

        // 将字节缓冲区转换为 UTF-8 字符串
        let body_str = String::from_utf8(body_buffer)
            .map_err(|e| McpError::Transport(format!("消息正文不是有效的 UTF-8: {}", e)))?;

        tracing::debug!("收到消息: {}", body_str);

        // 反序列化 JSON 为消息对象
        serde_json::from_str(&body_str)
            .map_err(|e| McpError::Protocol(format!("JSON 反序列化失败: {}", e)))
    }

    /// 将消息写入输出流
    ///
    /// 写入流程：
    /// 1. 将消息序列化为 JSON 字符串
    /// 2. 计算 JSON 的字节长度
    /// 3. 写入 Content-Length 头部和空行分隔符
    /// 4. 写入 JSON 正文并刷新缓冲区
    async fn write_message(
        writer: &mut BufWriter<tokio::io::Stdout>,
        message: &JsonRpcMessage,
    ) -> Result<(), McpError> {
        // 序列化消息为 JSON
        let json_body = serde_json::to_string(message)
            .map_err(|e| McpError::Protocol(format!("JSON 序列化失败: {}", e)))?;

        tracing::debug!("发送消息: {}", json_body);

        let body_bytes = json_body.as_bytes();

        // 写入 Content-Length 头部
        let header = format!("Content-Length: {}\r\n\r\n", body_bytes.len());
        writer
            .write_all(header.as_bytes())
            .await
            .map_err(|e| McpError::Transport(format!("写入消息头部失败: {}", e)))?;

        // 写入 JSON 正文
        writer
            .write_all(body_bytes)
            .await
            .map_err(|e| McpError::Transport(format!("写入消息正文失败: {}", e)))?;

        // 刷新缓冲区，确保消息立即发送
        writer
            .flush()
            .await
            .map_err(|e| McpError::Transport(format!("刷新输出缓冲区失败: {}", e)))?;

        Ok(())
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    /// 通过 stdout 发送消息
    async fn send(&self, message: &JsonRpcMessage) -> Result<(), McpError> {
        // 检查传输是否已关闭
        let closed = self.closed.lock().await;
        if *closed {
            return Err(McpError::Transport("传输已关闭".to_string()));
        }
        drop(closed);

        let mut writer = self.writer.lock().await;
        Self::write_message(&mut writer, message).await
    }

    /// 从 stdin 接收消息
    async fn receive(&self) -> Result<JsonRpcMessage, McpError> {
        // 检查传输是否已关闭
        let closed = self.closed.lock().await;
        if *closed {
            return Err(McpError::Transport("传输已关闭".to_string()));
        }
        drop(closed);

        let mut reader = self.reader.lock().await;
        Self::read_message(&mut reader).await
    }

    /// 关闭标准 IO 传输
    async fn close(&self) -> Result<(), McpError> {
        let mut closed = self.closed.lock().await;
        *closed = true;
        tracing::info!("标准 IO 传输已关闭");
        Ok(())
    }
}

// ============================================================================
// 内存传输实现（用于测试）
// ============================================================================

/// 基于内存通道的传输实现（仅用于测试）
///
/// 使用 tokio 的 mpsc 通道在内存中传递消息，
/// 便于在不依赖真实 IO 的情况下测试 MCP 协议逻辑。
#[cfg(test)]
pub struct MemoryTransport {
    /// 发送端：用于向对端发送消息
    sender: tokio::sync::mpsc::Sender<JsonRpcMessage>,
    /// 接收端：用于从对端接收消息（互斥锁保护）
    receiver: Mutex<tokio::sync::mpsc::Receiver<JsonRpcMessage>>,
    /// 传输连接是否已关闭
    closed: Mutex<bool>,
}

#[cfg(test)]
impl MemoryTransport {
    /// 创建一对互相连接的内存传输
    ///
    /// 返回两个 MemoryTransport 实例，A 发送的消息 B 可以接收，反之亦然。
    pub fn pair() -> (Self, Self) {
        let (tx_a, rx_b) = tokio::sync::mpsc::channel(32);
        let (tx_b, rx_a) = tokio::sync::mpsc::channel(32);

        let transport_a = Self {
            sender: tx_a,
            receiver: Mutex::new(rx_a),
            closed: Mutex::new(false),
        };
        let transport_b = Self {
            sender: tx_b,
            receiver: Mutex::new(rx_b),
            closed: Mutex::new(false),
        };

        (transport_a, transport_b)
    }
}

#[cfg(test)]
#[async_trait]
impl Transport for MemoryTransport {
    async fn send(&self, message: &JsonRpcMessage) -> Result<(), McpError> {
        let closed = self.closed.lock().await;
        if *closed {
            return Err(McpError::Transport("传输已关闭".to_string()));
        }
        drop(closed);

        self.sender
            .send(message.clone())
            .await
            .map_err(|e| McpError::Transport(format!("内存通道发送失败: {}", e)))
    }

    async fn receive(&self) -> Result<JsonRpcMessage, McpError> {
        let closed = self.closed.lock().await;
        if *closed {
            return Err(McpError::Transport("传输已关闭".to_string()));
        }
        drop(closed);

        let mut receiver = self.receiver.lock().await;
        receiver
            .recv()
            .await
            .ok_or_else(|| McpError::Transport("内存通道已关闭".to_string()))
    }

    async fn close(&self) -> Result<(), McpError> {
        let mut closed = self.closed.lock().await;
        *closed = true;
        Ok(())
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RequestId};
    use serde_json::json;

    /// 测试内存传输的基本消息收发
    #[tokio::test]
    async fn test_memory_transport_send_receive() {
        let (transport_a, transport_b) = MemoryTransport::pair();

        // 从 A 发送请求消息
        let request = JsonRpcRequest::new(
            "test/method",
            Some(json!({"key": "value"})),
            RequestId::Number(1),
        );
        let message = JsonRpcMessage::Request(request);

        transport_a.send(&message).await.expect("发送失败");

        // 在 B 端接收消息
        let received = transport_b.receive().await.expect("接收失败");
        match received {
            JsonRpcMessage::Request(req) => {
                assert_eq!(req.method, "test/method");
                assert_eq!(req.id, RequestId::Number(1));
            }
            _ => panic!("期望收到请求消息"),
        }
    }

    /// 测试内存传输的双向通信
    #[tokio::test]
    async fn test_memory_transport_bidirectional() {
        let (transport_a, transport_b) = MemoryTransport::pair();

        // A 向 B 发送请求
        let request =
            JsonRpcMessage::Request(JsonRpcRequest::new("ping", None, RequestId::Number(1)));
        transport_a.send(&request).await.expect("A 发送失败");

        // B 接收请求
        let _ = transport_b.receive().await.expect("B 接收失败");

        // B 向 A 发送响应
        let response = JsonRpcMessage::Response(JsonRpcResponse::success(
            RequestId::Number(1),
            json!("pong"),
        ));
        transport_b.send(&response).await.expect("B 发送失败");

        // A 接收响应
        let received = transport_a.receive().await.expect("A 接收失败");
        match received {
            JsonRpcMessage::Response(resp) => {
                assert_eq!(resp.result, Some(json!("pong")));
            }
            _ => panic!("期望收到响应消息"),
        }
    }

    /// 测试关闭传输后发送消息应报错
    #[tokio::test]
    async fn test_transport_close() {
        let (transport_a, _transport_b) = MemoryTransport::pair();

        transport_a.close().await.expect("关闭失败");

        // 关闭后发送消息应返回错误
        let message = JsonRpcMessage::Notification(JsonRpcNotification::new("test", None));
        let result = transport_a.send(&message).await;
        assert!(result.is_err());
    }

    /// 测试通知消息的传输
    #[tokio::test]
    async fn test_notification_transport() {
        let (transport_a, transport_b) = MemoryTransport::pair();

        let notification = JsonRpcMessage::Notification(JsonRpcNotification::new(
            "notifications/initialized",
            None,
        ));
        transport_a.send(&notification).await.expect("发送失败");

        let received = transport_b.receive().await.expect("接收失败");
        match received {
            JsonRpcMessage::Notification(notif) => {
                assert_eq!(notif.method, "notifications/initialized");
            }
            _ => panic!("期望收到通知消息"),
        }
    }
}
