//! # 模型路由与运行时配置
//!
//! 从 `orange.json` 读取执行期配置，并按任务难度与类型选择模型。

use orangecoding_core::{OrangeError, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::Path;

/// 任务难度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    /// 简单任务
    Easy,
    /// 中等任务
    Medium,
    /// 困难任务
    Hard,
    /// 史诗级任务
    Epic,
}

impl Difficulty {
    /// 从用户输入推断任务难度。
    pub fn infer(text: &str) -> Self {
        let lower = text.to_lowercase();

        if let Some(explicit) = infer_explicit_difficulty(&lower) {
            return explicit;
        }

        if contains_any(&lower, &["史诗", "epic", "平台", "完整系统", "长任务"]) {
            return Difficulty::Epic;
        }
        if contains_any(&lower, &["复杂", "hard", "架构", "重构"]) {
            return Difficulty::Hard;
        }
        if text.chars().count() > 120 && !matches!(TaskType::infer(text), TaskType::Chat) {
            return Difficulty::Medium;
        }

        Difficulty::Easy
    }
}

/// 任务类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    /// 编码、构建、测试或修复任务
    Code,
    /// 文档或写作任务
    Write,
    /// 分析、排查或诊断任务
    Analyze,
    /// 普通对话任务
    Chat,
}

impl TaskType {
    /// 从用户输入推断任务类型。
    pub fn infer(text: &str) -> Self {
        let lower = text.to_lowercase();

        if contains_any(
            &lower,
            &[
                "code", "build", "test", "fix", "编译", "代码", "修复", "测试",
            ],
        ) {
            TaskType::Code
        } else if contains_any(&lower, &["doc", "write", "文档", "撰写", "设计文稿"]) {
            TaskType::Write
        } else if contains_any(&lower, &["analyze", "分析", "排查", "诊断"]) {
            TaskType::Analyze
        } else {
            TaskType::Chat
        }
    }
}

/// 单条模型路由规则。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingRule {
    /// 匹配的难度，`None` 表示通配。
    #[serde(
        default,
        deserialize_with = "deserialize_optional_difficulty",
        serialize_with = "serialize_optional_difficulty"
    )]
    pub difficulty: Option<Difficulty>,
    /// 匹配的任务类型，`None` 表示通配。
    #[serde(
        default,
        deserialize_with = "deserialize_optional_task_type",
        serialize_with = "serialize_optional_task_type"
    )]
    pub task_type: Option<TaskType>,
    /// 命中的模型名称。
    pub model: String,
}

impl RoutingRule {
    /// 创建一条路由规则。
    pub fn new(
        difficulty: Option<Difficulty>,
        task_type: Option<TaskType>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            difficulty,
            task_type,
            model: model.into(),
        }
    }
}

/// 模型路由器。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelRouter {
    /// 按优先级扫描的路由规则。
    pub rules: Vec<RoutingRule>,
    /// 未命中任何规则时使用的模型。
    #[serde(rename = "fallback_model")]
    pub fallback: String,
}

impl ModelRouter {
    /// 根据难度和任务类型选择模型。
    pub fn route(&self, difficulty: Difficulty, task_type: TaskType) -> &str {
        if let Some(rule) = self
            .rules
            .iter()
            .find(|rule| rule.difficulty == Some(difficulty) && rule.task_type == Some(task_type))
        {
            return &rule.model;
        }

        if let Some(rule) = self.rules.iter().find(|rule| {
            (rule.difficulty == Some(difficulty) && rule.task_type.is_none())
                || (rule.difficulty.is_none() && rule.task_type == Some(task_type))
        }) {
            return &rule.model;
        }

        if let Some(rule) = self
            .rules
            .iter()
            .find(|rule| rule.difficulty.is_none() && rule.task_type.is_none())
        {
            return &rule.model;
        }

        &self.fallback
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self {
            rules: vec![
                RoutingRule::new(
                    Some(Difficulty::Easy),
                    Some(TaskType::Chat),
                    "deepseek-chat",
                ),
                RoutingRule::new(
                    Some(Difficulty::Easy),
                    Some(TaskType::Code),
                    "deepseek-chat",
                ),
                RoutingRule::new(
                    Some(Difficulty::Medium),
                    Some(TaskType::Code),
                    "deepseek-coder",
                ),
                RoutingRule::new(
                    Some(Difficulty::Medium),
                    Some(TaskType::Analyze),
                    "deepseek-coder",
                ),
                RoutingRule::new(
                    Some(Difficulty::Hard),
                    Some(TaskType::Code),
                    "claude-sonnet-4-5",
                ),
                RoutingRule::new(Some(Difficulty::Hard), None, "claude-sonnet-4-5"),
                RoutingRule::new(Some(Difficulty::Epic), None, "claude-opus-4-7"),
            ],
            fallback: "deepseek-chat".to_string(),
        }
    }
}

