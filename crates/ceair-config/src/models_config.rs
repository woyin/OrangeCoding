//! # 模型配置模块
//!
//! 定义 AI 模型提供商的配置结构，支持从 YAML 文件加载。
//! 包括提供商配置、模型定义、成本信息等。

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use ceair_core::CeairError;

// ---------------------------------------------------------------------------
// API 类型
// ---------------------------------------------------------------------------

/// API 类型 — 定义与模型交互的协议
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiType {
    /// OpenAI 补全接口
    #[serde(rename = "openai-completions")]
    OpenAiCompletions,
    /// OpenAI 响应接口
    #[serde(rename = "openai-responses")]
    OpenAiResponses,
    /// Anthropic 消息接口
    #[serde(rename = "anthropic-messages")]
    AnthropicMessages,
    /// Google 生成式 AI 接口
    #[serde(rename = "google-generative-ai")]
    GoogleGenerativeAi,
}

// ---------------------------------------------------------------------------
// 认证类型
// ---------------------------------------------------------------------------

/// 认证类型 — 定义 API 认证方式
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthType {
    /// Bearer Token 认证
    #[serde(rename = "bearer")]
    Bearer,
    /// API Key 认证
    #[serde(rename = "api-key")]
    ApiKey,
    /// 无认证
    #[serde(rename = "none")]
    None,
}

// ---------------------------------------------------------------------------
// 模型成本
// ---------------------------------------------------------------------------

/// 模型成本 — 每百万 token 的价格（美元）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelCost {
    /// 输入 token 价格
    pub input: f64,
    /// 输出 token 价格
    pub output: f64,
    /// 缓存读取价格
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    /// 缓存写入价格
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
}

// ---------------------------------------------------------------------------
// 模型定义
// ---------------------------------------------------------------------------

/// 模型定义 — 单个模型的完整配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelDefinition {
    /// 模型唯一标识符
    pub id: String,
    /// 模型显示名称
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 是否支持推理
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    /// 支持的输入类型（如 "text", "image"）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<String>>,
    /// 模型成本
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<ModelCost>,
    /// 上下文窗口大小（token 数）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// 最大输出 token 数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
}

// ---------------------------------------------------------------------------
// 模型发现配置
// ---------------------------------------------------------------------------

/// 模型发现配置 — 用于自动发现本地模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// 发现类型（"ollama", "lm-studio", "llama.cpp"）
    #[serde(rename = "type")]
    pub discovery_type: String,
}

// ---------------------------------------------------------------------------
// 提供商配置
// ---------------------------------------------------------------------------

/// 提供商配置 — 单个 AI 服务提供商的完整配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API 基础 URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// API 密钥（环境变量名或直接值）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// API 类型
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<ApiType>,
    /// 自定义请求头
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    /// 认证方式
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthType>,
    /// 模型列表
    #[serde(default)]
    pub models: Vec<ModelDefinition>,
    /// 模型发现配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<DiscoveryConfig>,
}

// ---------------------------------------------------------------------------
// 模型配置文件
// ---------------------------------------------------------------------------

