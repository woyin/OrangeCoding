//! # Agent间直接通信模块
//!
//! 实现Agent之间的点对点消息传递，支持请求-响应和广播模式。

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// 默认通道容量
// ---------------------------------------------------------------------------

/// 默认的通信通道容量
const DEFAULT_COMM_CAPACITY: usize = 128;

// ---------------------------------------------------------------------------
// Agent 消息类型
// ---------------------------------------------------------------------------

/// Agent间通信的消息类型
///
/// 定义了Agent之间可以传递的各种消息格式，涵盖状态更新、
/// 知识共享、求助请求和任务结果等场景。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AgentMessage {
    /// 状态更新消息，用于通知其他Agent当前进度
    StatusUpdate {
        /// 进度百分比（0-100）
        progress: u8,
        /// 描述性状态信息
        message: String,
    },

    /// 知识共享消息，用于在Agent间传递键值对知识
    WisdomShare {
        /// 知识的键名
        key: String,
        /// 知识的值
        value: String,
    },

    /// 求助请求消息，当Agent遇到困难时向其他Agent求助
    HelpRequest {
        /// 发起求助的Agent标识
        from: String,
        /// 相关的任务 ID
        task_id: String,
        /// 问题描述
        description: String,
    },

    /// 任务结果消息，报告任务执行的最终结果
    TaskResult {
        /// 任务 ID
        task_id: String,
        /// 是否成功
        success: bool,
        /// 输出内容
        output: String,
    },
}

impl fmt::Display for AgentMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentMessage::StatusUpdate { progress, message } => {
                write!(f, "状态更新[{}%]: {}", progress, message)
            }
            AgentMessage::WisdomShare { key, value } => {
                write!(f, "知识共享: {} = {}", key, value)
            }
            AgentMessage::HelpRequest {
                from,
                task_id,
                description,
            } => {
                write!(f, "求助[{}] 来自 {}: {}", task_id, from, description)
            }
            AgentMessage::TaskResult {
                task_id,
                success,
                output,
            } => {
                let status = if *success { "成功" } else { "失败" };
                write!(f, "任务结果[{}] {}: {}", task_id, status, output)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 消息信封
// ---------------------------------------------------------------------------

/// 消息信封 - 包装 AgentMessage 的传输容器
///
/// 提供消息的路由信息（发送者、接收者）和元数据（时间戳）。
/// `to` 为 `None` 时表示广播消息。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    /// 消息发送者的标识
    pub from: String,
    /// 消息接收者的标识，`None` 表示广播
    pub to: Option<String>,
    /// 消息内容
    pub message: AgentMessage,
    /// 发送时间戳
    pub timestamp: DateTime<Utc>,
}

impl Envelope {
    /// 创建一个定向消息信封
    pub fn directed(from: impl Into<String>, to: impl Into<String>, message: AgentMessage) -> Self {
        Self {
            from: from.into(),
            to: Some(to.into()),
            message,
            timestamp: Utc::now(),
        }
    }

    /// 创建一个广播消息信封
    pub fn broadcast(from: impl Into<String>, message: AgentMessage) -> Self {
        Self {
            from: from.into(),
            to: None,
            message,
            timestamp: Utc::now(),
        }
    }

    /// 判断该消息是否为广播消息
    pub fn is_broadcast(&self) -> bool {
        self.to.is_none()
    }

    /// 判断该消息是否发送给指定Agent
    pub fn is_for(&self, agent_id: &str) -> bool {
        match &self.to {
            None => true,
            Some(target) => target == agent_id,
        }
    }
}

impl fmt::Display for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.to {
            Some(to) => write!(f, "{} -> {}: {}", self.from, to, self.message),
            None => write!(f, "{} -> *: {}", self.from, self.message),
        }
    }
}

// ---------------------------------------------------------------------------
// Agent 通信总线
// ---------------------------------------------------------------------------

