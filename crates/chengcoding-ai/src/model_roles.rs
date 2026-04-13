//! 模型角色路由模块
//!
//! 提供按任务类型选择不同模型的能力，支持回退链和环境变量配置。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 模型角色 — 不同任务使用不同模型
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelRole {
    /// 默认模型 — 正常编码工作
    Default,
    /// 小模型 — 快速/便宜的探索任务
    Smol,
    /// 慢模型 — 深度推理
    Slow,
    /// 计划模型 — 架构规划
    Plan,
    /// 提交模型 — Git 提交消息生成
    Commit,
}

/// 思考级别
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingLevel {
    /// 关闭思考
    Off,
    /// 最低程度思考
    Minimal,
    /// 低程度思考
    Low,
    /// 中等程度思考
    Medium,
    /// 高程度思考
    High,
    /// 极高程度思考
    XHigh,
}

impl ThinkingLevel {
    /// 获取思考级别的数值排序（越高越深度思考）
    pub fn ordinal(&self) -> u8 {
        match self {
            ThinkingLevel::Off => 0,
            ThinkingLevel::Minimal => 1,
            ThinkingLevel::Low => 2,
            ThinkingLevel::Medium => 3,
            ThinkingLevel::High => 4,
            ThinkingLevel::XHigh => 5,
        }
    }

    /// 从字符串解析思考级别
    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" => Some(ThinkingLevel::Off),
            "minimal" => Some(ThinkingLevel::Minimal),
            "low" => Some(ThinkingLevel::Low),
            "medium" | "med" => Some(ThinkingLevel::Medium),
            "high" => Some(ThinkingLevel::High),
            "xhigh" | "x-high" | "extra-high" => Some(ThinkingLevel::XHigh),
            _ => None,
        }
    }
}

impl PartialOrd for ThinkingLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ThinkingLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ordinal().cmp(&other.ordinal())
    }
}

/// 模型配置
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 提供者名称（如 "openai"、"anthropic"）
    pub provider: String,
    /// 模型标识（如 "gpt-4o"、"claude-sonnet-4-20250514"）
    pub model_id: String,
    /// 可选的思考级别
    pub thinking_level: Option<ThinkingLevel>,
}

/// 模型角色路由器
///
/// 管理不同任务角色到具体模型配置的映射，支持回退链。
pub struct ModelRoleRouter {
    /// 角色到模型配置的映射
    roles: HashMap<ModelRole, ModelConfig>,
    /// 角色的回退链（按优先级排列的模型 ID 列表）
    fallback_chains: HashMap<ModelRole, Vec<String>>,
}

impl ModelRoleRouter {
    /// 创建空的路由器
    pub fn new() -> Self {
        Self {
            roles: HashMap::new(),
            fallback_chains: HashMap::new(),
        }
    }

    /// 设置角色的模型配置
    pub fn set_role(&mut self, role: ModelRole, config: ModelConfig) {
        self.roles.insert(role, config);
    }

    /// 获取角色的模型配置
    pub fn get_role(&self, role: &ModelRole) -> Option<&ModelConfig> {
        self.roles.get(role)
    }

    /// 解析模型 ID（支持 "provider/model:thinking" 格式）
    ///
    /// 支持的格式：
    /// - `"gpt-4o"` → (None, "gpt-4o", None)
    /// - `"anthropic/claude-sonnet-4"` → (Some("anthropic"), "claude-sonnet-4", None)
    /// - `"claude-sonnet-4:high"` → (None, "claude-sonnet-4", Some(High))
    /// - `"anthropic/claude-sonnet-4:high"` → (Some("anthropic"), "claude-sonnet-4", Some(High))
    pub fn parse_model_id(id: &str) -> (Option<String>, String, Option<ThinkingLevel>) {
        let (rest, thinking) = if let Some(colon_pos) = id.rfind(':') {
            let thinking_str = &id[colon_pos + 1..];
            match ThinkingLevel::from_str_name(thinking_str) {
                Some(level) => (&id[..colon_pos], Some(level)),
                None => (id, None),
            }
        } else {
            (id, None)
        };

        let (provider, model) = if let Some(slash_pos) = rest.find('/') {
            (
                Some(rest[..slash_pos].to_string()),
                rest[slash_pos + 1..].to_string(),
            )
        } else {
            (None, rest.to_string())
        };

        (provider, model, thinking)
    }

    /// 设置回退链
    pub fn set_fallback_chain(&mut self, role: ModelRole, chain: Vec<String>) {
        self.fallback_chains.insert(role, chain);
    }

    /// 获取下一个回退模型
    pub fn next_fallback(&self, role: &ModelRole, current_index: usize) -> Option<&str> {
        self.fallback_chains
            .get(role)
            .and_then(|chain| chain.get(current_index))
            .map(|s| s.as_str())
    }

    /// 从环境变量加载角色配置
    ///
    /// 支持的环境变量：
    /// - `ChengCoding_MODEL_DEFAULT` — 默认模型
    /// - `ChengCoding_MODEL_SMOL` — 小模型
    /// - `ChengCoding_MODEL_SLOW` — 慢模型
    /// - `ChengCoding_MODEL_PLAN` — 计划模型
    /// - `ChengCoding_MODEL_COMMIT` — 提交模型
    ///
    /// 值格式: `provider/model:thinking` (provider 和 thinking 可选)
    pub fn from_env() -> Self {
        let mut router = Self::new();

        let env_mappings = [
            ("ChengCoding_MODEL_DEFAULT", ModelRole::Default),
            ("ChengCoding_MODEL_SMOL", ModelRole::Smol),
            ("ChengCoding_MODEL_SLOW", ModelRole::Slow),
            ("ChengCoding_MODEL_PLAN", ModelRole::Plan),
            ("ChengCoding_MODEL_COMMIT", ModelRole::Commit),
        ];

        for (env_var, role) in &env_mappings {
            if let Ok(value) = std::env::var(env_var) {
                let (provider, model_id, thinking_level) = Self::parse_model_id(&value);
                router.set_role(
                    role.clone(),
                    ModelConfig {
                        provider: provider.unwrap_or_default(),
                        model_id,
                        thinking_level,
                    },
                );
            }
        }

        router
    }
}

