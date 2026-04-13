//! # 模型回退链模块
//!
//! 当主模型不可用时（API 错误、超时、限流等），自动切换到备用模型。
//! 每个 Agent 可配置一条回退链，系统按顺序尝试直到成功。
//!
//! ## 错误码触发
//!
//! | HTTP 状态码 | 含义 | 是否触发回退 |
//! |-------------|------|-------------|
//! | 429 | 限流 | 是 |
//! | 503 | 服务不可用 | 是 |
//! | 529 | 过载 | 是 |
//! | 401 | 认证失败 | 是（配置错误） |
//! | 5xx | 服务器错误 | 部分触发 |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ============================================================
// FallbackEntry — 回退链中的单个条目
// ============================================================

/// 回退链中的一个模型条目。
///
/// 可以是简单的模型字符串，也可以携带额外的配置（variant、thinking 等）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackEntry {
    /// 模型标识符（如 "openai/gpt-5.4"）
    pub model: String,

    /// 模型变体（如 "high", "medium"）
    #[serde(default)]
    pub variant: Option<String>,

    /// 思考链预算 token 数
    #[serde(default)]
    pub thinking_budget: Option<u32>,
}

impl FallbackEntry {
    /// 创建简单的模型回退条目
    pub fn simple(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            variant: None,
            thinking_budget: None,
        }
    }

    /// 创建带变体的模型回退条目
    pub fn with_variant(model: impl Into<String>, variant: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            variant: Some(variant.into()),
            thinking_budget: None,
        }
    }
}

// ============================================================
// FallbackChain — 回退链
// ============================================================

/// 模型回退链——按顺序尝试备用模型直到成功。
///
/// 支持冷却期管理：模型触发回退后在冷却期内不会被再次尝试。
#[derive(Debug, Clone)]
pub struct FallbackChain {
    /// 主模型标识符
    primary_model: String,
    /// 有序的回退模型列表
    entries: Vec<FallbackEntry>,
}

impl FallbackChain {
    /// 创建新的回退链
    ///
    /// `primary` 是主模型，`entries` 是按优先级排序的备用模型列表。
    pub fn new(primary: impl Into<String>, entries: Vec<FallbackEntry>) -> Self {
        Self {
            primary_model: primary.into(),
            entries,
        }
    }

    /// 创建无回退的链（仅主模型）
    pub fn no_fallback(primary: impl Into<String>) -> Self {
        Self {
            primary_model: primary.into(),
            entries: vec![],
        }
    }

    /// 返回主模型标识符
    pub fn primary(&self) -> &str {
        &self.primary_model
    }

    /// 返回回退条目数量（不含主模型）
    pub fn fallback_count(&self) -> usize {
        self.entries.len()
    }

    /// 返回所有回退条目的引用
    pub fn entries(&self) -> &[FallbackEntry] {
        &self.entries
    }

    /// 是否有可用的回退模型
    pub fn has_fallbacks(&self) -> bool {
        !self.entries.is_empty()
    }
}

// ============================================================
// RetryableError — 可重试错误判断
// ============================================================

/// 判断 HTTP 状态码是否表示可重试的错误（应触发模型回退）
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 503 | 529 | 500 | 502 | 504)
}

/// 判断错误是否是配置错误（如 API Key 缺失），需要切换提供商
pub fn is_config_error_status(status: u16) -> bool {
    matches!(status, 401 | 403)
}

/// 判断是否应该触发模型回退
pub fn should_fallback(status: u16) -> bool {
    is_retryable_status(status) || is_config_error_status(status)
}

// ============================================================
// CooldownManager — 冷却期管理
// ============================================================

/// 模型冷却期管理器。
///
/// 当某个模型触发回退后，在冷却期内不再尝试该模型，
/// 避免对已知不可用的模型进行无效请求。
pub struct CooldownManager {
    /// 模型标识 → 冷却结束时间
    cooldowns: HashMap<String, Instant>,
    /// 默认冷却时长
    default_duration: Duration,
}

impl CooldownManager {
    /// 创建新的冷却期管理器
    pub fn new(default_cooldown: Duration) -> Self {
        Self {
            cooldowns: HashMap::new(),
            default_duration: default_cooldown,
        }
    }

    /// 将模型标记为冷却中
    pub fn mark_cooling(&mut self, model: &str) {
        let until = Instant::now() + self.default_duration;
        self.cooldowns.insert(model.to_string(), until);
    }

    /// 将模型标记为冷却中（自定义时长）
    pub fn mark_cooling_for(&mut self, model: &str, duration: Duration) {
        let until = Instant::now() + duration;
        self.cooldowns.insert(model.to_string(), until);
    }

    /// 检查模型是否在冷却中
    pub fn is_cooling(&self, model: &str) -> bool {
        if let Some(until) = self.cooldowns.get(model) {
            Instant::now() < *until
        } else {
            false
        }
    }