/// Agent通信总线 - 管理Agent之间的直接通信
///
/// 基于 `tokio::sync::broadcast` 实现，维护Agent的订阅关系，
/// 支持定向发送和广播两种模式。
pub struct AgentCommBus {
    /// 广播通道发送端
    sender: broadcast::Sender<Envelope>,
    /// 已注册的Agent ID 集合（用于验证）
    registered_agents: parking_lot::RwLock<HashMap<String, bool>>,
}

impl AgentCommBus {
    /// 创建一个新的Agent通信总线
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_COMM_CAPACITY)
    }

    /// 创建一个指定容量的Agent通信总线
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        debug!(capacity = capacity, "创建Agent通信总线");
        Self {
            sender,
            registered_agents: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// 注册Agent并订阅消息通道
    ///
    /// 返回该Agent的消息接收器。Agent需要通过此接收器来接收消息。
    pub fn subscribe(&self, agent_id: impl Into<String>) -> broadcast::Receiver<Envelope> {
        let id = agent_id.into();
        self.registered_agents.write().insert(id.clone(), true);
        debug!(agent_id = %id, "Agent订阅通信总线");
        self.sender.subscribe()
    }

    /// 向指定Agent发送消息
    ///
    /// 消息会通过广播通道发送，接收方需自行过滤。
    pub fn send(&self, from: impl Into<String>, to: impl Into<String>, message: AgentMessage) {
        let envelope = Envelope::directed(from, to, message);
        debug!(from = %envelope.from, to = ?envelope.to, "发送定向消息");
        match self.sender.send(envelope) {
            Ok(n) => debug!(receivers = n, "定向消息已发送"),
            Err(_) => warn!("发送消息失败：没有活跃的订阅者"),
        }
    }

    /// 向所有Agent广播消息
    pub fn broadcast(&self, from: impl Into<String>, message: AgentMessage) {
        let envelope = Envelope::broadcast(from, message);
        debug!(from = %envelope.from, "广播消息");
        match self.sender.send(envelope) {
            Ok(n) => debug!(receivers = n, "广播消息已发送"),
            Err(_) => warn!("广播消息失败：没有活跃的订阅者"),
        }
    }

    /// 获取当前订阅者数量
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// 获取已注册的Agent数量
    pub fn registered_count(&self) -> usize {
        self.registered_agents.read().len()
    }

    /// 检查指定Agent是否已注册
    pub fn is_registered(&self, agent_id: &str) -> bool {
        self.registered_agents.read().contains_key(agent_id)
    }
}

impl Default for AgentCommBus {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AgentCommBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentCommBus")
            .field("subscriber_count", &self.sender.receiver_count())
            .field("registered_count", &self.registered_count())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试创建状态更新消息() {
        let msg = AgentMessage::StatusUpdate {
            progress: 50,
            message: "正在编译".to_string(),
        };
        let display = format!("{msg}");
        assert!(display.contains("50%"));
        assert!(display.contains("正在编译"));
    }

    #[test]
    fn 测试创建知识共享消息() {
        let msg = AgentMessage::WisdomShare {
            key: "最佳实践".to_string(),
            value: "使用迭代器而非索引循环".to_string(),
        };
        let display = format!("{msg}");
        assert!(display.contains("最佳实践"));
    }

    #[test]
    fn 测试创建求助请求消息() {
        let msg = AgentMessage::HelpRequest {
            from: "agent-a".to_string(),
            task_id: "task-001".to_string(),
            description: "无法解析配置文件".to_string(),
        };
        let display = format!("{msg}");
        assert!(display.contains("agent-a"));
        assert!(display.contains("task-001"));
    }

    #[test]
    fn 测试创建任务结果消息() {
        let msg = AgentMessage::TaskResult {
            task_id: "task-002".to_string(),
            success: true,
            output: "编译通过".to_string(),
        };
        let display = format!("{msg}");
        assert!(display.contains("成功"));
        assert!(display.contains("编译通过"));
    }