impl Default for ModelRoleRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_get_role() {
        // 验证角色配置的设置和获取
        let mut router = ModelRoleRouter::new();
        let config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            thinking_level: None,
        };

        router.set_role(ModelRole::Default, config.clone());

        let result = router.get_role(&ModelRole::Default);
        assert!(result.is_some());
        assert_eq!(result.unwrap().model_id, "gpt-4o");
        assert_eq!(result.unwrap().provider, "openai");

        // 未设置的角色应返回 None
        assert!(router.get_role(&ModelRole::Smol).is_none());
    }

    #[test]
    fn test_parse_model_id_simple() {
        // 验证简单模型 ID 解析
        let (provider, model, thinking) = ModelRoleRouter::parse_model_id("gpt-4o");
        assert!(provider.is_none());
        assert_eq!(model, "gpt-4o");
        assert!(thinking.is_none());
    }

    #[test]
    fn test_parse_model_id_with_provider() {
        // 验证带提供者前缀的模型 ID 解析
        let (provider, model, thinking) =
            ModelRoleRouter::parse_model_id("anthropic/claude-sonnet-4");
        assert_eq!(provider.unwrap(), "anthropic");
        assert_eq!(model, "claude-sonnet-4");
        assert!(thinking.is_none());
    }

    #[test]
    fn test_parse_model_id_with_thinking() {
        // 验证带思考级别的模型 ID 解析
        let (provider, model, thinking) = ModelRoleRouter::parse_model_id("claude-sonnet-4:high");
        assert!(provider.is_none());
        assert_eq!(model, "claude-sonnet-4");
        assert_eq!(thinking.unwrap(), ThinkingLevel::High);
    }

    #[test]
    fn test_parse_model_id_full() {
        // 验证完整格式的模型 ID 解析
        let (provider, model, thinking) =
            ModelRoleRouter::parse_model_id("anthropic/claude-sonnet-4:high");
        assert_eq!(provider.unwrap(), "anthropic");
        assert_eq!(model, "claude-sonnet-4");
        assert_eq!(thinking.unwrap(), ThinkingLevel::High);
    }

    #[test]
    fn test_fallback_chain() {
        // 验证回退链的设置和查询
        let mut router = ModelRoleRouter::new();
        router.set_fallback_chain(
            ModelRole::Default,
            vec![
                "gpt-4o".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
        );

        assert_eq!(router.next_fallback(&ModelRole::Default, 0), Some("gpt-4o"));
        assert_eq!(
            router.next_fallback(&ModelRole::Default, 1),
            Some("gpt-4-turbo")
        );
        assert_eq!(
            router.next_fallback(&ModelRole::Default, 2),
            Some("gpt-3.5-turbo")
        );
        // 超出范围应返回 None
        assert!(router.next_fallback(&ModelRole::Default, 3).is_none());
        // 未设置回退链的角色应返回 None
        assert!(router.next_fallback(&ModelRole::Smol, 0).is_none());
    }

    #[test]
    fn test_default_roles() {
        // 验证新建路由器默认无角色配置
        let router = ModelRoleRouter::new();
        assert!(router.get_role(&ModelRole::Default).is_none());
        assert!(router.get_role(&ModelRole::Smol).is_none());
        assert!(router.get_role(&ModelRole::Slow).is_none());
        assert!(router.get_role(&ModelRole::Plan).is_none());
        assert!(router.get_role(&ModelRole::Commit).is_none());
    }

    #[test]
    fn test_thinking_level_ordering() {
        // 验证思考级别的排序
        assert!(ThinkingLevel::Off < ThinkingLevel::Minimal);
        assert!(ThinkingLevel::Minimal < ThinkingLevel::Low);
        assert!(ThinkingLevel::Low < ThinkingLevel::Medium);
        assert!(ThinkingLevel::Medium < ThinkingLevel::High);
        assert!(ThinkingLevel::High < ThinkingLevel::XHigh);

        // 验证相等
        assert_eq!(ThinkingLevel::High, ThinkingLevel::High);
    }

    #[test]
    fn test_from_env() {
        // 设置环境变量，验证从环境变量加载
        std::env::set_var("ChengCoding_MODEL_DEFAULT", "openai/gpt-4o:medium");
        std::env::set_var("ChengCoding_MODEL_SMOL", "gpt-4o-mini");

        let router = ModelRoleRouter::from_env();

        let default_config = router.get_role(&ModelRole::Default).unwrap();
        assert_eq!(default_config.provider, "openai");
        assert_eq!(default_config.model_id, "gpt-4o");
        assert_eq!(default_config.thinking_level, Some(ThinkingLevel::Medium));

        let smol_config = router.get_role(&ModelRole::Smol).unwrap();
        assert_eq!(smol_config.provider, "");
        assert_eq!(smol_config.model_id, "gpt-4o-mini");
        assert!(smol_config.thinking_level.is_none());

        // 清理环境变量
        std::env::remove_var("ChengCoding_MODEL_DEFAULT");
        std::env::remove_var("ChengCoding_MODEL_SMOL");
    }
}
