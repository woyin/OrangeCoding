//! # KAIROS — Post-Sampling Hook 系统
//!
//! 在 LLM 采样（推理）完成后、工具执行之前提供拦截点。
//!
//! # 设计思想
//! 参考 reference 中的 KAIROS（post-sampling hooks）：
//! - 每次 LLM 返回响应后触发一组钩子
//! - 钩子可以观察响应内容、修改行为、注入提示
//! - 多个钩子按优先级顺序执行，支持短路
//! - 与工具执行管道解耦，独立于 Agent 循环

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// 采样响应
// ---------------------------------------------------------------------------

/// LLM 采样响应的简化表示
///
/// 包含钩子决策所需的关键信息，不携带完整 API 响应体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SamplingResponse {
    /// 响应文本内容
    pub text: String,
    /// 是否包含工具调用
    pub has_tool_calls: bool,
    /// 工具调用数量
    pub tool_call_count: usize,
    /// 消耗的 token 数
    pub tokens_used: usize,
    /// 当前对话轮次
    pub turn_index: usize,
}

// ---------------------------------------------------------------------------
// 钩子决策
// ---------------------------------------------------------------------------

/// Post-Sampling 钩子的决策结果
///
/// 钩子返回此枚举告知系统如何处理 LLM 响应
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostSamplingDecision {
    /// 不干预，继续正常流程
    Pass,

    /// 注入系统提示（在下一轮对话中追加）
    ///
    /// 常见场景：KAIROS 提示、安全警告、行为引导
    InjectPrompt(String),

    /// 中止当前操作（如安全拦截）
    Abort(String),
}

impl PostSamplingDecision {
    /// 判断是否为干预型决策
    pub fn is_intervention(&self) -> bool {
        !matches!(self, PostSamplingDecision::Pass)
    }
}

impl fmt::Display for PostSamplingDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PostSamplingDecision::Pass => write!(f, "pass"),
            PostSamplingDecision::InjectPrompt(msg) => write!(f, "inject: {}", msg),
            PostSamplingDecision::Abort(reason) => write!(f, "abort: {}", reason),
        }
    }
}

// ---------------------------------------------------------------------------
// Post-Sampling Hook Trait
// ---------------------------------------------------------------------------

/// Post-Sampling 钩子 trait
///
/// 实现此 trait 来创建自定义的采样后钩子。
/// 钩子按 priority() 从小到大执行，第一个非 Pass 的决策会生效。
pub trait PostSamplingHook: Send + Sync {
    /// 钩子名称（用于日志和调试）
    fn name(&self) -> &str;

    /// 优先级（越小越先执行）
    fn priority(&self) -> i32 {
        100
    }

    /// 是否启用
    fn is_enabled(&self) -> bool {
        true
    }

    /// 评估 LLM 响应，返回决策
    fn evaluate(&self, response: &SamplingResponse) -> PostSamplingDecision;
}

// ---------------------------------------------------------------------------
// Hook 管道
// ---------------------------------------------------------------------------

/// Post-Sampling 钩子管道
///
/// 管理多个钩子的注册和按优先级执行。
/// 设计为可多次执行（每轮 LLM 响应后调用一次）。
pub struct PostSamplingPipeline {
    hooks: Vec<Box<dyn PostSamplingHook>>,
    /// 是否已按优先级排序
    sorted: bool,
}

impl PostSamplingPipeline {
    /// 创建空管道
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            sorted: true,
        }
    }

    /// 注册钩子
    pub fn register(&mut self, hook: Box<dyn PostSamplingHook>) {
        self.hooks.push(hook);
        self.sorted = false;
    }

    /// 执行所有钩子，返回第一个非 Pass 的决策
    ///
    /// 如果所有钩子都返回 Pass，则返回 Pass。
    /// 钩子按优先级从小到大执行。
    pub fn evaluate(&mut self, response: &SamplingResponse) -> PostSamplingDecision {
        if !self.sorted {
            self.hooks.sort_by_key(|h| h.priority());
            self.sorted = true;
        }

        for hook in &self.hooks {
            if !hook.is_enabled() {
                continue;
            }
            let decision = hook.evaluate(response);
            if decision.is_intervention() {
                return decision;
            }
        }

        PostSamplingDecision::Pass
    }

    /// 获取已注册的钩子数量
    pub fn hook_count(&self) -> usize {
        self.hooks.len()
    }

    /// 获取已启用的钩子数量
    pub fn enabled_count(&self) -> usize {
        self.hooks.iter().filter(|h| h.is_enabled()).count()
    }
}