/// `orange.json` 根配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrangeRuntimeConfig {
    /// 模型路由配置。
    #[serde(default, rename = "model_routing")]
    pub routing: ModelRouter,
    /// 执行期控制配置。
    #[serde(default)]
    pub execution: ExecutionRuntimeConfig,
}

impl OrangeRuntimeConfig {
    /// 从指定路径加载运行时配置。
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|err| OrangeError::config_with_source("解析 orange.json 失败", err))
    }

    /// 从指定路径加载配置，缺失或无效时回退到默认配置。
    pub fn load_or_default(path: &Path) -> Self {
        match Self::load(path) {
            Ok(config) => config,
            Err(err) => {
                if path.exists() {
                    tracing::warn!(error = %err, path = %path.display(), "orange.json 无效，使用默认运行时配置");
                }
                Self::default()
            }
        }
    }

    /// 返回模型路由器。
    pub fn model_router(&self) -> &ModelRouter {
        &self.routing
    }
}

impl Default for OrangeRuntimeConfig {
    fn default() -> Self {
        Self {
            routing: ModelRouter::default(),
            execution: ExecutionRuntimeConfig::default(),
        }
    }
}

/// 执行期控制配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExecutionRuntimeConfig {
    /// 多少步后插入一次指令回锚。
    pub anchor_interval_steps: u32,
    /// 初始步骤预算。
    pub step_budget_initial: u32,
    /// 循环检测阈值。
    pub loop_detection_threshold: u32,
}

impl Default for ExecutionRuntimeConfig {
    fn default() -> Self {
        Self {
            anchor_interval_steps: 5,
            step_budget_initial: 100,
            loop_detection_threshold: 3,
        }
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn infer_explicit_difficulty(text: &str) -> Option<Difficulty> {
    let marker = "difficulty:";
    let (_, value) = text.split_once(marker)?;
    let difficulty = value
        .trim_start()
        .trim_start_matches(|ch: char| matches!(ch, '"' | '\'' | ':' | '='))
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>();

    match difficulty.as_str() {
        "easy" => Some(Difficulty::Easy),
        "medium" => Some(Difficulty::Medium),
        "hard" => Some(Difficulty::Hard),
        "epic" => Some(Difficulty::Epic),
        _ => None,
    }
}

fn deserialize_optional_difficulty<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Difficulty>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_wildcard_option(deserializer)
}

fn deserialize_optional_task_type<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<TaskType>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_wildcard_option(deserializer)
}

