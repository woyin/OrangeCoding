//! 模型路由模块
//!
//! 根据任务类型和配置的路由规则，动态选择最佳的 AI 模型/提供商。
//! 支持基于任务类型、复杂度阈值等条件进行灵活的路由匹配。

use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// 任务类型
// ---------------------------------------------------------------------------

/// 任务类型 - 描述代理需要执行的任务类别
///
/// 不同任务类型可能需要不同的 AI 模型来获得最佳效果，
/// 例如代码生成任务可能需要更强的推理模型，而文档任务则注重文本质量。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// 编码任务 - 编写、修改或重构代码
    Coding,
    /// 审查任务 - 审查代码质量和正确性
    Review,
    /// 规划任务 - 制定计划、分解任务
    Planning,
    /// 文档任务 - 编写文档、注释或说明
    Documentation,
    /// 测试任务 - 编写和运行测试
    Testing,
    /// 通用任务 - 其他未归类的任务
    General,
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            TaskType::Coding => "编码",
            TaskType::Review => "审查",
            TaskType::Planning => "规划",
            TaskType::Documentation => "文档",
            TaskType::Testing => "测试",
            TaskType::General => "通用",
        };
        write!(f, "{label}")
    }
}

// ---------------------------------------------------------------------------
// 路由条件
// ---------------------------------------------------------------------------

/// 路由条件 - 定义路由规则的匹配条件
///
/// 可以基于任务类型、复杂度阈值或自定义标签进行匹配。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingCondition {
    /// 匹配指定的任务类型
    TaskTypeMatch(TaskType),
    /// 复杂度阈值匹配 - 当任务复杂度大于等于阈值时匹配
    ComplexityThreshold(u32),
    /// 自定义标签匹配
    Tag(String),
    /// 匹配所有任务（兜底规则）
    Any,
}

impl RoutingCondition {
    /// 检查此条件是否与给定的路由上下文匹配
    fn matches(&self, context: &RoutingContext) -> bool {
        match self {
            RoutingCondition::TaskTypeMatch(task_type) => &context.task_type == task_type,
            RoutingCondition::ComplexityThreshold(threshold) => context.complexity >= *threshold,
            RoutingCondition::Tag(tag) => context.tags.contains(tag),
            RoutingCondition::Any => true,
        }
    }
}

// ---------------------------------------------------------------------------
// 路由上下文
// ---------------------------------------------------------------------------

/// 路由上下文 - 路由决策时使用的上下文信息
///
/// 包含任务类型、复杂度评分和自定义标签等信息，
/// 路由器根据这些信息匹配最合适的路由规则。
#[derive(Clone, Debug)]
pub struct RoutingContext {
    /// 当前任务的类型
    pub task_type: TaskType,
    /// 任务复杂度评分（0-100），数值越高表示越复杂
    pub complexity: u32,
    /// 自定义标签列表，用于精细化路由控制
    pub tags: Vec<String>,
}

impl RoutingContext {
    /// 创建一个基本的路由上下文（默认复杂度为 50，无标签）
    pub fn new(task_type: TaskType) -> Self {
        Self {
            task_type,
            complexity: 50,
            tags: Vec::new(),
        }
    }

    /// 创建一个带完整参数的路由上下文
    pub fn with_details(task_type: TaskType, complexity: u32, tags: Vec<String>) -> Self {
        Self {
            task_type,
            complexity: complexity.min(100),
            tags,
        }
    }
}

// ---------------------------------------------------------------------------
// 路由规则
// ---------------------------------------------------------------------------

/// 路由规则 - 定义条件到模型/提供商的映射
///
/// 每条规则包含一个匹配条件、目标提供商和模型名称以及优先级。
/// 优先级数值越高，规则越优先被选用。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingRule {
    /// 规则的唯一标识名称
    pub name: String,
    /// 匹配条件
    pub condition: RoutingCondition,
    /// AI 提供商名称（如 "openai"、"anthropic"）
    pub provider_name: String,
    /// 模型名称（如 "gpt-4"、"claude-3-opus"）
    pub model_name: String,
    /// 规则优先级，数值越大优先级越高
    pub priority: u32,
}

