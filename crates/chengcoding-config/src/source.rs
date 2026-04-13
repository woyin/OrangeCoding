//! # 配置源与分层合并模块
//!
//! 实现多来源的配置加载与合并。支持以下配置源（优先级从低到高）：
//! 1. 默认值 (Default)
//! 2. 配置文件 (File)
//! 3. 环境变量 (Environment)
//! 4. 命令行参数 (CommandLine)
//!
//! 高优先级的配置源会覆盖低优先级的对应值。

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use chengcoding_core::CeairError;

use crate::config::CeairConfig;

// ---------------------------------------------------------------------------
// 配置来源枚举
// ---------------------------------------------------------------------------

/// 配置值的来源类型
///
/// 每种来源具有不同的默认优先级，命令行参数优先级最高。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigSource {
    /// 从配置文件加载（如 config.toml）
    File(PathBuf),

    /// 从环境变量加载（如 ChengCoding_AI_PROVIDER）
    Environment,

    /// 程序内置的默认值
    Default,

    /// 从命令行参数加载
    CommandLine,
}

impl ConfigSource {
    /// 获取配置来源的默认优先级
    ///
    /// 数值越大，优先级越高。
    pub fn default_priority(&self) -> u32 {
        match self {
            // 默认值优先级最低
            ConfigSource::Default => 0,
            // 配置文件优先级次之
            ConfigSource::File(_) => 10,
            // 环境变量优先级较高
            ConfigSource::Environment => 20,
            // 命令行参数优先级最高
            ConfigSource::CommandLine => 30,
        }
    }