fn deserialize_wildcard_option<'de, T, D>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(value)) if value == "*" => Ok(None),
        Some(value) => T::deserialize(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

fn serialize_optional_difficulty<S>(
    value: &Option<Difficulty>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serialize_wildcard_option(value, serializer)
}

fn serialize_optional_task_type<S>(
    value: &Option<TaskType>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serialize_wildcard_option(value, serializer)
}

fn serialize_wildcard_option<T, S>(
    value: &Option<T>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    match value {
        Some(value) => value.serialize(serializer),
        None => serializer.serialize_str("*"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Difficulty, ExecutionRuntimeConfig, ModelRouter, OrangeRuntimeConfig, RoutingRule, TaskType,
    };
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_config_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::current_dir()
            .expect("current dir should exist")
            .join("target")
            .join("model_router_tests")
            .join(format!("{}_{}_{}.json", name, std::process::id(), nonce))
    }

    #[test]
    fn 测试精确匹配优先于通配规则() {
        let router = ModelRouter {
            rules: vec![
                RoutingRule::new(Some(Difficulty::Hard), None, "hard-wildcard"),
                RoutingRule::new(Some(Difficulty::Hard), Some(TaskType::Code), "hard-code"),
                RoutingRule::new(None, Some(TaskType::Code), "code-wildcard"),
                RoutingRule::new(None, None, "full-wildcard"),
            ],
            fallback: "fallback".to_string(),
        };

        assert_eq!(router.route(Difficulty::Hard, TaskType::Code), "hard-code");
        assert_eq!(
            router.route(Difficulty::Hard, TaskType::Chat),
            "hard-wildcard"
        );
        assert_eq!(
            router.route(Difficulty::Easy, TaskType::Write),
            "full-wildcard"
        );
    }

    #[test]
    fn 测试缺失配置文件使用默认值() {
        let config = OrangeRuntimeConfig::load_or_default(&test_config_path("missing"));

        assert_eq!(config.execution.anchor_interval_steps, 5);
        assert_eq!(config.execution.step_budget_initial, 100);
        assert_eq!(config.execution.loop_detection_threshold, 3);
        assert_eq!(
            config
                .model_router()
                .route(Difficulty::Easy, TaskType::Chat),
            "deepseek-chat"
        );
        assert_eq!(
            config
                .model_router()
                .route(Difficulty::Epic, TaskType::Analyze),
            "claude-opus-4-7"
        );
    }

    #[test]
    fn 测试从_json_读取路由和执行配置() {
        let path = test_config_path("custom");
        std::fs::create_dir_all(path.parent().expect("path should have parent"))
            .expect("test dir should be creatable");
        std::fs::write(
            &path,
            r#"{
              "model_routing": {
                "rules": [
                  { "difficulty": "easy", "task_type": "chat", "model": "fast-chat" },
                  { "difficulty": "hard", "task_type": "*", "model": "hard-any" },
                  { "difficulty": "*", "task_type": "analyze", "model": "any-analyze" }
                ],
                "fallback_model": "custom-fallback"
              },
              "execution": {
                "anchor_interval_steps": 7,
                "step_budget_initial": 55,
                "loop_detection_threshold": 4
              }
            }"#,
        )
        .expect("config should be writable");

        let config = OrangeRuntimeConfig::load(&path).expect("custom config should load");

        assert_eq!(
            config.execution,
            ExecutionRuntimeConfig {
                anchor_interval_steps: 7,
                step_budget_initial: 55,
                loop_detection_threshold: 4,
            }
        );
        assert_eq!(
            config
                .model_router()
                .route(Difficulty::Easy, TaskType::Chat),
            "fast-chat"
        );
        assert_eq!(
            config
                .model_router()
                .route(Difficulty::Hard, TaskType::Write),
            "hard-any"
        );
        assert_eq!(
            config
                .model_router()
                .route(Difficulty::Medium, TaskType::Analyze),
            "any-analyze"
        );
        assert_eq!(
            config
                .model_router()
                .route(Difficulty::Medium, TaskType::Chat),
            "custom-fallback"
        );

        std::fs::remove_file(path).expect("test config should be removable");
    }

    #[test]
    fn 测试任务文本推断类型和难度() {
        assert_eq!(
            TaskType::infer("修复 Rust 编译错误，并运行 cargo test"),
            TaskType::Code
        );
        assert_eq!(
            TaskType::infer("请撰写 design doc，说明执行流程"),
            TaskType::Write
        );
        assert_eq!(
            Difficulty::infer("difficulty: epic 实现完整系统"),
            Difficulty::Epic
        );
    }
}