impl RoutingRule {
    /// 创建一个新的路由规则
    pub fn new(
        name: impl Into<String>,
        condition: RoutingCondition,
        provider_name: impl Into<String>,
        model_name: impl Into<String>,
        priority: u32,
    ) -> Self {
        Self {
            name: name.into(),
            condition,
            provider_name: provider_name.into(),
            model_name: model_name.into(),
            priority,
        }
    }
}

impl fmt::Display for RoutingRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "规则[{}] -> {}/{} (优先级: {})",
            self.name, self.provider_name, self.model_name, self.priority
        )
    }
}

// ---------------------------------------------------------------------------
// 路由决策
// ---------------------------------------------------------------------------

/// 路由决策 - 路由器的输出结果
///
/// 包含选中的提供商、模型名称以及匹配的规则名称。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingDecision {
    /// 选中的 AI 提供商名称
    pub provider_name: String,
    /// 选中的模型名称
    pub model_name: String,
    /// 匹配到的规则名称
    pub matched_rule: String,
}

impl fmt::Display for RoutingDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{} (匹配规则: {})",
            self.provider_name, self.model_name, self.matched_rule
        )
    }
}

// ---------------------------------------------------------------------------
// 模型路由器
// ---------------------------------------------------------------------------

/// 模型路由器 - 根据任务上下文动态选择最佳 AI 模型
///
/// 管理一组路由规则，按优先级从高到低匹配，返回第一个匹配的规则对应的模型。
/// 使用 `RwLock` 保护规则列表以支持并发访问。
///
/// # 示例
///
/// ```rust
/// use chengcoding_mesh::model_router::{ModelRouter, RoutingRule, RoutingCondition, TaskType, RoutingContext};
///
/// let mut router = ModelRouter::new();
/// router.add_rule(RoutingRule::new(
///     "coding-rule",
///     RoutingCondition::TaskTypeMatch(TaskType::Coding),
///     "openai",
///     "gpt-4",
///     100,
/// ));
///
/// let ctx = RoutingContext::new(TaskType::Coding);
/// let decision = router.route(&ctx).unwrap();
/// assert_eq!(decision.model_name, "gpt-4");
/// ```
#[derive(Debug)]
pub struct ModelRouter {
    /// 路由规则列表，受读写锁保护
    rules: RwLock<Vec<RoutingRule>>,
    /// 默认提供商名称（当没有规则匹配时使用）
    default_provider: String,
    /// 默认模型名称（当没有规则匹配时使用）
    default_model: String,
}

impl ModelRouter {
    /// 创建一个新的模型路由器
    ///
    /// 默认使用 "openai" 提供商和 "gpt-4" 模型作为兜底选项。
    pub fn new() -> Self {
        debug!("创建新的模型路由器");
        Self {
            rules: RwLock::new(Vec::new()),
            default_provider: "openai".to_string(),
            default_model: "gpt-4".to_string(),
        }
    }

