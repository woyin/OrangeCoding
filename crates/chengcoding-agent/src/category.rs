//! # 类别路由模块
//!
//! 基于意图的模型路由系统。通过语义化的类别名称（而非具体模型名称）来委派任务，
//! 系统自动将类别映射到最佳模型和配置。
//!
//! ## 内置类别
//!
//! | 类别 | 默认模型 | 用途 |
//! |------|---------|------|
//! | visual-engineering | gemini-3.1-pro | 前端、UI/UX |
//! | ultrabrain | gpt-5.4 (xhigh) | 深度逻辑推理 |
//! | deep | gpt-5.4 (medium) | 自主问题解决 |
//! | artistry | gemini-3.1-pro (high) | 创意任务 |
//! | quick | gpt-5.4-mini | 简单快速任务 |
//! | unspecified-low | claude-sonnet-4-6 | 低难度通用 |
//! | unspecified-high | claude-opus-4-6 (max) | 高难度通用 |
//! | writing | gemini-3-flash | 文档编写 |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================
// CategoryConfig — 类别配置
// ============================================================

/// 单个类别的配置定义。
///
/// 包含模型选择、采样参数、思考链配置等所有影响 Agent 行为的参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryConfig {
    /// 类别的人类可读描述
    #[serde(default)]
    pub description: String,

    /// AI 模型标识符（如 "google/gemini-3.1-pro"）
    #[serde(default)]
    pub model: Option<String>,

    /// 模型变体（如 "max", "xhigh", "high", "medium", "low"）
    #[serde(default)]
    pub variant: Option<String>,

    /// 采样温度（0.0-2.0），越低越确定性
    #[serde(default)]
    pub temperature: Option<f32>,

    /// Top-p 核采样参数（0.0-1.0）
    #[serde(default)]
    pub top_p: Option<f32>,

    /// 追加到系统提示词的额外内容
    #[serde(default)]
    pub prompt_append: Option<String>,

    /// 思考链配置
    #[serde(default)]
    pub thinking: Option<ThinkingConfig>,

    /// 推理努力级别
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,

    /// 文本详细程度
    #[serde(default)]
    pub text_verbosity: Option<Verbosity>,

    /// 工具使用控制（true=启用, false=禁用）
    #[serde(default)]
    pub tools: HashMap<String, bool>,

    /// 最大响应 token 数
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// 标记为不稳定 Agent（强制后台模式）
    #[serde(default)]
    pub is_unstable_agent: bool,
}

impl Default for CategoryConfig {
    fn default() -> Self {
        Self {
            description: String::new(),
            model: None,
            variant: None,
            temperature: None,
            top_p: None,
            prompt_append: None,
            thinking: None,
            reasoning_effort: None,
            text_verbosity: None,
            tools: HashMap::new(),
            max_tokens: None,
            is_unstable_agent: false,
        }
    }
}

impl CategoryConfig {
    /// 返回生效的模型标识符，优先使用自定义配置，回退到提供的默认值
    pub fn effective_model(&self, default: &str) -> String {
        self.model.clone().unwrap_or_else(|| default.to_string())
    }

    /// 返回生效的变体
    pub fn effective_variant(&self, default: Option<&str>) -> Option<String> {
        self.variant
            .clone()
            .or_else(|| default.map(|s| s.to_string()))
    }

    /// 返回生效的温度参数
    pub fn effective_temperature(&self, default: f32) -> f32 {
        self.temperature.unwrap_or(default)
    }

    /// 合并另一个配置（用于覆盖式配置合并）
    ///
    /// `other` 中的非空字段会覆盖 `self` 中的对应字段。
    pub fn merge_with(&mut self, other: &CategoryConfig) {
        if other.model.is_some() {
            self.model = other.model.clone();
        }
        if other.variant.is_some() {
            self.variant = other.variant.clone();
        }
        if other.temperature.is_some() {
            self.temperature = other.temperature;
        }
        if other.top_p.is_some() {
            self.top_p = other.top_p;
        }
        if other.prompt_append.is_some() {
            self.prompt_append = other.prompt_append.clone();
        }
        if other.thinking.is_some() {
            self.thinking = other.thinking.clone();
        }
        if other.reasoning_effort.is_some() {
            self.reasoning_effort = other.reasoning_effort;
        }
        if other.text_verbosity.is_some() {
            self.text_verbosity = other.text_verbosity;
        }
        if other.max_tokens.is_some() {
            self.max_tokens = other.max_tokens;
        }
        // 工具配置逐项合并
        for (tool, enabled) in &other.tools {
            self.tools.insert(tool.clone(), *enabled);
        }
        if other.is_unstable_agent {
            self.is_unstable_agent = true;
        }
    }
}