    /// 清除指定模型的冷却状态
    pub fn clear_cooldown(&mut self, model: &str) {
        self.cooldowns.remove(model);
    }

    /// 清除所有冷却状态
    pub fn clear_all(&mut self) {
        self.cooldowns.clear();
    }

    /// 返回当前冷却中的模型数量
    pub fn cooling_count(&self) -> usize {
        let now = Instant::now();
        self.cooldowns
            .values()
            .filter(|until| now < **until)
            .count()
    }
}

impl Default for CooldownManager {
    fn default() -> Self {
        // 默认冷却 60 秒
        Self::new(Duration::from_secs(60))
    }
}

// ============================================================
// FallbackResolver — 回退解析器
// ============================================================

/// 回退解析器——根据回退链和冷却状态选择可用模型。
///
/// 按以下优先级选择模型：
/// 1. 主模型（如果未冷却）
/// 2. 回退链中第一个未冷却的模型
/// 3. 如果全部冷却，返回主模型（强制使用）
pub struct FallbackResolver {
    /// 冷却期管理器
    cooldown: CooldownManager,
}

impl FallbackResolver {
    /// 创建新的回退解析器
    pub fn new(cooldown: CooldownManager) -> Self {
        Self { cooldown }
    }

    /// 解析当前应使用的模型
    ///
    /// 返回 (模型标识符, 变体, 是否为回退模型)
    pub fn resolve(&self, chain: &FallbackChain) -> ResolvedModel {
        // 优先使用主模型
        if !self.cooldown.is_cooling(chain.primary()) {
            return ResolvedModel {
                model: chain.primary().to_string(),
                variant: None,
                is_fallback: false,
                fallback_index: None,
            };
        }

        // 尝试回退链中的模型
        for (idx, entry) in chain.entries().iter().enumerate() {
            if !self.cooldown.is_cooling(&entry.model) {
                return ResolvedModel {
                    model: entry.model.clone(),
                    variant: entry.variant.clone(),
                    is_fallback: true,
                    fallback_index: Some(idx),
                };
            }
        }

        // 全部冷却——强制使用主模型
        ResolvedModel {
            model: chain.primary().to_string(),
            variant: None,
            is_fallback: false,
            fallback_index: None,
        }
    }

    /// 标记模型回退（触发冷却）
    pub fn mark_failed(&mut self, model: &str) {
        self.cooldown.mark_cooling(model);
    }

    /// 标记模型恢复（清除冷却）
    pub fn mark_recovered(&mut self, model: &str) {
        self.cooldown.clear_cooldown(model);
    }
}

impl Default for FallbackResolver {
    fn default() -> Self {
        Self::new(CooldownManager::default())
    }
}