    /// 创建一个带自定义默认模型的路由器
    pub fn with_defaults(
        default_provider: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
            default_provider: default_provider.into(),
            default_model: default_model.into(),
        }
    }

    /// 添加一条路由规则
    pub fn add_rule(&self, rule: RoutingRule) {
        info!(rule_name = %rule.name, priority = rule.priority, "添加路由规则");
        let mut rules = self.rules.write();
        rules.push(rule);
        // 按优先级降序排序，确保高优先级规则优先匹配
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// 移除指定名称的路由规则
    ///
    /// 返回是否成功移除。
    pub fn remove_rule(&self, rule_name: &str) -> bool {
        let mut rules = self.rules.write();
        let original_len = rules.len();
        rules.retain(|r| r.name != rule_name);
        let removed = rules.len() < original_len;
        if removed {
            info!(rule_name = %rule_name, "移除路由规则");
        }
        removed
    }

    /// 根据路由上下文选择最佳模型
    ///
    /// 按优先级从高到低遍历规则，返回第一个匹配的规则对应的模型。
    /// 如果没有规则匹配，返回 `None`。
    pub fn route(&self, context: &RoutingContext) -> Option<RoutingDecision> {
        let rules = self.rules.read();

        for rule in rules.iter() {
            if rule.condition.matches(context) {
                debug!(
                    rule_name = %rule.name,
                    provider = %rule.provider_name,
                    model = %rule.model_name,
                    "路由匹配成功"
                );
                return Some(RoutingDecision {
                    provider_name: rule.provider_name.clone(),
                    model_name: rule.model_name.clone(),
                    matched_rule: rule.name.clone(),
                });
            }
        }

        debug!("没有匹配的路由规则");
        None
    }

    /// 根据路由上下文选择模型，无匹配时使用默认值
    pub fn route_or_default(&self, context: &RoutingContext) -> RoutingDecision {
        self.route(context).unwrap_or_else(|| {
            debug!(
                provider = %self.default_provider,
                model = %self.default_model,
                "使用默认路由"
            );
            RoutingDecision {
                provider_name: self.default_provider.clone(),
                model_name: self.default_model.clone(),
                matched_rule: "<default>".to_string(),
            }
        })
    }

    /// 获取当前路由规则的数量
    pub fn rule_count(&self) -> usize {
        self.rules.read().len()
    }

    /// 获取所有路由规则的副本
    pub fn list_rules(&self) -> Vec<RoutingRule> {
        self.rules.read().clone()
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试任务类型的显示() {
        assert_eq!(format!("{}", TaskType::Coding), "编码");
        assert_eq!(format!("{}", TaskType::Review), "审查");
        assert_eq!(format!("{}", TaskType::Planning), "规划");
        assert_eq!(format!("{}", TaskType::Documentation), "文档");
        assert_eq!(format!("{}", TaskType::Testing), "测试");
        assert_eq!(format!("{}", TaskType::General), "通用");
    }

    #[test]
    fn 测试任务类型的序列化() {
        let json = serde_json::to_string(&TaskType::Coding).unwrap();
        assert_eq!(json, "\"coding\"");

        let deserialized: TaskType = serde_json::from_str("\"review\"").unwrap();
        assert_eq!(deserialized, TaskType::Review);
    }

    #[test]
    fn 测试路由上下文的创建() {
        let ctx = RoutingContext::new(TaskType::Coding);
        assert_eq!(ctx.task_type, TaskType::Coding);
        assert_eq!(ctx.complexity, 50);
        assert!(ctx.tags.is_empty());
    }

    #[test]
    fn 测试路由上下文的复杂度上限() {
        let ctx = RoutingContext::with_details(TaskType::General, 200, vec![]);
        // 复杂度应该被限制在 100 以内
        assert_eq!(ctx.complexity, 100);
    }

    #[test]
    fn 测试添加和匹配路由规则() {
        let router = ModelRouter::new();

        router.add_rule(RoutingRule::new(
            "coding-gpt4",
            RoutingCondition::TaskTypeMatch(TaskType::Coding),
            "openai",
            "gpt-4",
            100,
        ));

        let ctx = RoutingContext::new(TaskType::Coding);
        let decision = router.route(&ctx).unwrap();

        assert_eq!(decision.provider_name, "openai");
        assert_eq!(decision.model_name, "gpt-4");
        assert_eq!(decision.matched_rule, "coding-gpt4");
    }

    #[test]
    fn 测试无匹配规则时返回None() {
        let router = ModelRouter::new();

        router.add_rule(RoutingRule::new(
            "coding-only",
            RoutingCondition::TaskTypeMatch(TaskType::Coding),
            "openai",
            "gpt-4",
            100,
        ));

        let ctx = RoutingContext::new(TaskType::Review);
        assert!(router.route(&ctx).is_none());
    }

    #[test]
    fn 测试默认路由() {
        let router = ModelRouter::with_defaults("anthropic", "claude-3-sonnet");
        let ctx = RoutingContext::new(TaskType::General);

        let decision = router.route_or_default(&ctx);
        assert_eq!(decision.provider_name, "anthropic");
        assert_eq!(decision.model_name, "claude-3-sonnet");
        assert_eq!(decision.matched_rule, "<default>");
    }

    #[test]
    fn 测试优先级排序() {
        let router = ModelRouter::new();

        // 低优先级规则
        router.add_rule(RoutingRule::new(
            "low-priority",
            RoutingCondition::TaskTypeMatch(TaskType::Coding),
            "openai",
            "gpt-3.5-turbo",
            10,
        ));

        // 高优先级规则
        router.add_rule(RoutingRule::new(
            "high-priority",
            RoutingCondition::TaskTypeMatch(TaskType::Coding),
            "openai",
            "gpt-4",
            100,
        ));

        let ctx = RoutingContext::new(TaskType::Coding);
        let decision = router.route(&ctx).unwrap();

        // 应该匹配高优先级规则
        assert_eq!(decision.model_name, "gpt-4");
        assert_eq!(decision.matched_rule, "high-priority");
    }

    #[test]
    fn 测试复杂度阈值路由() {
        let router = ModelRouter::new();

        // 高复杂度使用高端模型
        router.add_rule(RoutingRule::new(
            "complex-task",
            RoutingCondition::ComplexityThreshold(80),
            "openai",
            "gpt-4",
            100,
        ));

        // 低复杂度使用轻量模型
        router.add_rule(RoutingRule::new(
            "simple-task",
            RoutingCondition::Any,
            "openai",
            "gpt-3.5-turbo",
            10,
        ));

        // 高复杂度任务
        let complex_ctx = RoutingContext::with_details(TaskType::Coding, 90, vec![]);
        let decision = router.route(&complex_ctx).unwrap();
        assert_eq!(decision.model_name, "gpt-4");

        // 低复杂度任务
        let simple_ctx = RoutingContext::with_details(TaskType::Coding, 30, vec![]);
        let decision = router.route(&simple_ctx).unwrap();
        assert_eq!(decision.model_name, "gpt-3.5-turbo");
    }

    #[test]
    fn 测试标签匹配路由() {
        let router = ModelRouter::new();

        router.add_rule(RoutingRule::new(
            "fast-mode",
            RoutingCondition::Tag("fast".to_string()),
            "openai",
            "gpt-3.5-turbo",
            100,
        ));

        let ctx_with_tag =
            RoutingContext::with_details(TaskType::General, 50, vec!["fast".to_string()]);
        let decision = router.route(&ctx_with_tag).unwrap();
        assert_eq!(decision.model_name, "gpt-3.5-turbo");

        let ctx_without_tag = RoutingContext::new(TaskType::General);
        assert!(router.route(&ctx_without_tag).is_none());
    }

    #[test]
    fn 测试Any条件匹配所有() {
        let router = ModelRouter::new();

        router.add_rule(RoutingRule::new(
            "catch-all",
            RoutingCondition::Any,
            "openai",
            "gpt-4",
            1,
        ));

        // 任何任务类型都应该匹配
        for task_type in [
            TaskType::Coding,
            TaskType::Review,
            TaskType::Planning,
            TaskType::Documentation,
            TaskType::Testing,
            TaskType::General,
        ] {
            let ctx = RoutingContext::new(task_type);
            assert!(router.route(&ctx).is_some());
        }
    }

    #[test]
    fn 测试移除路由规则() {
        let router = ModelRouter::new();

        router.add_rule(RoutingRule::new(
            "to-remove",
            RoutingCondition::Any,
            "openai",
            "gpt-4",
            100,
        ));

        assert_eq!(router.rule_count(), 1);

        assert!(router.remove_rule("to-remove"));
        assert_eq!(router.rule_count(), 0);

        // 移除不存在的规则应返回 false
        assert!(!router.remove_rule("not-exists"));
    }

    #[test]
    fn 测试列出所有规则() {
        let router = ModelRouter::new();

        router.add_rule(RoutingRule::new(
            "rule1",
            RoutingCondition::Any,
            "openai",
            "gpt-4",
            100,
        ));
        router.add_rule(RoutingRule::new(
            "rule2",
            RoutingCondition::Any,
            "anthropic",
            "claude-3",
            50,
        ));

        let rules = router.list_rules();
        assert_eq!(rules.len(), 2);
        // 验证按优先级降序排列
        assert_eq!(rules[0].name, "rule1");
        assert_eq!(rules[1].name, "rule2");
    }

    #[test]
    fn 测试路由规则的显示() {
        let rule = RoutingRule::new("test-rule", RoutingCondition::Any, "openai", "gpt-4", 100);
        let display = format!("{rule}");
        assert!(display.contains("test-rule"));
        assert!(display.contains("openai"));
        assert!(display.contains("gpt-4"));
    }

    #[test]
    fn 测试路由决策的显示() {
        let decision = RoutingDecision {
            provider_name: "openai".to_string(),
            model_name: "gpt-4".to_string(),
            matched_rule: "coding-rule".to_string(),
        };
        let display = format!("{decision}");
        assert!(display.contains("openai/gpt-4"));
    }

    #[test]
    fn 测试默认构造() {
        let router = ModelRouter::default();
        assert_eq!(router.rule_count(), 0);
    }
}