// ============================================================
// ThinkingConfig — 思考链配置
// ============================================================

/// 扩展思考（Extended Thinking）配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// 思考类型（"enabled" / "disabled"）
    #[serde(rename = "type")]
    pub thinking_type: String,

    /// 思考预算 token 数
    #[serde(default)]
    pub budget_tokens: Option<u32>,
}

// ============================================================
// ReasoningEffort — 推理努力级别
// ============================================================

/// 推理努力级别枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    /// 低努力——快速响应
    Low,
    /// 中等努力——平衡速度与质量
    Medium,
    /// 高努力——深度推理
    High,
    /// 极高努力——最深度推理
    #[serde(rename = "xhigh")]
    XHigh,
}

// ============================================================
// Verbosity — 文本详细程度
// ============================================================

/// 文本输出的详细程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verbosity {
    /// 简洁输出
    Low,
    /// 适中详细
    Medium,
    /// 详细输出
    High,
}

// ============================================================
// BuiltinCategory — 内置类别定义
// ============================================================

/// 内置类别描述（不可变的默认值）
struct BuiltinCategory {
    /// 类别标识名
    name: &'static str,
    /// 描述
    description: &'static str,
    /// 默认模型
    default_model: &'static str,
    /// 默认变体
    default_variant: Option<&'static str>,
    /// 默认温度
    default_temperature: f32,
}

/// 所有内置类别的定义
const BUILTIN_CATEGORIES: &[BuiltinCategory] = &[
    BuiltinCategory {
        name: "visual-engineering",
        description: "前端、UI/UX、设计、样式、动画等视觉工程任务",
        default_model: "google/gemini-3.1-pro",
        default_variant: Some("high"),
        default_temperature: 0.3,
    },
    BuiltinCategory {
        name: "ultrabrain",
        description: "深度逻辑推理，复杂架构决策，需要大量分析的任务",
        default_model: "openai/gpt-5.4",
        default_variant: Some("xhigh"),
        default_temperature: 0.1,
    },
    BuiltinCategory {
        name: "deep",
        description: "目标驱动的自主问题解决，深度研究后再行动",
        default_model: "openai/gpt-5.4",
        default_variant: Some("medium"),
        default_temperature: 0.1,
    },
    BuiltinCategory {
        name: "artistry",
        description: "高度创意/艺术性任务，新颖想法",
        default_model: "google/gemini-3.1-pro",
        default_variant: Some("high"),
        default_temperature: 0.7,
    },
    BuiltinCategory {
        name: "quick",
        description: "简单任务——单文件修改、拼写修复、简单修改",
        default_model: "openai/gpt-5.4-mini",
        default_variant: None,
        default_temperature: 0.1,
    },
    BuiltinCategory {
        name: "unspecified-low",
        description: "不适合其他类别的通用任务，低难度",
        default_model: "anthropic/claude-sonnet-4-6",
        default_variant: None,
        default_temperature: 0.1,
    },
    BuiltinCategory {
        name: "unspecified-high",
        description: "不适合其他类别的通用任务，高难度",
        default_model: "anthropic/claude-opus-4-6",
        default_variant: Some("max"),
        default_temperature: 0.1,
    },
    BuiltinCategory {
        name: "writing",
        description: "文档、散文、技术写作",
        default_model: "google/gemini-3-flash",
        default_variant: None,
        default_temperature: 0.5,
    },
];

// ============================================================
// CategoryRegistry — 类别注册表
// ============================================================

/// 类别注册表，管理所有可用的任务类别及其配置。
///
/// 在系统启动时加载内置类别，随后合并用户自定义配置。
pub struct CategoryRegistry {
    /// 类别名称到配置的映射
    categories: HashMap<String, ResolvedCategory>,
}

/// 已解析的类别——合并了内置默认值和用户覆盖
#[derive(Debug, Clone)]
pub struct ResolvedCategory {
    /// 类别名称
    pub name: String,
    /// 类别描述
    pub description: String,
    /// 内置默认模型
    pub default_model: String,
    /// 内置默认变体
    pub default_variant: Option<String>,
    /// 内置默认温度
    pub default_temperature: f32,
    /// 用户覆盖配置（如有）
    pub user_override: Option<CategoryConfig>,
}

