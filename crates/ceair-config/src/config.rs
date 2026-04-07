//! # 主配置系统
//!
//! 定义 CEAIR 的所有配置结构体，以及用于加载、保存和管理配置的 `ConfigManager`。
//! 支持从 XDG 配置目录加载 TOML 格式的配置文件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use ceair_core::CeairError;

// ---------------------------------------------------------------------------
// AI 配置
// ---------------------------------------------------------------------------

/// AI 模型提供商的配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiConfig {
    /// AI 提供商名称（例如 "openai"、"anthropic"）
    pub provider: String,

    /// API 密钥（可选，建议使用加密存储）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// 使用的模型名称
    pub model: String,

    /// 采样温度，控制输出随机性（0.0 - 2.0）
    pub temperature: f64,

    /// 单次请求的最大令牌数
    pub max_tokens: u32,

    /// 自定义 API 基础 URL（可选，用于代理或自托管服务）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// 为 `AiConfig` 提供合理的默认值
impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            api_key: None,
            model: "gpt-4".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            base_url: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Agent（智能体）配置
// ---------------------------------------------------------------------------

/// 智能体行为相关配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AgentConfig {
    /// 智能体最大迭代次数，防止无限循环
    pub max_iterations: u32,

    /// 单次操作超时时间（秒）
    pub timeout_secs: u64,

    /// 是否自动批准工具调用（不需要用户确认）
    pub auto_approve_tools: bool,
}

/// 为 `AgentConfig` 提供合理的默认值
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            timeout_secs: 300,
            auto_approve_tools: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 工具配置
// ---------------------------------------------------------------------------

/// 文件系统工具的安全配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ToolsConfig {
    /// 允许访问的路径列表
    #[serde(default)]
    pub allowed_paths: Vec<PathBuf>,

    /// 禁止访问的路径列表（优先级高于允许列表）
    #[serde(default)]
    pub blocked_paths: Vec<PathBuf>,

    /// 允许读取的最大文件大小（字节）
    pub max_file_size: usize,
}

/// 为 `ToolsConfig` 提供合理的默认值
impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            allowed_paths: vec![PathBuf::from(".")],
            blocked_paths: vec![],
            // 默认最大文件大小为 10MB
            max_file_size: 10 * 1024 * 1024,
        }
    }
}

// ---------------------------------------------------------------------------
// TUI（终端界面）配置
// ---------------------------------------------------------------------------

/// 终端用户界面的显示配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TuiConfig {
    /// 主题名称（例如 "dark"、"light"）
    pub theme: String,

    /// 是否显示令牌使用量
    pub show_token_usage: bool,

    /// 是否显示时间戳
    pub show_timestamps: bool,
}

/// 为 `TuiConfig` 提供合理的默认值
impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            show_token_usage: true,
            show_timestamps: true,
        }
    }
}

// ---------------------------------------------------------------------------
// 日志配置
// ---------------------------------------------------------------------------

/// 日志记录相关配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LoggingConfig {
    /// 日志级别（trace / debug / info / warn / error）
    pub level: String,

    /// 日志文件路径（可选，不设置时仅输出到标准错误）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,

    /// 是否使用 JSON 格式输出日志
    pub json_format: bool,
}

/// 为 `LoggingConfig` 提供合理的默认值
impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            json_format: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 顶层配置结构体
// ---------------------------------------------------------------------------

/// CEAIR 的顶层配置，聚合所有子配置模块
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CeairConfig {
    /// AI 模型提供商配置
    #[serde(default)]
    pub ai: AiConfig,

    /// 智能体行为配置
    #[serde(default)]
    pub agent: AgentConfig,

    /// 文件系统工具配置
    #[serde(default)]
    pub tools: ToolsConfig,

    /// 终端界面配置
    #[serde(default)]
    pub tui: TuiConfig,

    /// 日志记录配置
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// 为 `CeairConfig` 提供默认值，各子配置均使用自己的默认值
impl Default for CeairConfig {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
            agent: AgentConfig::default(),
            tools: ToolsConfig::default(),
            tui: TuiConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl CeairConfig {
    /// 从 TOML 字符串解析配置
    pub fn from_toml(content: &str) -> ceair_core::Result<Self> {
        toml::from_str(content)
            .map_err(|e| CeairError::config(format!("TOML 解析失败: {e}")))
    }

    /// 将配置序列化为 TOML 字符串
    pub fn to_toml(&self) -> ceair_core::Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| CeairError::serialization(format!("TOML 序列化失败: {e}")))
    }

    /// 将配置序列化为 JSON 字符串
    pub fn to_json(&self) -> ceair_core::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| CeairError::serialization(format!("JSON 序列化失败: {e}")))
    }
}