    #[test]
    fn 测试信封定向消息() {
        let envelope = Envelope::directed(
            "agent-a",
            "agent-b",
            AgentMessage::StatusUpdate {
                progress: 100,
                message: "完成".to_string(),
            },
        );
        assert!(!envelope.is_broadcast());
        assert!(envelope.is_for("agent-b"));
        assert!(!envelope.is_for("agent-c"));
    }

    #[test]
    fn 测试信封广播消息() {
        let envelope = Envelope::broadcast(
            "agent-a",
            AgentMessage::WisdomShare {
                key: "提示".to_string(),
                value: "记得写测试".to_string(),
            },
        );
        assert!(envelope.is_broadcast());
        assert!(envelope.is_for("agent-b"));
        assert!(envelope.is_for("agent-c"));
    }

    #[tokio::test]
    async fn 测试通信总线定向发送() {
        let bus = AgentCommBus::new();
        let mut rx = bus.subscribe("agent-b");

        bus.send(
            "agent-a",
            "agent-b",
            AgentMessage::StatusUpdate {
                progress: 75,
                message: "处理中".to_string(),
            },
        );

        let envelope = rx.recv().await.unwrap();
        assert_eq!(envelope.from, "agent-a");
        assert!(envelope.is_for("agent-b"));
        if let AgentMessage::StatusUpdate { progress, .. } = &envelope.message {
            assert_eq!(*progress, 75);
        } else {
            panic!("消息类型不匹配");
        }
    }

    #[tokio::test]
    async fn 测试通信总线广播() {
        let bus = AgentCommBus::new();
        let mut rx1 = bus.subscribe("agent-a");
        let mut rx2 = bus.subscribe("agent-b");

        bus.broadcast(
            "agent-c",
            AgentMessage::WisdomShare {
                key: "全局通知".to_string(),
                value: "系统即将维护".to_string(),
            },
        );

        let env1 = rx1.recv().await.unwrap();
        let env2 = rx2.recv().await.unwrap();
        assert!(env1.is_broadcast());
        assert!(env2.is_broadcast());
        assert_eq!(env1.from, "agent-c");
        assert_eq!(env2.from, "agent-c");
    }

    #[test]
    fn 测试通信总线注册和订阅() {
        let bus = AgentCommBus::new();
        assert_eq!(bus.subscriber_count(), 0);
        assert_eq!(bus.registered_count(), 0);

        let _rx = bus.subscribe("agent-a");
        assert_eq!(bus.subscriber_count(), 1);
        assert_eq!(bus.registered_count(), 1);
        assert!(bus.is_registered("agent-a"));
        assert!(!bus.is_registered("agent-b"));
    }

    #[tokio::test]
    async fn 测试无订阅者时发送不崩溃() {
        let bus = AgentCommBus::new();
        // 没有订阅者时发送消息不应panic
        bus.send(
            "agent-a",
            "agent-b",
            AgentMessage::TaskResult {
                task_id: "t1".to_string(),
                success: false,
                output: "超时".to_string(),
            },
        );
        bus.broadcast(
            "agent-a",
            AgentMessage::StatusUpdate {
                progress: 0,
                message: "初始化".to_string(),
            },
        );
    }

    #[test]
    fn 测试消息序列化与反序列化() {
        let msg = AgentMessage::HelpRequest {
            from: "agent-x".to_string(),
            task_id: "task-99".to_string(),
            description: "需要帮助处理错误".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn 测试信封显示格式() {
        let env = Envelope::directed(
            "a",
            "b",
            AgentMessage::StatusUpdate {
                progress: 10,
                message: "开始".to_string(),
            },
        );
        let display = format!("{env}");
        assert!(display.contains("a"));
        assert!(display.contains("b"));

        let broadcast_env = Envelope::broadcast(
            "a",
            AgentMessage::StatusUpdate {
                progress: 10,
                message: "开始".to_string(),
            },
        );
        let display2 = format!("{broadcast_env}");
        assert!(display2.contains("*"));
    }

    #[test]
    fn 测试默认构造() {
        let bus = AgentCommBus::default();
        assert_eq!(bus.subscriber_count(), 0);
        assert_eq!(bus.registered_count(), 0);
    }
}