impl ResolvedCategory {
    /// 返回生效的模型标识符
    pub fn effective_model(&self) -> String {
        if let Some(ref overr) = self.user_override {
            overr.effective_model(&self.default_model)
        } else {
            self.default_model.clone()
        }
    }

    /// 返回生效的变体
    pub fn effective_variant(&self) -> Option<String> {
        if let Some(ref overr) = self.user_override {
            overr.effective_variant(self.default_variant.as_deref())
        } else {
            self.default_variant.clone()
        }
    }

    /// 返回生效的温度参数
    pub fn effective_temperature(&self) -> f32 {
        if let Some(ref overr) = self.user_override {
            overr.effective_temperature(self.default_temperature)
        } else {
            self.default_temperature
        }
    }
}

impl CategoryRegistry {
    /// 创建空的类别注册表
    pub fn new() -> Self {
        Self {
            categories: HashMap::new(),
        }
    }

    /// 返回已注册的类别数量
    pub fn len(&self) -> usize {
        self.categories.len()
    }

    /// 注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }

    /// 通过名称获取已解析的类别
    pub fn get(&self, name: &str) -> Option<&ResolvedCategory> {
        self.categories.get(name)
    }

    /// 覆盖指定类别的配置
    ///
    /// 如果类别已存在，将合并用户覆盖；如果不存在，创建新的自定义类别。
    pub fn override_category(&mut self, name: &str, config: CategoryConfig) {
        if let Some(existing) = self.categories.get_mut(name) {
            existing.user_override = Some(config);
        } else {
            // 自定义类别——必须提供模型
            let model = config.model.clone().unwrap_or_default();
            self.categories.insert(
                name.to_string(),
                ResolvedCategory {
                    name: name.to_string(),
                    description: config.description.clone(),
                    default_model: model,
                    default_variant: config.variant.clone(),
                    default_temperature: config.temperature.unwrap_or(0.1),
                    user_override: Some(config),
                },
            );
        }
    }