// ---------------------------------------------------------------------------
// 配置管理器
// ---------------------------------------------------------------------------

/// 配置管理器，负责配置的加载、保存、更新和热重载
///
/// 使用 `Arc<RwLock<>>` 确保配置可以在多线程环境中安全共享和修改。
pub struct ConfigManager {
    /// 当前生效的配置（线程安全的读写锁）
    config: Arc<RwLock<CeairConfig>>,

    /// 配置文件所在目录
    config_dir: PathBuf,

    /// 配置文件的完整路径
    config_path: PathBuf,
}

impl ConfigManager {
    /// 创建新的配置管理器
    ///
    /// 自动检测 XDG 配置目录（~/.config/ceair/），如果不存在则创建。
    pub fn new() -> ceair_core::Result<Self> {
        let config_dir = Self::resolve_config_dir()?;
        let config_path = config_dir.join("config.toml");

        debug!("配置目录: {:?}", config_dir);
        debug!("配置文件路径: {:?}", config_path);

        Ok(Self {
            config: Arc::new(RwLock::new(CeairConfig::default())),
            config_dir,
            config_path,
        })
    }

    /// 使用自定义目录创建配置管理器（主要用于测试）
    pub fn with_dir(dir: PathBuf) -> ceair_core::Result<Self> {
        let config_path = dir.join("config.toml");
        Ok(Self {
            config: Arc::new(RwLock::new(CeairConfig::default())),
            config_dir: dir,
            config_path,
        })
    }

    /// 解析 XDG 配置目录路径
    ///
    /// 优先使用 `dirs::config_dir()`（通常为 ~/.config），
    /// 然后拼接 "ceair" 子目录。
    fn resolve_config_dir() -> ceair_core::Result<PathBuf> {
        let base = dirs::config_dir().ok_or_else(|| {
            CeairError::config("无法确定系统配置目录")
        })?;
        Ok(base.join("ceair"))
    }

    /// 从磁盘加载配置文件
    ///
    /// 如果配置文件不存在，使用默认配置并创建文件。
    pub async fn load(&self) -> ceair_core::Result<CeairConfig> {
        if self.config_path.exists() {
            info!("从文件加载配置: {:?}", self.config_path);
            let content = tokio::fs::read_to_string(&self.config_path)
                .await
                .map_err(CeairError::from)?;
            let loaded = CeairConfig::from_toml(&content)?;

            // 更新内存中的配置
            let mut config = self.config.write().await;
            *config = loaded.clone();
            Ok(loaded)
        } else {
            info!("配置文件不存在，使用默认配置");
            let default_config = CeairConfig::default();

            // 尝试创建默认配置文件
            if let Err(e) = self.save_config(&default_config).await {
                warn!("无法保存默认配置文件: {e}");
            }

            let mut config = self.config.write().await;
            *config = default_config.clone();
            Ok(default_config)
        }
    }

    /// 将当前配置保存到磁盘
    pub async fn save(&self) -> ceair_core::Result<()> {
        let config = self.config.read().await;
        self.save_config(&config).await
    }

    /// 将指定配置写入磁盘
    async fn save_config(&self, config: &CeairConfig) -> ceair_core::Result<()> {
        // 确保配置目录存在
        if !self.config_dir.exists() {
            tokio::fs::create_dir_all(&self.config_dir)
                .await
                .map_err(|e| CeairError::io(format!("创建配置目录失败: {e}")))?;
        }

        let content = config.to_toml()?;
        tokio::fs::write(&self.config_path, content)
            .await
            .map_err(CeairError::from)?;

        info!("配置已保存到: {:?}", self.config_path);
        Ok(())
    }

    /// 获取当前配置的只读副本
    pub async fn get(&self) -> CeairConfig {
        self.config.read().await.clone()
    }

    /// 使用闭包更新配置
    ///
    /// 闭包接收可变引用，修改后自动保存到磁盘。
    pub async fn update<F>(&self, updater: F) -> ceair_core::Result<()>
    where
        F: FnOnce(&mut CeairConfig),
    {
        {
            let mut config = self.config.write().await;
            updater(&mut config);
        }
        // 保存更新后的配置
        self.save().await?;
        info!("配置已更新并保存");
        Ok(())
    }

