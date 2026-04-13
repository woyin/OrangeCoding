//! # 代理统一配置模块
//!
//! 定义代理的完整配置结构，支持从 TOML 文件加载、保存、合并与验证。

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// 代理完整配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentConfig {
    /// 模型配置
    pub model: ModelConfig,
    /// 工具配置
    pub tools: ToolsConfig,
    /// 压缩配置
    pub compaction: CompactionSettings,
    /// 记忆配置
    pub memory: MemorySettings,
    /// TTSR 配置
    pub ttsr: TtsrSettings,
    /// 系统提示词
    pub system_prompt: String,
    /// 最大连续工具调用次数
    pub max_tool_rounds: usize,
    /// 是否自动压缩
    pub auto_compact: bool,
    /// 沙箱模式
    pub sandbox: bool,
}

/// 模型配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelConfig {
    /// AI 提供商名称
    pub provider: String,
    /// 模型名称
    pub model: String,
    /// 采样温度
    pub temperature: Option<f32>,
    /// 最大 token 数
    pub max_tokens: Option<u64>,
    /// 思考级别（仅部分模型支持）
    pub thinking_level: Option<String>,
}

/// 工具配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// 启用的工具列表
    pub enabled_tools: Vec<String>,
    /// 禁用的工具列表
    pub disabled_tools: Vec<String>,
    /// Bash 命令超时（秒）
    pub bash_timeout: u64,
    /// 是否允许危险操作
    pub allow_dangerous: bool,
}

/// 压缩设置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactionSettings {
    /// 是否启用压缩
    pub enabled: bool,
    /// 触发压缩的最大 token 数
    pub max_tokens: usize,
    /// 保留最近消息数
    pub keep_recent: usize,
}

/// 记忆设置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemorySettings {
    /// 是否启用记忆
    pub enabled: bool,
    /// 最大记忆条目数
    pub max_entries: usize,
}

/// TTSR 设置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtsrSettings {
    /// 是否启用 TTSR
    pub enabled: bool,
    /// 规则列表
    pub rules: Vec<TtsrRuleConfig>,
}

/// TTSR 规则配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtsrRuleConfig {
    /// 规则名称
    pub name: String,
    /// 触发正则
    pub trigger: String,
    /// 注入内容
    pub injection: String,
    /// 是否只触发一次
    pub once: bool,
}

// ---------------------------------------------------------------------------
// 默认实现
// ---------------------------------------------------------------------------

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: ModelConfig {
                provider: "openai".to_string(),
                model: "gpt-4o".to_string(),
                temperature: Some(0.7),
                max_tokens: Some(4096),
                thinking_level: None,
            },
            tools: ToolsConfig {
                enabled_tools: vec![],
                disabled_tools: vec![],
                bash_timeout: 30,
                allow_dangerous: false,
            },
            compaction: CompactionSettings {
                enabled: true,
                max_tokens: 100_000,
                keep_recent: 10,
            },
            memory: MemorySettings {
                enabled: false,
                max_entries: 1000,
            },
            ttsr: TtsrSettings {
                enabled: false,
                rules: vec![],
            },
            system_prompt: "你是一个有帮助的AI编程助手。".to_string(),
            max_tool_rounds: 50,
            auto_compact: true,
            sandbox: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 核心方法
// ---------------------------------------------------------------------------

