//! AI 服务商具体适配器模块
//!
//! 本模块包含各个 AI 服务商的适配器实现：
//! - `deepseek` — DeepSeek 深度求索
//! - `qianwen` — 通义千问（阿里云 DashScope）
//! - `wenxin` — 文心一言（百度智能云）

/// DeepSeek 深度求索适配器
pub mod deepseek;

/// 通义千问适配器（阿里云 DashScope）
pub mod qianwen;

/// 文心一言适配器（百度智能云）
pub mod wenxin;