    /// 获取配置来源的显示名称
    pub fn display_name(&self) -> String {
        match self {
            ConfigSource::File(path) => format!("文件: {}", path.display()),
            ConfigSource::Environment => "环境变量".to_string(),
            ConfigSource::Default => "默认值".to_string(),
            ConfigSource::CommandLine => "命令行参数".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// 配置层
// ---------------------------------------------------------------------------

/// 单个配置层，代表来自某一来源的完整或部分配置
///
/// 每个层包含一个优先级和一组键值对。
/// 键使用点分隔的路径表示（如 "ai.provider"、"agent.timeout_secs"）。
#[derive(Debug, Clone)]
pub struct ConfigLayer {
    /// 配置来源
    pub source: ConfigSource,

    /// 优先级（数值越大越优先）
    pub priority: u32,

    /// 键值对存储（使用 BTreeMap 保证键的有序性）
    values: BTreeMap<String, toml::Value>,
}

impl ConfigLayer {
    /// 创建新的配置层
    ///
    /// 使用来源的默认优先级。
    pub fn new(source: ConfigSource) -> Self {
        let priority = source.default_priority();
        Self {
            source,
            priority,
            values: BTreeMap::new(),
        }
    }

    /// 创建指定优先级的配置层
    pub fn with_priority(source: ConfigSource, priority: u32) -> Self {
        Self {
            source,
            priority,
            values: BTreeMap::new(),
        }
    }

    /// 从 CeairConfig 构建配置层
    ///
    /// 将结构体扁平化为点分隔的键值对。
    pub fn from_config(
        source: ConfigSource,
        config: &CeairConfig,
    ) -> chengcoding_core::Result<Self> {
        let mut layer = Self::new(source);

        // 将配置序列化为 TOML 值，然后扁平化
        let toml_str = config.to_toml()?;
        let toml_value: toml::Value = toml::from_str(&toml_str)
            .map_err(|e| CeairError::config(format!("配置序列化失败: {e}")))?;

        if let toml::Value::Table(table) = toml_value {
            flatten_toml_table(&table, "", &mut layer.values);
        }

        Ok(layer)
    }

    /// 设置单个配置值
    pub fn set(&mut self, key: &str, value: toml::Value) {
        self.values.insert(key.to_string(), value);
    }

    /// 获取单个配置值
    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        self.values.get(key)
    }

    /// 获取所有键值对的迭代器
    pub fn iter(&self) -> impl Iterator<Item = (&String, &toml::Value)> {
        self.values.iter()
    }

    /// 检查层中是否包含指定键
    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// 获取层中的键值对数量
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// 检查层是否为空
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

// ---------------------------------------------------------------------------
// 分层配置管理器
// ---------------------------------------------------------------------------

/// 分层配置管理器
///
/// 将多个配置层按优先级合并，高优先级的值覆盖低优先级的值。
/// 支持动态添加和移除配置层。
pub struct LayeredConfig {
    /// 所有配置层（按优先级排序）
    layers: Vec<ConfigLayer>,
}

impl LayeredConfig {
    /// 创建空的分层配置管理器
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// 添加一个配置层
    ///
    /// 添加后自动按优先级重新排序。
    pub fn add_layer(&mut self, layer: ConfigLayer) {
        info!(
            "添加配置层: {} (优先级: {})",
            layer.source.display_name(),
            layer.priority
        );
        self.layers.push(layer);
        // 按优先级升序排列，这样后面的（高优先级）覆盖前面的
        self.layers.sort_by_key(|l| l.priority);
    }

    /// 解析指定键的最终值
    ///
    /// 从高优先级到低优先级查找，返回第一个匹配的值。
    pub fn resolve(&self, key: &str) -> Option<&toml::Value> {
        debug!("解析配置键: {}", key);

        // 从高优先级到低优先级遍历
        for layer in self.layers.iter().rev() {
            if let Some(value) = layer.get(key) {
                debug!(
                    "键 '{}' 解析到来源: {} (优先级: {})",
                    key,
                    layer.source.display_name(),
                    layer.priority
                );
                return Some(value);
            }
        }

        debug!("键 '{}' 未在任何配置层中找到", key);
        None
    }

    /// 获取合并后的最终配置
    ///
    /// 从默认配置开始，依次用每个层的值覆盖，
    /// 最终得到一个完整的 `CeairConfig`。
    pub fn get_effective_config(&self) -> chengcoding_core::Result<CeairConfig> {
        info!("计算有效配置（共 {} 个配置层）", self.layers.len());

        // 收集所有扁平化的键值对，按优先级合并
        let mut merged: BTreeMap<String, toml::Value> = BTreeMap::new();

        for layer in &self.layers {
            for (key, value) in layer.iter() {
                merged.insert(key.clone(), value.clone());
            }
        }

        // 将扁平化的键值对重建为嵌套的 TOML 结构
        let nested = unflatten_to_toml(&merged);

        // 序列化为 TOML 字符串，再反序列化为配置结构体
        let toml_str = toml::to_string_pretty(&nested)
            .map_err(|e| CeairError::serialization(format!("合并配置序列化失败: {e}")))?;

        CeairConfig::from_toml(&toml_str)
    }

    /// 获取当前层的数量
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// 获取所有配置层的只读引用
    pub fn layers(&self) -> &[ConfigLayer] {
        &self.layers
    }
}

impl Default for LayeredConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 将嵌套的 TOML 表扁平化为点分隔的键值对
///
/// 例如：`[ai] provider = "openai"` 变为 `"ai.provider" = "openai"`
fn flatten_toml_table(
    table: &toml::map::Map<String, toml::Value>,
    prefix: &str,
    output: &mut BTreeMap<String, toml::Value>,
) {
    for (key, value) in table {
        // 构造完整的键路径
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };

        match value {
            toml::Value::Table(sub_table) => {
                // 递归处理子表
                flatten_toml_table(sub_table, &full_key, output);
            }
            _ => {
                // 叶节点，直接存储
                output.insert(full_key, value.clone());
            }
        }
    }
}

/// 将扁平化的点分隔键值对重建为嵌套的 TOML 表
///
/// 例如：`"ai.provider" = "openai"` 变为 `[ai] provider = "openai"`
fn unflatten_to_toml(flat: &BTreeMap<String, toml::Value>) -> toml::Value {
    let mut root = toml::map::Map::new();

    for (key, value) in flat {
        let parts: Vec<&str> = key.split('.').collect();
        insert_nested(&mut root, &parts, value.clone());
    }

    toml::Value::Table(root)
}

/// 递归地将值插入到嵌套的 TOML 表中
fn insert_nested(
    table: &mut toml::map::Map<String, toml::Value>,
    parts: &[&str],
    value: toml::Value,
) {
    match parts.len() {
        0 => {}
        1 => {
            // 最后一级，直接插入值
            table.insert(parts[0].to_string(), value);
        }
        _ => {
            // 中间级别，确保子表存在并递归
            let sub_table = table
                .entry(parts[0].to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

            if let toml::Value::Table(ref mut sub) = sub_table {
                insert_nested(sub, &parts[1..], value);
            }
        }
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试配置来源的默认优先级
    #[test]
    fn test_source_priority() {
        let default_src = ConfigSource::Default;
        let file_src = ConfigSource::File(PathBuf::from("/config.toml"));
        let env_src = ConfigSource::Environment;
        let cli_src = ConfigSource::CommandLine;

        // 验证优先级递增关系
        assert!(default_src.default_priority() < file_src.default_priority());
        assert!(file_src.default_priority() < env_src.default_priority());
        assert!(env_src.default_priority() < cli_src.default_priority());
    }

    /// 测试配置来源的显示名称
    #[test]
    fn test_source_display_name() {
        assert_eq!(ConfigSource::Default.display_name(), "默认值");
        assert_eq!(ConfigSource::Environment.display_name(), "环境变量");
        assert_eq!(ConfigSource::CommandLine.display_name(), "命令行参数");

        let file_src = ConfigSource::File(PathBuf::from("/etc/ceair/config.toml"));
        assert!(file_src.display_name().contains("config.toml"));
    }

    /// 测试单个配置层的基本操作
    #[test]
    fn test_config_layer_basic_ops() {
        let mut layer = ConfigLayer::new(ConfigSource::Default);

        // 初始应为空
        assert!(layer.is_empty());
        assert_eq!(layer.len(), 0);

        // 设置值
        layer.set("ai.provider", toml::Value::String("openai".to_string()));
        layer.set("agent.max_iterations", toml::Value::Integer(100));

        // 验证值
        assert_eq!(layer.len(), 2);
        assert!(!layer.is_empty());
        assert!(layer.contains_key("ai.provider"));
        assert_eq!(
            layer.get("ai.provider").unwrap().as_str().unwrap(),
            "openai"
        );
        assert_eq!(
            layer
                .get("agent.max_iterations")
                .unwrap()
                .as_integer()
                .unwrap(),
            100
        );

        // 不存在的键
        assert!(!layer.contains_key("nonexistent"));
        assert!(layer.get("nonexistent").is_none());
    }

    /// 测试分层配置的优先级覆盖
    #[test]
    fn test_layered_config_priority_override() {
        let mut layered = LayeredConfig::new();

        // 默认层（最低优先级）
        let mut default_layer = ConfigLayer::new(ConfigSource::Default);
        default_layer.set("ai.provider", toml::Value::String("openai".to_string()));
        default_layer.set("ai.model", toml::Value::String("gpt-3.5".to_string()));

        // 文件层（中等优先级）
        let mut file_layer = ConfigLayer::new(ConfigSource::File(PathBuf::from("config.toml")));
        file_layer.set("ai.model", toml::Value::String("gpt-4".to_string()));

        // 环境变量层（高优先级）
        let mut env_layer = ConfigLayer::new(ConfigSource::Environment);
        env_layer.set("ai.model", toml::Value::String("gpt-4-turbo".to_string()));

        // 添加层
        layered.add_layer(default_layer);
        layered.add_layer(file_layer);
        layered.add_layer(env_layer);

        // 验证层数
        assert_eq!(layered.layer_count(), 3);

        // ai.provider 仅在默认层设置，应返回默认值
        let provider = layered.resolve("ai.provider").unwrap();
        assert_eq!(provider.as_str().unwrap(), "openai");

        // ai.model 在三个层都设置，应返回最高优先级的值（环境变量）
        let model = layered.resolve("ai.model").unwrap();
        assert_eq!(model.as_str().unwrap(), "gpt-4-turbo");

        // 不存在的键
        assert!(layered.resolve("nonexistent.key").is_none());
    }

    /// 测试 TOML 扁平化与重建
    #[test]
    fn test_flatten_and_unflatten() {
        let toml_str = r#"
[ai]
provider = "openai"
model = "gpt-4"
temperature = 0.7

[agent]
max_iterations = 50
"#;
        let toml_value: toml::Value = toml::from_str(toml_str).unwrap();
        let table = toml_value.as_table().unwrap();

        // 扁平化
        let mut flat = BTreeMap::new();
        flatten_toml_table(table, "", &mut flat);

        // 验证扁平化结果
        assert_eq!(flat.get("ai.provider").unwrap().as_str().unwrap(), "openai");
        assert_eq!(flat.get("ai.model").unwrap().as_str().unwrap(), "gpt-4");
        assert!(
            (flat.get("ai.temperature").unwrap().as_float().unwrap() - 0.7).abs() < f64::EPSILON
        );
        assert_eq!(
            flat.get("agent.max_iterations")
                .unwrap()
                .as_integer()
                .unwrap(),
            50
        );

        // 重建
        let rebuilt = unflatten_to_toml(&flat);
        let rebuilt_table = rebuilt.as_table().unwrap();

        // 验证重建结果的结构
        assert!(rebuilt_table.contains_key("ai"));
        assert!(rebuilt_table.contains_key("agent"));

        let ai = rebuilt_table["ai"].as_table().unwrap();
        assert_eq!(ai["provider"].as_str().unwrap(), "openai");
    }

    /// 测试从配置结构体构建配置层
    #[test]
    fn test_layer_from_config() {
        let config = CeairConfig::default();
        let layer =
            ConfigLayer::from_config(ConfigSource::Default, &config).expect("从配置构建层失败");

        // 验证默认配置的值已被扁平化
        assert_eq!(
            layer.get("ai.provider").unwrap().as_str().unwrap(),
            "openai"
        );
        assert_eq!(
            layer
                .get("agent.max_iterations")
                .unwrap()
                .as_integer()
                .unwrap(),
            50
        );
        assert_eq!(layer.get("tui.theme").unwrap().as_str().unwrap(), "dark");
    }

    /// 测试获取有效配置（完整合并流程）
    #[test]
    fn test_get_effective_config() {
        let mut layered = LayeredConfig::new();

        // 从默认配置构建基础层
        let default_config = CeairConfig::default();
        let default_layer = ConfigLayer::from_config(ConfigSource::Default, &default_config)
            .expect("构建默认层失败");
        layered.add_layer(default_layer);

        // 添加覆盖层
        let mut override_layer =
            ConfigLayer::new(ConfigSource::File(PathBuf::from("override.toml")));
        override_layer.set("ai.provider", toml::Value::String("anthropic".to_string()));
        override_layer.set("ai.model", toml::Value::String("claude-3".to_string()));
        override_layer.set("agent.max_iterations", toml::Value::Integer(100));
        layered.add_layer(override_layer);

        // 获取合并后的有效配置
        let effective = layered.get_effective_config().expect("获取有效配置失败");

        // 验证被覆盖的值
        assert_eq!(effective.ai.provider, "anthropic");
        assert_eq!(effective.ai.model, "claude-3");
        assert_eq!(effective.agent.max_iterations, 100);

        // 验证未被覆盖的值保持默认
        assert_eq!(effective.tui.theme, "dark");
        assert_eq!(effective.logging.level, "info");
        assert!((effective.ai.temperature - 0.7).abs() < f64::EPSILON);
    }

    /// 测试自定义优先级
    #[test]
    fn test_custom_priority() {
        let mut layered = LayeredConfig::new();

        // 使用自定义优先级覆盖默认优先级
        let mut low_layer = ConfigLayer::with_priority(ConfigSource::Default, 5);
        low_layer.set("ai.provider", toml::Value::String("low".to_string()));

        let mut high_layer = ConfigLayer::with_priority(ConfigSource::Default, 50);
        high_layer.set("ai.provider", toml::Value::String("high".to_string()));

        // 先添加高优先级，再添加低优先级
        layered.add_layer(high_layer);
        layered.add_layer(low_layer);

        // 无论添加顺序，高优先级应胜出
        let resolved = layered.resolve("ai.provider").unwrap();
        assert_eq!(resolved.as_str().unwrap(), "high");
    }

    /// 测试空的分层配置
    #[test]
    fn test_empty_layered_config() {
        let layered = LayeredConfig::new();

        assert_eq!(layered.layer_count(), 0);
        assert!(layered.resolve("any.key").is_none());
    }
}
