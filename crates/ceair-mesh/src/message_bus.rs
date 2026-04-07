//! 消息总线模块
//!
//! 提供基于 `tokio::sync::broadcast` 的发布/订阅消息系统，
//! 支持代理间的一对一消息、广播消息和基于主题的消息过滤。

use std::fmt;

use ceair_core::AgentId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// 默认广播通道容量
// ---------------------------------------------------------------------------

/// 默认的广播通道容量
const DEFAULT_CHANNEL_CAPACITY: usize = 256;

// ---------------------------------------------------------------------------
// 总线消息
// ---------------------------------------------------------------------------

/// 总线消息 - 代理间通信的消息格式
///
/// 每条消息包含发送者、接收者（可选，`None` 表示广播）、
/// 主题、负载数据和时间戳。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BusMessage {
    /// 消息发送者的代理 ID
    pub from: AgentId,
    /// 消息接收者的代理 ID，`None` 表示广播给所有订阅者
    pub to: Option<AgentId>,
    /// 消息主题，用于过滤和路由
    pub topic: String,
    /// 消息负载，使用 JSON 值以支持任意数据结构
    pub payload: Value,
    /// 消息创建的时间戳
    pub timestamp: DateTime<Utc>,
}

impl BusMessage {
    /// 创建一个广播消息（发送给所有订阅者）
    pub fn broadcast(from: AgentId, topic: impl Into<String>, payload: Value) -> Self {
        Self {
            from,
            to: None,
            topic: topic.into(),
            payload,
            timestamp: Utc::now(),
        }
    }

    /// 创建一个定向消息（发送给指定代理）
    pub fn directed(
        from: AgentId,
        to: AgentId,
        topic: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            from,
            to: Some(to),
            topic: topic.into(),
            payload,
            timestamp: Utc::now(),
        }
    }

    /// 检查此消息是否为广播消息
    pub fn is_broadcast(&self) -> bool {
        self.to.is_none()
    }

    /// 检查此消息是否发送给指定代理
    pub fn is_for(&self, agent_id: &AgentId) -> bool {
        match &self.to {
            // 广播消息对所有代理可见
            None => true,
            // 定向消息只对目标代理可见
            Some(target) => target == agent_id,
        }
    }
}

impl fmt::Display for BusMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.to {
            Some(to) => write!(f, "[{}] {} -> {}: {}", self.topic, self.from, to, self.payload),
            None => write!(f, "[{}] {} -> *: {}", self.topic, self.from, self.payload),
        }
    }
}

// ---------------------------------------------------------------------------
// 消息总线
// ---------------------------------------------------------------------------

/// 消息总线 - 代理间的发布/订阅通信系统
///
/// 基于 `tokio::sync::broadcast` 实现，支持多生产者多消费者模式。
/// 每个订阅者都会收到所有已发布的消息，可以在接收端按主题或目标过滤。
///
/// # 示例
///
/// ```rust
/// use ceair_mesh::message_bus::{MessageBus, BusMessage};
/// use ceair_core::AgentId;
///
/// #[tokio::main]
/// async fn main() {
///     let bus = MessageBus::new();
///     let mut rx = bus.subscribe();
///
///     let sender = AgentId::new();
///     bus.publish(BusMessage::broadcast(
///         sender,
///         "task.created",
///         serde_json::json!({"task": "编写代码"}),
///     ));
/// }
/// ```
pub struct MessageBus {
    /// 广播发送端
    sender: broadcast::Sender<BusMessage>,
}