impl AgentConfig {
    /// 从 TOML 文件加载配置
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("读取配置文件失败: {}", e))?;
        toml::from_str(&content).map_err(|e| format!("解析 TOML 配置失败: {}", e))
    }

    /// 保存配置到 TOML 文件
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        let content = toml::to_string_pretty(self).map_err(|e| format!("序列化配置失败: {}", e))?;
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
        std::fs::write(path, content).map_err(|e| format!("写入配置文件失败: {}", e))
    }

    /// 合并两个配置（other 的非默认值覆盖 self）
    pub fn merge(&mut self, other: &AgentConfig) {
        // 模型配置始终覆盖
        self.model = other.model.clone();
        self.tools = other.tools.clone();
        self.compaction = other.compaction.clone();
        self.memory = other.memory.clone();
        self.ttsr = other.ttsr.clone();
        self.system_prompt = other.system_prompt.clone();
        self.max_tool_rounds = other.max_tool_rounds;
        self.auto_compact = other.auto_compact;
        self.sandbox = other.sandbox;
    }

    /// 验证配置合法性，返回错误列表
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // 验证模型配置
        if self.model.provider.is_empty() {
            errors.push("模型提供商不能为空".to_string());
        }
        if self.model.model.is_empty() {
            errors.push("模型名称不能为空".to_string());
        }
        if let Some(temp) = self.model.temperature {
            if !(0.0..=2.0).contains(&temp) {
                errors.push(format!("温度值必须在 0.0 到 2.0 之间，当前值: {}", temp));
            }
        }

        // 验证工具配置
        if self.tools.bash_timeout == 0 {
            errors.push("Bash 超时时间不能为 0".to_string());
        }

        // 验证压缩配置
        if self.compaction.enabled && self.compaction.max_tokens == 0 {
            errors.push("压缩启用时 max_tokens 不能为 0".to_string());
        }

        // 验证最大工具轮次
        if self.max_tool_rounds == 0 {
            errors.push("最大工具调用次数不能为 0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试默认配置的各字段值
    #[test]
    fn test_default_config() {
        let cfg = AgentConfig::default();
        assert_eq!(cfg.model.provider, "openai");
        assert_eq!(cfg.model.model, "gpt-4o");
        assert_eq!(cfg.model.temperature, Some(0.7));
        assert_eq!(cfg.model.max_tokens, Some(4096));
        assert!(cfg.model.thinking_level.is_none());
        assert!(cfg.tools.enabled_tools.is_empty());
        assert!(!cfg.tools.allow_dangerous);
        assert_eq!(cfg.tools.bash_timeout, 30);
        assert!(cfg.compaction.enabled);
        assert_eq!(cfg.compaction.max_tokens, 100_000);
        assert_eq!(cfg.compaction.keep_recent, 10);
        assert!(!cfg.memory.enabled);
        assert_eq!(cfg.memory.max_entries, 1000);
        assert!(!cfg.ttsr.enabled);
        assert!(cfg.ttsr.rules.is_empty());
        assert_eq!(cfg.system_prompt, "你是一个有帮助的AI编程助手。");
        assert_eq!(cfg.max_tool_rounds, 50);
        assert!(cfg.auto_compact);
        assert!(!cfg.sandbox);
    }

    /// 测试从 TOML 字符串加载配置
    #[test]
    fn test_load_from_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let toml_content = r#"
system_prompt = "测试提示词"
max_tool_rounds = 20
auto_compact = false
sandbox = true

[model]
provider = "deepseek"
model = "deepseek-chat"
temperature = 0.5
max_tokens = 8192

[tools]
enabled_tools = ["bash", "read"]
disabled_tools = []
bash_timeout = 60
allow_dangerous = true

[compaction]
enabled = false
max_tokens = 50000
keep_recent = 5

[memory]
enabled = true
max_entries = 500

[ttsr]
enabled = true

[[ttsr.rules]]
name = "test_rule"
trigger = "TODO"
injection = "请注意待办事项"
once = true
"#;
        std::fs::write(&path, toml_content).unwrap();

        let cfg = AgentConfig::load_from_file(&path).unwrap();
        assert_eq!(cfg.model.provider, "deepseek");
        assert_eq!(cfg.model.model, "deepseek-chat");
        assert_eq!(cfg.model.temperature, Some(0.5));
        assert_eq!(cfg.model.max_tokens, Some(8192));
        assert_eq!(cfg.system_prompt, "测试提示词");
        assert_eq!(cfg.max_tool_rounds, 20);
        assert!(!cfg.auto_compact);
        assert!(cfg.sandbox);
        assert!(cfg.tools.allow_dangerous);
        assert_eq!(cfg.tools.bash_timeout, 60);
        assert_eq!(cfg.tools.enabled_tools, vec!["bash", "read"]);
        assert!(!cfg.compaction.enabled);
        assert!(cfg.memory.enabled);
        assert_eq!(cfg.memory.max_entries, 500);
        assert!(cfg.ttsr.enabled);
        assert_eq!(cfg.ttsr.rules.len(), 1);
        assert_eq!(cfg.ttsr.rules[0].name, "test_rule");
        assert!(cfg.ttsr.rules[0].once);
    }

    /// 测试保存后重新加载的一致性
    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roundtrip.toml");

        let mut cfg = AgentConfig::default();
        cfg.model.provider = "anthropic".to_string();
        cfg.model.model = "claude-3".to_string();
        cfg.max_tool_rounds = 30;
        cfg.ttsr.rules.push(TtsrRuleConfig {
            name: "r1".to_string(),
            trigger: "pattern".to_string(),
            injection: "注入内容".to_string(),
            once: false,
        });

        cfg.save_to_file(&path).unwrap();
        let loaded = AgentConfig::load_from_file(&path).unwrap();

        assert_eq!(loaded.model.provider, "anthropic");
        assert_eq!(loaded.model.model, "claude-3");
        assert_eq!(loaded.max_tool_rounds, 30);
        assert_eq!(loaded.ttsr.rules.len(), 1);
        assert_eq!(loaded.ttsr.rules[0].name, "r1");
    }

    /// 测试合法配置通过验证
    #[test]
    fn test_validate_valid() {
        let cfg = AgentConfig::default();
        assert!(cfg.validate().is_ok());
    }

    /// 测试非法模型配置被拒绝
    #[test]
    fn test_validate_invalid_model() {
        let mut cfg = AgentConfig::default();
        cfg.model.provider = String::new();
        cfg.model.model = String::new();
        cfg.model.temperature = Some(3.0);

        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("提供商")));
        assert!(errors.iter().any(|e| e.contains("模型名称")));
        assert!(errors.iter().any(|e| e.contains("温度值")));
    }

    /// 测试合并配置（other 覆盖 self）
    #[test]
    fn test_merge_configs() {
        let mut base = AgentConfig::default();
        let mut overlay = AgentConfig::default();
        overlay.model.provider = "anthropic".to_string();
        overlay.max_tool_rounds = 100;
        overlay.sandbox = true;

        base.merge(&overlay);

        assert_eq!(base.model.provider, "anthropic");
        assert_eq!(base.max_tool_rounds, 100);
        assert!(base.sandbox);
    }

    /// 测试模型配置默认值
    #[test]
    fn test_model_config_defaults() {
        let cfg = AgentConfig::default();
        assert_eq!(cfg.model.provider, "openai");
        assert_eq!(cfg.model.model, "gpt-4o");
        assert_eq!(cfg.model.temperature, Some(0.7));
        assert_eq!(cfg.model.max_tokens, Some(4096));
        assert!(cfg.model.thinking_level.is_none());
    }

    /// 测试工具配置
    #[test]
    fn test_tools_config() {
        let mut cfg = AgentConfig::default();
        cfg.tools.enabled_tools = vec!["bash".to_string(), "read".to_string()];
        cfg.tools.disabled_tools = vec!["write".to_string()];
        cfg.tools.bash_timeout = 120;
        cfg.tools.allow_dangerous = true;

        assert_eq!(cfg.tools.enabled_tools.len(), 2);
        assert_eq!(cfg.tools.disabled_tools.len(), 1);
        assert_eq!(cfg.tools.bash_timeout, 120);
        assert!(cfg.tools.allow_dangerous);
    }

    /// 测试压缩设置验证
    #[test]
    fn test_compaction_settings() {
        let mut cfg = AgentConfig::default();
        // 默认压缩设置应通过验证
        assert!(cfg.validate().is_ok());

        // 启用压缩但 max_tokens 为 0 应报错
        cfg.compaction.enabled = true;
        cfg.compaction.max_tokens = 0;
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("max_tokens")));
    }
}
