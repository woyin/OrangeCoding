//! CEAIR 多代理网格系统
//!
//! `ceair-mesh` 是 CEAIR 系统的多代理协调 crate，负责管理代理之间的
//! 通信、协作和任务编排。提供了共享状态、消息总线、代理注册、
//! 模型路由、角色系统和任务编排等核心功能。
//!
//! # 模块结构
//!
//! - [`shared_state`] - 线程安全的共享状态存储，支持 TTL
//! - [`message_bus`] - 基于发布/订阅的代理间消息总线
//! - [`agent_registry`] - 代理注册表，管理代理的生命周期
//! - [`model_router`] - 动态模型路由，根据任务类型选择最佳模型
//! - [`role_system`] - 角色定义系统，管理代理角色和权限
//! - [`task_orchestrator`] - DAG 工作流任务编排器
//!
//! # 使用示例
//!
//! ```rust
//! use ceair_mesh::shared_state::SharedState;
//! use ceair_mesh::agent_registry::{AgentRegistry, AgentInfo};
//! use ceair_core::{AgentId, AgentRole, AgentStatus};
//!
//! // 创建共享状态
//! let state = SharedState::new();
//! state.set("current_task", serde_json::json!("编写排序函数"));
//!
//! // 注册代理
//! let registry = AgentRegistry::new();
//! let agent_id = AgentId::new();
//! let info = AgentInfo::new(agent_id.clone(), "编码代理", AgentRole::Coder);
//! registry.register(info);
//! ```

/// 共享状态模块 - 线程安全的键值存储，支持 TTL 过期
pub mod shared_state;

/// 消息总线模块 - 基于广播的代理间通信系统
pub mod message_bus;

/// 代理注册表模块 - 代理的注册、发现和状态管理
pub mod agent_registry;

/// 模型路由模块 - 根据任务类型动态选择 AI 模型
pub mod model_router;

/// 角色系统模块 - 角色定义、权限和系统提示词管理
pub mod role_system;

/// 任务编排器模块 - 基于 DAG 的任务依赖管理和调度
pub mod task_orchestrator;

// ---------------------------------------------------------------------------
// 便捷的重导出 - 让常用类型可以直接从 crate 根引用
// ---------------------------------------------------------------------------

/// 重导出共享状态
pub use shared_state::SharedState;

/// 重导出消息总线核心类型
pub use message_bus::{BusMessage, MessageBus};

/// 重导出代理注册表核心类型
pub use agent_registry::{AgentInfo, AgentRegistry};

/// 重导出模型路由核心类型
pub use model_router::{ModelRouter, RoutingDecision, RoutingRule, TaskType};

/// 重导出角色系统核心类型
pub use role_system::{RoleDefinition, RoleRegistry};

/// 重导出任务编排器核心类型
pub use task_orchestrator::{Task, TaskId, TaskOrchestrator, TaskStatus};
