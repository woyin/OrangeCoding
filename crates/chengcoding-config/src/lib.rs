//! # ChengCoding 配置管理模块
//!
//! 本模块提供 ChengCoding CLI 工具的完整配置管理功能，包括：
//! - 多层配置加载与合并（文件、环境变量、命令行参数、默认值）
//! - API 密钥的加密存储
//! - 配置的热重载
//! - XDG 标准目录支持

/// 主配置系统模块
pub mod config;

/// 加密存储模块，用于安全存储 API 密钥等敏感信息
pub mod crypto;

/// 多工具配置发现模块
pub mod discovery;

/// AI 模型配置模块
pub mod models_config;

/// 配置源与分层合并模块
pub mod source;

/// JSONC（JSON with Comments）解析模块
pub mod jsonc;

// 重新导出常用类型，方便外部使用
pub use config::{
    AgentConfig, AiConfig, CeairConfig, ConfigManager, LoggingConfig, ToolsConfig, TuiConfig,
};
pub use crypto::CryptoStore;
pub use discovery::{ConfigDiscovery, ConfigProvider, DiscoveredItem, DiscoveryType};
pub use models_config::{
    ApiType, AuthType, ModelCost, ModelDefinition, ModelsConfig, ProviderConfig,
};
pub use source::{ConfigLayer, ConfigSource, LayeredConfig};