/// 解析后的模型选择结果
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    /// 选定的模型标识符
    pub model: String,
    /// 模型变体
    pub variant: Option<String>,
    /// 是否为回退模型（非主模型）
    pub is_fallback: bool,
    /// 在回退链中的索引（如果是回退模型）
    pub fallback_index: Option<usize>,
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试简单回退条目创建
    #[test]
    fn test_fallback_entry_simple() {
        let entry = FallbackEntry::simple("openai/gpt-5.4");
        assert_eq!(entry.model, "openai/gpt-5.4");
        assert!(entry.variant.is_none());
    }

    /// 测试带变体的回退条目
    #[test]
    fn test_fallback_entry_with_variant() {
        let entry = FallbackEntry::with_variant("openai/gpt-5.4", "high");
        assert_eq!(entry.model, "openai/gpt-5.4");
        assert_eq!(entry.variant.as_deref(), Some("high"));
    }

    /// 测试创建回退链
    #[test]
    fn test_fallback_chain_creation() {
        let chain = FallbackChain::new(
            "anthropic/claude-opus-4-6",
            vec![
                FallbackEntry::simple("openai/gpt-5.4"),
                FallbackEntry::with_variant("google/gemini-3.1-pro", "high"),
            ],
        );
        assert_eq!(chain.primary(), "anthropic/claude-opus-4-6");
        assert_eq!(chain.fallback_count(), 2);
        assert!(chain.has_fallbacks());
    }

    /// 测试无回退链
    #[test]
    fn test_no_fallback_chain() {
        let chain = FallbackChain::no_fallback("openai/gpt-5.4");
        assert_eq!(chain.primary(), "openai/gpt-5.4");
        assert_eq!(chain.fallback_count(), 0);
        assert!(!chain.has_fallbacks());
    }

    /// 测试可重试状态码判断
    #[test]
    fn test_retryable_status() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(503));
        assert!(is_retryable_status(529));
        assert!(is_retryable_status(500));
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(404));
    }

    /// 测试配置错误状态码
    #[test]
    fn test_config_error_status() {
        assert!(is_config_error_status(401));
        assert!(is_config_error_status(403));
        assert!(!is_config_error_status(429));
        assert!(!is_config_error_status(200));
    }

    /// 测试 should_fallback 综合判断
    #[test]
    fn test_should_fallback() {
        assert!(should_fallback(429));
        assert!(should_fallback(401));
        assert!(should_fallback(503));
        assert!(!should_fallback(200));
        assert!(!should_fallback(400));
    }

    /// 测试冷却期管理
    #[test]
    fn test_cooldown_manager() {
        let mut mgr = CooldownManager::new(Duration::from_secs(60));

        assert!(!mgr.is_cooling("model-a"));

        mgr.mark_cooling("model-a");
        assert!(mgr.is_cooling("model-a"));
        assert!(!mgr.is_cooling("model-b"));

        mgr.clear_cooldown("model-a");
        assert!(!mgr.is_cooling("model-a"));
    }

    /// 测试冷却计数
    #[test]
    fn test_cooldown_count() {
        let mut mgr = CooldownManager::new(Duration::from_secs(60));
        assert_eq!(mgr.cooling_count(), 0);

        mgr.mark_cooling("model-a");
        mgr.mark_cooling("model-b");
        assert_eq!(mgr.cooling_count(), 2);

        mgr.clear_all();
        assert_eq!(mgr.cooling_count(), 0);
    }

    /// 测试冷却期过期
    #[test]
    fn test_cooldown_expiry() {
        let mut mgr = CooldownManager::new(Duration::from_millis(1));
        mgr.mark_cooling("model-a");
        // 等待冷却期过期
        std::thread::sleep(Duration::from_millis(10));
        assert!(!mgr.is_cooling("model-a"));
    }

    /// 测试回退解析器——主模型优先
    #[test]
    fn test_resolver_primary_first() {
        let resolver = FallbackResolver::default();
        let chain = FallbackChain::new("primary-model", vec![FallbackEntry::simple("fallback-1")]);

        let resolved = resolver.resolve(&chain);
        assert_eq!(resolved.model, "primary-model");
        assert!(!resolved.is_fallback);
        assert!(resolved.fallback_index.is_none());
    }

    /// 测试回退解析器——主模型冷却后使用回退
    #[test]
    fn test_resolver_fallback_on_primary_cooling() {
        let mut resolver = FallbackResolver::default();
        let chain = FallbackChain::new(
            "primary-model",
            vec![
                FallbackEntry::simple("fallback-1"),
                FallbackEntry::with_variant("fallback-2", "high"),
            ],
        );

        // 主模型冷却
        resolver.mark_failed("primary-model");

        let resolved = resolver.resolve(&chain);
        assert_eq!(resolved.model, "fallback-1");
        assert!(resolved.is_fallback);
        assert_eq!(resolved.fallback_index, Some(0));
    }

    /// 测试回退解析器——跳过冷却中的回退模型
    #[test]
    fn test_resolver_skip_cooling_fallback() {
        let mut resolver = FallbackResolver::default();
        let chain = FallbackChain::new(
            "primary",
            vec![
                FallbackEntry::simple("fallback-1"),
                FallbackEntry::with_variant("fallback-2", "high"),
            ],
        );

        // 主模型和第一个回退都冷却
        resolver.mark_failed("primary");
        resolver.mark_failed("fallback-1");

        let resolved = resolver.resolve(&chain);
        assert_eq!(resolved.model, "fallback-2");
        assert_eq!(resolved.variant.as_deref(), Some("high"));
        assert_eq!(resolved.fallback_index, Some(1));
    }

    /// 测试回退解析器——全部冷却时强制使用主模型
    #[test]
    fn test_resolver_force_primary_when_all_cooling() {
        let mut resolver = FallbackResolver::default();
        let chain = FallbackChain::new("primary", vec![FallbackEntry::simple("fallback-1")]);

        resolver.mark_failed("primary");
        resolver.mark_failed("fallback-1");

        let resolved = resolver.resolve(&chain);
        assert_eq!(resolved.model, "primary");
        assert!(!resolved.is_fallback); // 强制使用主模型
    }

    /// 测试模型恢复
    #[test]
    fn test_resolver_mark_recovered() {
        let mut resolver = FallbackResolver::default();
        resolver.mark_failed("model-a");
        assert!(resolver.cooldown.is_cooling("model-a"));

        resolver.mark_recovered("model-a");
        assert!(!resolver.cooldown.is_cooling("model-a"));
    }

    /// 测试无回退链的解析
    #[test]
    fn test_resolver_no_fallback_chain() {
        let resolver = FallbackResolver::default();
        let chain = FallbackChain::no_fallback("only-model");

        let resolved = resolver.resolve(&chain);
        assert_eq!(resolved.model, "only-model");
        assert!(!resolved.is_fallback);
    }

    /// 测试自定义冷却时长
    #[test]
    fn test_custom_cooldown_duration() {
        let mut mgr = CooldownManager::new(Duration::from_secs(3600));
        mgr.mark_cooling_for("model-a", Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        // 自定义时长已过期
        assert!(!mgr.is_cooling("model-a"));
    }
}
