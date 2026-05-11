//! # 编排工作流模块
//!
//! 实现多Agent协作的工作流引擎，包含规划、执行、会话连续性等核心流程。

/// Atlas 执行编排
pub mod atlas;
/// Boulder 会话连续性系统
pub mod boulder;
/// Goal 自主迭代循环（Planning → Executing → Verifying → Replan/Done）
pub mod goal;
/// Prometheus 规划工作流
pub mod prometheus;
/// UltraWork 全自动模式
pub mod ultrawork;

/// Metadata key indicating autonomous execution mode
pub const EXECUTION_MODE_KEY: &str = "execution_mode";
/// Metadata value for autonomous mode
pub const EXECUTION_MODE_AUTONOMOUS: &str = "autonomous";