impl Default for PostSamplingPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 内置钩子示例：Token 使用警告
// ---------------------------------------------------------------------------

/// Token 使用量警告钩子
///
/// 当单轮 token 使用量超过阈值时注入提示，
/// 引导 LLM 精简输出。
pub struct TokenUsageWarningHook {
    /// 触发警告的 token 阈值
    threshold: usize,
    /// 是否启用
    enabled: bool,
}

impl TokenUsageWarningHook {
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            enabled: true,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl PostSamplingHook for TokenUsageWarningHook {
    fn name(&self) -> &str {
        "token_usage_warning"
    }

    fn priority(&self) -> i32 {
        50
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn evaluate(&self, response: &SamplingResponse) -> PostSamplingDecision {
        if response.tokens_used > self.threshold {
            PostSamplingDecision::InjectPrompt(format!(
                "注意：上一轮使用了 {} tokens（阈值 {}），请精简输出。",
                response.tokens_used, self.threshold
            ))
        } else {
            PostSamplingDecision::Pass
        }
    }
}

/// 工具调用数量限制钩子
///
/// 当单轮工具调用数量过多时发出警告
pub struct ToolCallLimitHook {
    /// 单轮最大工具调用数
    max_calls: usize,
}

impl ToolCallLimitHook {
    pub fn new(max_calls: usize) -> Self {
        Self { max_calls }
    }
}

impl PostSamplingHook for ToolCallLimitHook {
    fn name(&self) -> &str {
        "tool_call_limit"
    }

    fn priority(&self) -> i32 {
        40
    }

    fn evaluate(&self, response: &SamplingResponse) -> PostSamplingDecision {
        if response.tool_call_count > self.max_calls {
            PostSamplingDecision::InjectPrompt(format!(
                "注意：单轮请求了 {} 个工具调用（限制 {}），请减少并行调用。",
                response.tool_call_count, self.max_calls
            ))
        } else {
            PostSamplingDecision::Pass
        }
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的 SamplingResponse
    fn sample_response(text: &str, tokens: usize, tool_calls: usize) -> SamplingResponse {
        SamplingResponse {
            text: text.to_string(),
            has_tool_calls: tool_calls > 0,
            tool_call_count: tool_calls,
            tokens_used: tokens,
            turn_index: 0,
        }
    }

    // -----------------------------------------------------------------------
    // PostSamplingDecision 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_decision_is_intervention() {
        assert!(!PostSamplingDecision::Pass.is_intervention());
        assert!(PostSamplingDecision::InjectPrompt("test".into()).is_intervention());
        assert!(PostSamplingDecision::Abort("reason".into()).is_intervention());
    }

    #[test]
    fn test_decision_display() {
        assert_eq!(format!("{}", PostSamplingDecision::Pass), "pass");
        assert!(format!("{}", PostSamplingDecision::InjectPrompt("hi".into())).contains("inject"));
        assert!(format!("{}", PostSamplingDecision::Abort("err".into())).contains("abort"));
    }

    // -----------------------------------------------------------------------
    // TokenUsageWarningHook 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_token_warning_below_threshold() {
        let hook = TokenUsageWarningHook::new(1000);
        let resp = sample_response("ok", 500, 0);
        assert_eq!(hook.evaluate(&resp), PostSamplingDecision::Pass);
    }

    #[test]
    fn test_token_warning_above_threshold() {
        let hook = TokenUsageWarningHook::new(1000);
        let resp = sample_response("long output", 1500, 0);
        match hook.evaluate(&resp) {
            PostSamplingDecision::InjectPrompt(msg) => {
                assert!(msg.contains("1500"));
                assert!(msg.contains("1000"));
            }
            other => panic!("期望 InjectPrompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_token_warning_disabled() {
        let mut hook = TokenUsageWarningHook::new(1000);
        hook.set_enabled(false);
        assert!(!hook.is_enabled());
    }

    // -----------------------------------------------------------------------
    // ToolCallLimitHook 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_tool_limit_within() {
        let hook = ToolCallLimitHook::new(5);
        let resp = sample_response("ok", 100, 3);
        assert_eq!(hook.evaluate(&resp), PostSamplingDecision::Pass);
    }

    #[test]
    fn test_tool_limit_exceeded() {
        let hook = ToolCallLimitHook::new(5);
        let resp = sample_response("ok", 100, 8);
        match hook.evaluate(&resp) {
            PostSamplingDecision::InjectPrompt(msg) => {
                assert!(msg.contains("8"));
                assert!(msg.contains("5"));
            }
            other => panic!("期望 InjectPrompt，得到 {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // PostSamplingPipeline 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_pipeline_returns_pass() {
        let mut pipeline = PostSamplingPipeline::new();
        let resp = sample_response("ok", 100, 0);
        assert_eq!(pipeline.evaluate(&resp), PostSamplingDecision::Pass);
    }

    #[test]
    fn test_pipeline_priority_order() {
        // 注册两个钩子：工具限制(priority=40) 和 token 警告(priority=50)
        // 两个都触发时，优先级高（数值小）的先执行
        let mut pipeline = PostSamplingPipeline::new();
        pipeline.register(Box::new(TokenUsageWarningHook::new(100)));
        pipeline.register(Box::new(ToolCallLimitHook::new(2)));

        let resp = sample_response("ok", 500, 5);
        match pipeline.evaluate(&resp) {
            PostSamplingDecision::InjectPrompt(msg) => {
                // ToolCallLimitHook (priority=40) 应先触发
                assert!(msg.contains("工具调用"), "应先触发工具限制钩子: {}", msg);
            }
            other => panic!("期望 InjectPrompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_pipeline_skips_disabled() {
        let mut pipeline = PostSamplingPipeline::new();
        let mut hook = TokenUsageWarningHook::new(100);
        hook.set_enabled(false);
        pipeline.register(Box::new(hook));

        let resp = sample_response("ok", 500, 0);
        assert_eq!(pipeline.evaluate(&resp), PostSamplingDecision::Pass);
    }

    #[test]
    fn test_pipeline_all_pass() {
        let mut pipeline = PostSamplingPipeline::new();
        pipeline.register(Box::new(TokenUsageWarningHook::new(10000)));
        pipeline.register(Box::new(ToolCallLimitHook::new(100)));

        let resp = sample_response("ok", 100, 1);
        assert_eq!(pipeline.evaluate(&resp), PostSamplingDecision::Pass);
    }

    #[test]
    fn test_pipeline_hook_count() {
        let mut pipeline = PostSamplingPipeline::new();
        assert_eq!(pipeline.hook_count(), 0);

        pipeline.register(Box::new(TokenUsageWarningHook::new(1000)));
        assert_eq!(pipeline.hook_count(), 1);

        pipeline.register(Box::new(ToolCallLimitHook::new(5)));
        assert_eq!(pipeline.hook_count(), 2);
    }

    #[test]
    fn test_pipeline_enabled_count() {
        let mut pipeline = PostSamplingPipeline::new();
        let mut hook = TokenUsageWarningHook::new(1000);
        hook.set_enabled(false);

        pipeline.register(Box::new(hook));
        pipeline.register(Box::new(ToolCallLimitHook::new(5)));

        assert_eq!(pipeline.hook_count(), 2);
        assert_eq!(pipeline.enabled_count(), 1);
    }

    /// 自定义钩子测试
    struct AlwaysAbortHook;

    impl PostSamplingHook for AlwaysAbortHook {
        fn name(&self) -> &str {
            "always_abort"
        }
        fn priority(&self) -> i32 {
            0
        }
        fn evaluate(&self, _response: &SamplingResponse) -> PostSamplingDecision {
            PostSamplingDecision::Abort("测试中止".into())
        }
    }

    #[test]
    fn test_custom_abort_hook() {
        let mut pipeline = PostSamplingPipeline::new();
        pipeline.register(Box::new(AlwaysAbortHook));
        pipeline.register(Box::new(TokenUsageWarningHook::new(100)));

        let resp = sample_response("ok", 500, 0);
        match pipeline.evaluate(&resp) {
            PostSamplingDecision::Abort(reason) => {
                assert_eq!(reason, "测试中止");
            }
            other => panic!("期望 Abort，得到 {:?}", other),
        }
    }
}