    /// 重新从磁盘加载配置（热重载）
    pub async fn reload(&self) -> ceair_core::Result<CeairConfig> {
        info!("重新加载配置...");
        self.load().await
    }

    /// 获取配置目录路径
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// 从指定文件合并配置
    ///
    /// 读取额外的 TOML 配置文件，并将其中的值合并到当前配置中。
    /// 文件中存在的字段会覆盖当前值，不存在的字段保持不变。
    pub async fn merge_from_file(&self, path: &Path) -> ceair_core::Result<()> {
        info!("从文件合并配置: {:?}", path);

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(CeairError::from)?;

        // 解析为 TOML 值表以进行字段级合并
        let override_value: toml::Value = toml::from_str(&content)
            .map_err(|e| CeairError::config(format!("合并文件解析失败: {e}")))?;

        let mut config = self.config.write().await;

        // 将当前配置序列化为 TOML 值
        let current_toml = config.to_toml()?;
        let mut current_value: toml::Value = toml::from_str(&current_toml)
            .map_err(|e| CeairError::config(format!("当前配置序列化失败: {e}")))?;

        // 递归合并覆盖值
        merge_toml_values(&mut current_value, &override_value);

        // 将合并后的值反序列化回配置结构体
        let merged_toml = toml::to_string_pretty(&current_value)
            .map_err(|e| CeairError::serialization(format!("合并结果序列化失败: {e}")))?;
        *config = CeairConfig::from_toml(&merged_toml)?;

        info!("配置合并完成");
        Ok(())
    }
}

