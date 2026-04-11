//! # CEAIR 代理核心模块
//!
//! 本 crate 实现了 AI 编码代理的核心事件循环、上下文管理和工具执行。
//!
//! ## 模块结构
//!
//! - [`agent_loop`] - 代理主事件循环，协调 AI 调用与工具执行
//! - [`context`] - 代理上下文管理，维护对话历史和工作目录
//! - [`executor`] - 工具执行器，支持并行执行和超时控制
//!
//! ## 使用示例
//!
//! ```no_run
//! use ceair_agent::{AgentLoop, AgentLoopConfig, AgentContext};
//! use ceair_core::{AgentId, SessionId};
//! use std::sync::Arc;
//!
//! // 创建代理上下文
//! let context = AgentContext::new(SessionId::new(), std::path::PathBuf::from("."));
//!
//! // 配置代理循环
//! let config = AgentLoopConfig::default();
//! ```

/// 代理统一配置模块
pub mod agent_config;

/// 代理主事件循环模块
pub mod agent_loop;

/// 专业 Agent 定义模块（11 个内置 Agent）
pub mod agents;

/// 类别路由模块（意图驱动的模型选择）
pub mod category;

/// 上下文压缩模块
pub mod compaction;

/// 代理上下文管理模块
pub mod context;

/// 工具执行器模块
pub mod executor;

/// Hashline 编辑模块
pub mod hashline;

/// 钩子系统模块
pub mod hooks;

/// 意图门控模块（请求意图分类）
pub mod intent_gate;

/// 技能系统模块
pub mod skills;

/// 自定义工具模块
pub mod custom_tools;

/// TTSR（时间旅行流式规则）模块
pub mod ttsr;

/// 记忆系统模块
pub mod memory;

/// Token 预算状态机模块
pub mod token_budget;

/// 任务系统模块 — 任务 ID、状态机、注册表
pub mod task_system;

/// Buddy System — 确定性伙伴身份生成
pub mod buddy;

/// KAIROS — Post-Sampling Hook 系统
pub mod kairos;

/// 上下文感知 Tips 系统
pub mod tips;

/// 工具使用摘要生成器
pub mod tool_summary;

/// 取消令牌层级系统
pub mod cancellation;

/// 消息分组模块 — 按 API 轮次分组
pub mod message_group;

/// 会话记忆（短期记忆）
pub mod session_memory;

/// 自动压缩触发器与断路器
pub mod auto_compact;

/// AutoDream 门控与锁
pub mod auto_dream;

/// 代理管道模块
pub mod pipeline;

/// 编排工作流模块
pub mod workflows;

// ---------------------------------------------------------------------------
// 公共重导出 - 便于外部直接引用核心类型
// ---------------------------------------------------------------------------

/// 重导出代理事件循环相关类型
pub use agent_loop::{AgentLoop, AgentLoopConfig, AgentLoopResult};

/// 重导出代理上下文
pub use context::AgentContext;

/// 重导出工具执行器
pub use executor::ToolExecutor;