impl MessageBus {
    /// 创建一个新的消息总线（使用默认容量）
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    /// 创建一个指定容量的消息总线
    ///
    /// `capacity` 指定通道可以缓存的最大消息数量。
    /// 当缓存满且消费者没有及时消费时，最旧的消息会被丢弃。
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        debug!(capacity = capacity, "创建消息总线");
        Self { sender }
    }

    /// 发布一条消息到总线
    ///
    /// 所有当前的订阅者都会收到此消息的副本。
    /// 如果当前没有订阅者，消息会被丢弃。
    pub fn publish(&self, message: BusMessage) {
        debug!(
            topic = %message.topic,
            from = %message.from,
            "发布消息到总线"
        );
        match self.sender.send(message) {
            Ok(receiver_count) => {
                debug!(receiver_count = receiver_count, "消息已发送给订阅者");
            }
            Err(_) => {
                warn!("发布消息失败：当前没有活跃的订阅者");
            }
        }
    }

    /// 订阅总线上的所有消息
    ///
    /// 返回一个接收器，可以通过它接收所有后续发布的消息。
    pub fn subscribe(&self) -> broadcast::Receiver<BusMessage> {
        debug!("新增一个消息订阅者");
        self.sender.subscribe()
    }

    /// 订阅指定主题的消息
    ///
    /// 返回一个异步流，只包含匹配指定主题的消息。
    /// 内部使用 `tokio::spawn` 进行过滤转发。
    pub fn subscribe_topic(&self, topic: impl Into<String>) -> broadcast::Receiver<BusMessage> {
        let topic = topic.into();
        debug!(topic = %topic, "订阅指定主题的消息");

        // 创建一个新的广播通道用于过滤后的消息
        let (filtered_tx, filtered_rx) = broadcast::channel(DEFAULT_CHANNEL_CAPACITY);
        let mut source_rx = self.sender.subscribe();

        // 启动后台任务进行主题过滤
        tokio::spawn(async move {
            loop {
                match source_rx.recv().await {
                    Ok(msg) => {
                        if msg.topic == topic {
                            // 如果没有接收者了，退出过滤任务
                            if filtered_tx.send(msg).is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "主题过滤器落后，跳过了部分消息");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        filtered_rx
    }

    /// 获取当前订阅者数量
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MessageBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageBus")
            .field("subscriber_count", &self.sender.receiver_count())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn 测试创建广播消息() {
        let from = AgentId::new();
        let msg = BusMessage::broadcast(from.clone(), "test.topic", json!({"key": "value"}));

        assert_eq!(msg.from, from);
        assert!(msg.to.is_none());
        assert_eq!(msg.topic, "test.topic");
        assert!(msg.is_broadcast());
    }

    #[test]
    fn 测试创建定向消息() {
        let from = AgentId::new();
        let to = AgentId::new();
        let msg = BusMessage::directed(from.clone(), to.clone(), "direct", json!("你好"));

        assert_eq!(msg.from, from);
        assert_eq!(msg.to, Some(to.clone()));
        assert!(!msg.is_broadcast());
        assert!(msg.is_for(&to));
    }

    #[test]
    fn 测试消息可见性判断() {
        let sender = AgentId::new();
        let target = AgentId::new();
        let other = AgentId::new();

        // 广播消息对所有代理可见
        let broadcast_msg = BusMessage::broadcast(sender.clone(), "topic", json!(null));
        assert!(broadcast_msg.is_for(&target));
        assert!(broadcast_msg.is_for(&other));

        // 定向消息只对目标可见
        let direct_msg =
            BusMessage::directed(sender.clone(), target.clone(), "topic", json!(null));
        assert!(direct_msg.is_for(&target));
        assert!(!direct_msg.is_for(&other));
    }

    #[test]
    fn 测试消息的显示格式() {
        let from = AgentId::new();
        let msg = BusMessage::broadcast(from, "event.test", json!("数据"));
        let display = format!("{msg}");
        assert!(display.contains("event.test"));
        assert!(display.contains("*"));
    }

    #[tokio::test]
    async fn 测试发布和接收消息() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe();

        let sender = AgentId::new();
        let msg = BusMessage::broadcast(sender.clone(), "hello", json!("世界"));
        bus.publish(msg);

        let received = rx.recv().await.unwrap();
        assert_eq!(received.from, sender);
        assert_eq!(received.topic, "hello");
        assert_eq!(received.payload, json!("世界"));
    }

    #[tokio::test]
    async fn 测试多个订阅者接收消息() {
        let bus = MessageBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        let sender = AgentId::new();
        bus.publish(BusMessage::broadcast(
            sender,
            "multi",
            json!("多播"),
        ));

        // 两个订阅者都应该收到消息
        let msg1 = rx1.recv().await.unwrap();
        let msg2 = rx2.recv().await.unwrap();
        assert_eq!(msg1.topic, "multi");
        assert_eq!(msg2.topic, "multi");
    }

    #[tokio::test]
    async fn 测试没有订阅者时发布不会崩溃() {
        let bus = MessageBus::new();
        let sender = AgentId::new();
        // 没有订阅者，发布消息不应该 panic
        bus.publish(BusMessage::broadcast(sender, "no_sub", json!(null)));
    }

    #[test]
    fn 测试订阅者数量() {
        let bus = MessageBus::new();
        assert_eq!(bus.subscriber_count(), 0);

        let _rx1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);

        drop(_rx1);
        // 注意：broadcast 的 receiver_count 在 drop 后可能不会立即更新
        // 这里只验证基本功能
    }

    #[tokio::test]
    async fn 测试主题过滤订阅() {
        let bus = MessageBus::new();
        let mut filtered_rx = bus.subscribe_topic("important");

        let sender = AgentId::new();

        // 发布一条不匹配主题的消息
        bus.publish(BusMessage::broadcast(
            sender.clone(),
            "other",
            json!("不相关"),
        ));

        // 发布一条匹配主题的消息
        bus.publish(BusMessage::broadcast(
            sender.clone(),
            "important",
            json!("重要消息"),
        ));

        // 等待过滤后的消息
        let received = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            filtered_rx.recv(),
        )
        .await;

        assert!(received.is_ok());
        let msg = received.unwrap().unwrap();
        assert_eq!(msg.topic, "important");
        assert_eq!(msg.payload, json!("重要消息"));
    }

    #[test]
    fn 测试默认构造() {
        let bus = MessageBus::default();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn 测试调试输出() {
        let bus = MessageBus::new();
        let debug_str = format!("{:?}", bus);
        assert!(debug_str.contains("MessageBus"));
    }

    #[test]
    fn 测试消息的序列化和反序列化() {
        let msg = BusMessage::broadcast(AgentId::new(), "test", json!({"data": 42}));
        let json_str = serde_json::to_string(&msg).unwrap();
        let deserialized: BusMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.topic, "test");
        assert_eq!(deserialized.payload, json!({"data": 42}));
    }

    #[test]
    fn 测试指定容量创建() {
        let bus = MessageBus::with_capacity(512);
        assert_eq!(bus.subscriber_count(), 0);
    }
}