/// 递归合并两个 TOML 值
///
/// `base` 中的值会被 `overlay` 中对应的值覆盖。
/// 如果两个值都是表（Table），则递归合并。
fn merge_toml_values(base: &mut toml::Value, overlay: &toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base_table), toml::Value::Table(overlay_table)) => {
            // 两个都是表时，递归合并每个键
            for (key, overlay_val) in overlay_table {
                if let Some(base_val) = base_table.get_mut(key) {
                    merge_toml_values(base_val, overlay_val);
                } else {
                    // 基础表中不存在的键，直接插入
                    base_table.insert(key.clone(), overlay_val.clone());
                }
            }
        }
        (base, overlay) => {
            // 非表类型直接覆盖
            *base = overlay.clone();
        }
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试默认配置值是否合理
    #[test]
    fn test_default_config() {
        let config = CeairConfig::default();

        // 验证 AI 配置默认值
        assert_eq!(config.ai.provider, "openai");
        assert_eq!(config.ai.model, "gpt-4");
        assert!((config.ai.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(config.ai.max_tokens, 4096);
        assert!(config.ai.api_key.is_none());
        assert!(config.ai.base_url.is_none());

        // 验证智能体配置默认值
        assert_eq!(config.agent.max_iterations, 50);
        assert_eq!(config.agent.timeout_secs, 300);
        assert!(!config.agent.auto_approve_tools);

        // 验证工具配置默认值
        assert_eq!(config.tools.max_file_size, 10 * 1024 * 1024);
        assert!(!config.tools.allowed_paths.is_empty());
        assert!(config.tools.blocked_paths.is_empty());

        // 验证 TUI 配置默认值
        assert_eq!(config.tui.theme, "dark");
        assert!(config.tui.show_token_usage);
        assert!(config.tui.show_timestamps);

        // 验证日志配置默认值
        assert_eq!(config.logging.level, "info");
        assert!(config.logging.file.is_none());
        assert!(!config.logging.json_format);
    }

    /// 测试 TOML 序列化与反序列化的往返一致性
    #[test]
    fn test_toml_roundtrip() {
        let original = CeairConfig::default();
        let toml_str = original.to_toml().expect("序列化失败");
        let parsed = CeairConfig::from_toml(&toml_str).expect("反序列化失败");
        assert_eq!(original, parsed);
    }

    /// 测试从 TOML 字符串解析部分配置（缺失字段使用默认值）
    #[test]
    fn test_partial_toml_parse() {
        let partial_toml = r#"
[ai]
provider = "anthropic"
model = "claude-3"

[agent]
max_iterations = 100
"#;
        let config = CeairConfig::from_toml(partial_toml).expect("部分配置解析失败");

        // 明确指定的值
        assert_eq!(config.ai.provider, "anthropic");
        assert_eq!(config.ai.model, "claude-3");
        assert_eq!(config.agent.max_iterations, 100);

        // 未指定的值应使用默认值
        assert_eq!(config.tui.theme, "dark");
        assert_eq!(config.logging.level, "info");
    }

    /// 测试 JSON 序列化
    #[test]
    fn test_json_serialization() {
        let config = CeairConfig::default();
        let json = config.to_json().expect("JSON 序列化失败");

        // 验证 JSON 包含关键字段
        assert!(json.contains("\"provider\""));
        assert!(json.contains("\"openai\""));
        assert!(json.contains("\"max_iterations\""));
    }

    /// 测试 TOML 值递归合并
    #[test]
    fn test_merge_toml_values() {
        let base_str = r#"
[ai]
provider = "openai"
model = "gpt-4"
temperature = 0.7

[agent]
max_iterations = 50
"#;
        let overlay_str = r#"
[ai]
model = "gpt-4-turbo"
temperature = 0.5

[tui]
theme = "light"
"#;
        let mut base: toml::Value = toml::from_str(base_str).unwrap();
        let overlay: toml::Value = toml::from_str(overlay_str).unwrap();

        merge_toml_values(&mut base, &overlay);

        let table = base.as_table().unwrap();

        // 验证被覆盖的值
        let ai = table["ai"].as_table().unwrap();
        assert_eq!(ai["model"].as_str().unwrap(), "gpt-4-turbo");
        assert!((ai["temperature"].as_float().unwrap() - 0.5).abs() < f64::EPSILON);

        // 验证未被覆盖的值保持不变
        assert_eq!(ai["provider"].as_str().unwrap(), "openai");

        // 验证新增的值
        let tui = table["tui"].as_table().unwrap();
        assert_eq!(tui["theme"].as_str().unwrap(), "light");

        // 验证未被覆盖的段保持不变
        let agent = table["agent"].as_table().unwrap();
        assert_eq!(agent["max_iterations"].as_integer().unwrap(), 50);
    }

    /// 测试 ConfigManager 使用自定义目录创建
    #[tokio::test]
    async fn test_config_manager_with_dir() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let manager =
            ConfigManager::with_dir(tmp.path().to_path_buf()).expect("创建配置管理器失败");

        // 验证配置目录路径
        assert_eq!(manager.config_dir(), tmp.path());
    }

    /// 测试配置的加载、保存和重载流程
    #[tokio::test]
    async fn test_config_load_save_reload() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let manager =
            ConfigManager::with_dir(tmp.path().to_path_buf()).expect("创建配置管理器失败");

        // 首次加载（文件不存在，使用默认值）
        let config = manager.load().await.expect("加载配置失败");
        assert_eq!(config.ai.provider, "openai");

        // 更新配置
        manager
            .update(|c| {
                c.ai.provider = "anthropic".to_string();
                c.ai.model = "claude-3".to_string();
            })
            .await
            .expect("更新配置失败");

        // 验证内存中的配置已更新
        let updated = manager.get().await;
        assert_eq!(updated.ai.provider, "anthropic");

        // 重新加载，验证持久化成功
        let reloaded = manager.reload().await.expect("重载配置失败");
        assert_eq!(reloaded.ai.provider, "anthropic");
        assert_eq!(reloaded.ai.model, "claude-3");
    }

    /// 测试从额外文件合并配置
    #[tokio::test]
    async fn test_merge_from_file() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let manager =
            ConfigManager::with_dir(tmp.path().to_path_buf()).expect("创建配置管理器失败");

        // 先加载默认配置
        manager.load().await.expect("加载配置失败");

        // 创建覆盖配置文件
        let override_path = tmp.path().join("override.toml");
        let override_content = r#"
[ai]
provider = "azure"
model = "gpt-4-azure"

[logging]
level = "debug"
json_format = true
"#;
        tokio::fs::write(&override_path, override_content)
            .await
            .expect("写入覆盖文件失败");

        // 合并配置
        manager
            .merge_from_file(&override_path)
            .await
            .expect("合并配置失败");

        let merged = manager.get().await;

        // 验证被覆盖的值
        assert_eq!(merged.ai.provider, "azure");
        assert_eq!(merged.ai.model, "gpt-4-azure");
        assert_eq!(merged.logging.level, "debug");
        assert!(merged.logging.json_format);

        // 验证未被覆盖的值保持默认值
        assert_eq!(merged.agent.max_iterations, 50);
        assert_eq!(merged.tui.theme, "dark");
    }
}