/// 模型配置文件（models.yml） — 所有提供商及其模型的汇总配置
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModelsConfig {
    /// 提供商映射（名称 -> 配置）
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

impl ModelsConfig {
    /// 从 YAML 文件加载配置
    pub fn load_from_file(path: &Path) -> ceair_core::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            CeairError::config(format!("读取模型配置文件失败: {}: {}", path.display(), e))
        })?;

        serde_yaml::from_str(&content).map_err(|e| {
            CeairError::config(format!("解析模型配置 YAML 失败: {}: {}", path.display(), e))
        })
    }

    /// 合并另一个配置（另一个的值覆盖当前值）
    pub fn merge(&mut self, other: ModelsConfig) {
        for (name, provider) in other.providers {
            self.providers.insert(name, provider);
        }
    }

    /// 获取提供商配置
    pub fn get_provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// 列出所有模型，返回 `(提供商名称, 模型定义)` 列表
    pub fn list_models(&self) -> Vec<(String, &ModelDefinition)> {
        let mut models = Vec::new();
        for (provider_name, provider) in &self.providers {
            for model in &provider.models {
                models.push((provider_name.clone(), model));
            }
        }
        models
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // -----------------------------------------------------------------------
    // 1. test_load_yaml_basic
    // -----------------------------------------------------------------------
    #[test]
    fn test_load_yaml_basic() {
        let tmp = TempDir::new().unwrap();
        let yaml_path = tmp.path().join("models.yml");
        fs::write(
            &yaml_path,
            r#"
providers:
  openai:
    base_url: "https://api.openai.com/v1"
    api_key: "OPENAI_API_KEY"
    api: "openai-completions"
    auth: "bearer"
    models: []
"#,
        )
        .unwrap();

        let config = ModelsConfig::load_from_file(&yaml_path).unwrap();
        let openai = config.get_provider("openai").unwrap();

        assert_eq!(openai.base_url.as_deref(), Some("https://api.openai.com/v1"));
        assert_eq!(openai.api_key.as_deref(), Some("OPENAI_API_KEY"));
        assert_eq!(openai.api, Some(ApiType::OpenAiCompletions));
        assert_eq!(openai.auth, Some(AuthType::Bearer));
        assert!(openai.models.is_empty());
    }

    // -----------------------------------------------------------------------
    // 2. test_load_yaml_with_models
    // -----------------------------------------------------------------------
    #[test]
    fn test_load_yaml_with_models() {
        let tmp = TempDir::new().unwrap();
        let yaml_path = tmp.path().join("models.yml");
        fs::write(
            &yaml_path,
            r#"
providers:
  anthropic:
    base_url: "https://api.anthropic.com"
    api: "anthropic-messages"
    models:
      - id: "claude-sonnet-4-20250514"
        name: "Claude Sonnet 4"
        reasoning: true
        input: ["text", "image"]
        context_window: 200000
        max_tokens: 8192
        cost:
          input: 3.0
          output: 15.0
          cache_read: 0.3
          cache_write: 3.75
      - id: "claude-haiku-3.5"
        name: "Claude Haiku 3.5"
        context_window: 200000
        max_tokens: 4096
"#,
        )
        .unwrap();

        let config = ModelsConfig::load_from_file(&yaml_path).unwrap();
        let anthropic = config.get_provider("anthropic").unwrap();

        assert_eq!(anthropic.models.len(), 2);

        let sonnet = &anthropic.models[0];
        assert_eq!(sonnet.id, "claude-sonnet-4-20250514");
        assert_eq!(sonnet.name.as_deref(), Some("Claude Sonnet 4"));
        assert_eq!(sonnet.reasoning, Some(true));
        assert_eq!(
            sonnet.input.as_ref().unwrap(),
            &vec!["text".to_string(), "image".to_string()]
        );
        assert_eq!(sonnet.context_window, Some(200_000));

        let cost = sonnet.cost.as_ref().unwrap();
        assert!((cost.input - 3.0).abs() < f64::EPSILON);
        assert!((cost.output - 15.0).abs() < f64::EPSILON);
        assert!((cost.cache_read.unwrap() - 0.3).abs() < f64::EPSILON);
        assert!((cost.cache_write.unwrap() - 3.75).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 3. test_merge_configs
    // -----------------------------------------------------------------------
    #[test]
    fn test_merge_configs() {
        let mut base = ModelsConfig::default();
        base.providers.insert(
            "openai".to_string(),
            ProviderConfig {
                base_url: Some("https://api.openai.com/v1".to_string()),
                api_key: Some("old-key".to_string()),
                api: Some(ApiType::OpenAiCompletions),
                headers: None,
                auth: Some(AuthType::Bearer),
                models: vec![],
                discovery: None,
            },
        );

        let mut overlay = ModelsConfig::default();
        overlay.providers.insert(
            "openai".to_string(),
            ProviderConfig {
                base_url: Some("https://custom-proxy.com/v1".to_string()),
                api_key: Some("new-key".to_string()),
                api: Some(ApiType::OpenAiCompletions),
                headers: None,
                auth: Some(AuthType::Bearer),
                models: vec![ModelDefinition {
                    id: "gpt-4o".to_string(),
                    name: Some("GPT-4o".to_string()),
                    reasoning: None,
                    input: None,
                    cost: None,
                    context_window: Some(128_000),
                    max_tokens: Some(4096),
                }],
                discovery: None,
            },
        );
        overlay.providers.insert(
            "anthropic".to_string(),
            ProviderConfig {
                base_url: Some("https://api.anthropic.com".to_string()),
                api_key: None,
                api: Some(ApiType::AnthropicMessages),
                headers: None,
                auth: None,
                models: vec![],
                discovery: None,
            },
        );

        // 合并：overlay 覆盖 base
        base.merge(overlay);

        // openai 被覆盖
        let openai = base.get_provider("openai").unwrap();
        assert_eq!(openai.api_key.as_deref(), Some("new-key"));
        assert_eq!(openai.base_url.as_deref(), Some("https://custom-proxy.com/v1"));
        assert_eq!(openai.models.len(), 1);

        // anthropic 新增
        assert!(base.get_provider("anthropic").is_some());
    }

    // -----------------------------------------------------------------------
    // 4. test_api_type_serde
    // -----------------------------------------------------------------------
    #[test]
    fn test_api_type_serde() {
        // 序列化
        let json = serde_json::to_string(&ApiType::OpenAiCompletions).unwrap();
        assert_eq!(json, r#""openai-completions""#);

        let json = serde_json::to_string(&ApiType::AnthropicMessages).unwrap();
        assert_eq!(json, r#""anthropic-messages""#);

        let json = serde_json::to_string(&ApiType::GoogleGenerativeAi).unwrap();
        assert_eq!(json, r#""google-generative-ai""#);

        let json = serde_json::to_string(&ApiType::OpenAiResponses).unwrap();
        assert_eq!(json, r#""openai-responses""#);

        // 反序列化
        let api: ApiType = serde_json::from_str(r#""openai-completions""#).unwrap();
        assert_eq!(api, ApiType::OpenAiCompletions);

        let api: ApiType = serde_json::from_str(r#""anthropic-messages""#).unwrap();
        assert_eq!(api, ApiType::AnthropicMessages);
    }

    // -----------------------------------------------------------------------
    // 5. test_auth_type_serde
    // -----------------------------------------------------------------------
    #[test]
    fn test_auth_type_serde() {
        let json = serde_json::to_string(&AuthType::Bearer).unwrap();
        assert_eq!(json, r#""bearer""#);

        let json = serde_json::to_string(&AuthType::ApiKey).unwrap();
        assert_eq!(json, r#""api-key""#);

        let json = serde_json::to_string(&AuthType::None).unwrap();
        assert_eq!(json, r#""none""#);

        // 反序列化
        let auth: AuthType = serde_json::from_str(r#""api-key""#).unwrap();
        assert_eq!(auth, AuthType::ApiKey);
    }

    // -----------------------------------------------------------------------
    // 6. test_model_cost_defaults
    // -----------------------------------------------------------------------
    #[test]
    fn test_model_cost_defaults() {
        let yaml = r#"
input: 5.0
output: 15.0
"#;
        let cost: ModelCost = serde_yaml::from_str(yaml).unwrap();

        assert!((cost.input - 5.0).abs() < f64::EPSILON);
        assert!((cost.output - 15.0).abs() < f64::EPSILON);
        assert!(cost.cache_read.is_none());
        assert!(cost.cache_write.is_none());
    }

    // -----------------------------------------------------------------------
    // 7. test_list_models
    // -----------------------------------------------------------------------
    #[test]
    fn test_list_models() {
        let mut config = ModelsConfig::default();
        config.providers.insert(
            "openai".to_string(),
            ProviderConfig {
                base_url: None,
                api_key: None,
                api: None,
                headers: None,
                auth: None,
                models: vec![
                    ModelDefinition {
                        id: "gpt-4o".to_string(),
                        name: Some("GPT-4o".to_string()),
                        reasoning: None,
                        input: None,
                        cost: None,
                        context_window: None,
                        max_tokens: None,
                    },
                    ModelDefinition {
                        id: "gpt-4o-mini".to_string(),
                        name: Some("GPT-4o Mini".to_string()),
                        reasoning: None,
                        input: None,
                        cost: None,
                        context_window: None,
                        max_tokens: None,
                    },
                ],
                discovery: None,
            },
        );
        config.providers.insert(
            "anthropic".to_string(),
            ProviderConfig {
                base_url: None,
                api_key: None,
                api: None,
                headers: None,
                auth: None,
                models: vec![ModelDefinition {
                    id: "claude-sonnet-4-20250514".to_string(),
                    name: None,
                    reasoning: None,
                    input: None,
                    cost: None,
                    context_window: None,
                    max_tokens: None,
                }],
                discovery: None,
            },
        );

        let models = config.list_models();
        assert_eq!(models.len(), 3);

        let ids: Vec<&str> = models.iter().map(|(_, m)| m.id.as_str()).collect();
        assert!(ids.contains(&"gpt-4o"));
        assert!(ids.contains(&"gpt-4o-mini"));
        assert!(ids.contains(&"claude-sonnet-4-20250514"));
    }

    // -----------------------------------------------------------------------
    // 8. test_provider_with_discovery
    // -----------------------------------------------------------------------
    #[test]
    fn test_provider_with_discovery() {
        let tmp = TempDir::new().unwrap();
        let yaml_path = tmp.path().join("models.yml");
        fs::write(
            &yaml_path,
            r#"
providers:
  local:
    base_url: "http://localhost:11434"
    auth: "none"
    models: []
    discovery:
      type: "ollama"
"#,
        )
        .unwrap();

        let config = ModelsConfig::load_from_file(&yaml_path).unwrap();
        let local = config.get_provider("local").unwrap();

        assert_eq!(local.auth, Some(AuthType::None));
        let disc = local.discovery.as_ref().unwrap();
        assert_eq!(disc.discovery_type, "ollama");
    }

    // -----------------------------------------------------------------------
    // 9. test_empty_config
    // -----------------------------------------------------------------------
    #[test]
    fn test_empty_config() {
        let config = ModelsConfig::default();
        assert!(config.providers.is_empty());
        assert!(config.list_models().is_empty());
        assert!(config.get_provider("nonexistent").is_none());
    }
}