    /// 返回所有已注册类别名称（排序后）
    pub fn category_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.categories.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for CategoryRegistry {
    /// 创建包含所有内置类别的默认注册表
    fn default() -> Self {
        let mut registry = Self::new();
        for builtin in BUILTIN_CATEGORIES {
            registry.categories.insert(
                builtin.name.to_string(),
                ResolvedCategory {
                    name: builtin.name.to_string(),
                    description: builtin.description.to_string(),
                    default_model: builtin.default_model.to_string(),
                    default_variant: builtin.default_variant.map(|s| s.to_string()),
                    default_temperature: builtin.default_temperature,
                    user_override: None,
                },
            );
        }
        registry
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试默认注册表包含 8 个内置类别
    #[test]
    fn test_default_registry_has_8_categories() {
        let registry = CategoryRegistry::default();
        assert_eq!(registry.len(), 8);
    }

    /// 测试所有内置类别都存在
    #[test]
    fn test_all_builtin_categories_exist() {
        let registry = CategoryRegistry::default();
        let expected = vec![
            "visual-engineering",
            "ultrabrain",
            "deep",
            "artistry",
            "quick",
            "unspecified-low",
            "unspecified-high",
            "writing",
        ];
        for name in expected {
            assert!(registry.get(name).is_some(), "内置类别 '{}' 应存在", name);
        }
    }

    /// 测试 quick 类别使用 mini 模型
    #[test]
    fn test_quick_category_uses_mini_model() {
        let registry = CategoryRegistry::default();
        let quick = registry.get("quick").unwrap();
        assert!(quick.default_model.contains("mini"));
        assert!(quick.effective_model().contains("mini"));
    }

    /// 测试 ultrabrain 类别配置
    #[test]
    fn test_ultrabrain_category() {
        let registry = CategoryRegistry::default();
        let ultra = registry.get("ultrabrain").unwrap();
        assert!(ultra.default_model.contains("gpt-5.4"));
        assert_eq!(ultra.effective_variant().as_deref(), Some("xhigh"));
        assert_eq!(ultra.effective_temperature(), 0.1);
    }

    /// 测试类别覆盖——修改已有类别
    #[test]
    fn test_category_override_existing() {
        let mut registry = CategoryRegistry::default();
        registry.override_category(
            "quick",
            CategoryConfig {
                model: Some("openai/gpt-5.4".into()),
                temperature: Some(0.5),
                ..Default::default()
            },
        );
        let quick = registry.get("quick").unwrap();
        assert_eq!(quick.effective_model(), "openai/gpt-5.4");
        assert_eq!(quick.effective_temperature(), 0.5);
    }

    /// 测试类别覆盖——添加自定义类别
    #[test]
    fn test_category_override_custom() {
        let mut registry = CategoryRegistry::default();
        registry.override_category(
            "korean-writer",
            CategoryConfig {
                description: "韩语技术写作".into(),
                model: Some("google/gemini-3-flash".into()),
                temperature: Some(0.5),
                ..Default::default()
            },
        );
        assert_eq!(registry.len(), 9);
        let custom = registry.get("korean-writer").unwrap();
        assert_eq!(custom.effective_model(), "google/gemini-3-flash");
    }

    /// 测试 CategoryConfig 的合并逻辑
    #[test]
    fn test_category_config_merge() {
        let mut base = CategoryConfig {
            model: Some("model-a".into()),
            temperature: Some(0.5),
            ..Default::default()
        };
        let override_cfg = CategoryConfig {
            model: Some("model-b".into()),
            // temperature 未设置，不应覆盖
            ..Default::default()
        };
        base.merge_with(&override_cfg);
        assert_eq!(base.model.as_deref(), Some("model-b"));
        assert_eq!(base.temperature, Some(0.5)); // 保留原值
    }

    /// 测试 CategoryConfig 默认值
    #[test]
    fn test_category_config_defaults() {
        let config = CategoryConfig::default();
        assert!(config.model.is_none());
        assert!(config.variant.is_none());
        assert!(config.temperature.is_none());
        assert!(!config.is_unstable_agent);
    }

    /// 测试 effective_model 回退到默认值
    #[test]
    fn test_effective_model_fallback() {
        let config = CategoryConfig::default();
        assert_eq!(config.effective_model("fallback-model"), "fallback-model");

        let config = CategoryConfig {
            model: Some("override-model".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_model("fallback-model"), "override-model");
    }

    /// 测试 ReasoningEffort 序列化
    #[test]
    fn test_reasoning_effort_values() {
        let efforts = vec![
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh,
        ];
        // 确保所有变体都是有效的
        assert_eq!(efforts.len(), 4);
        assert_ne!(ReasoningEffort::Low, ReasoningEffort::High);
    }

    /// 测试类别名称排序输出
    #[test]
    fn test_category_names_sorted() {
        let registry = CategoryRegistry::default();
        let names = registry.category_names();
        assert_eq!(names.len(), 8);
        // 验证排序
        for window in names.windows(2) {
            assert!(window[0] <= window[1]);
        }
    }

    /// 测试工具控制配置
    #[test]
    fn test_tool_control() {
        let config = CategoryConfig {
            tools: {
                let mut m = HashMap::new();
                m.insert("websearch".into(), false);
                m.insert("bash".into(), true);
                m
            },
            ..Default::default()
        };
        assert_eq!(config.tools.get("websearch"), Some(&false));
        assert_eq!(config.tools.get("bash"), Some(&true));
        assert_eq!(config.tools.get("unknown"), None);
    }

    /// 测试工具配置合并
    #[test]
    fn test_tool_config_merge() {
        let mut base = CategoryConfig {
            tools: {
                let mut m = HashMap::new();
                m.insert("bash".into(), true);
                m.insert("websearch".into(), true);
                m
            },
            ..Default::default()
        };
        let override_cfg = CategoryConfig {
            tools: {
                let mut m = HashMap::new();
                m.insert("websearch".into(), false); // 覆盖
                m.insert("grep".into(), true); // 新增
                m
            },
            ..Default::default()
        };
        base.merge_with(&override_cfg);
        assert_eq!(base.tools.get("bash"), Some(&true)); // 保留
        assert_eq!(base.tools.get("websearch"), Some(&false)); // 覆盖
        assert_eq!(base.tools.get("grep"), Some(&true)); // 新增
    }

    /// 测试空注册表
    #[test]
    fn test_empty_registry() {
        let registry = CategoryRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.get("quick").is_none());
    }

    /// 测试 visual-engineering 类别使用 Gemini
    #[test]
    fn test_visual_engineering_uses_gemini() {
        let registry = CategoryRegistry::default();
        let ve = registry.get("visual-engineering").unwrap();
        assert!(ve.default_model.contains("gemini"));
        assert_eq!(ve.effective_variant().as_deref(), Some("high"));
    }

    /// 测试 writing 类别使用 flash 模型
    #[test]
    fn test_writing_category() {
        let registry = CategoryRegistry::default();
        let w = registry.get("writing").unwrap();
        assert!(w.default_model.contains("flash"));
        assert_eq!(w.effective_temperature(), 0.5);
    }
}
