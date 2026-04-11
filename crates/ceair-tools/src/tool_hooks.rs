//! # 工具执行后钩子系统
//!
//! 在工具执行完成后，按顺序调用注册的钩子链，支持：
//! - 观察执行结果（日志、统计等）
//! - 修改输出内容（格式转换、敏感信息脱敏等）
//! - 阻塞错误（安全审计、结果校验等）
//!
//! # 设计思想
//! 参考 reference 中 post-tool hooks 的设计：
//! - 钩子链按注册顺序执行，每个钩子看到的是上一个钩子处理后的结果
//! - Continue 表示不干预，ModifyOutput 修改结果，BlockingError 将成功转为错误
//! - 钩子是可扩展的，新的审计/监控逻辑可以通过添加钩子实现

use std::time::Duration;

// ---------------------------------------------------------------------------
// 钩子上下文
// ---------------------------------------------------------------------------

/// 工具执行后的钩子上下文
///
/// 提供工具执行的完整信息，钩子据此决定后续动作
#[derive(Clone, Debug)]
pub struct HookContext {
    /// 工具名称
    pub tool_name: String,
    /// 工具输入参数的摘要
    pub input_summary: String,
    /// 工具输出结果
    pub output: String,
    /// 执行耗时
    pub duration: Duration,
    /// 是否为错误结果
    pub is_error: bool,
}

// ---------------------------------------------------------------------------
// 钩子结果
// ---------------------------------------------------------------------------

/// 钩子执行结果
///
/// 决定工具执行流程的后续行为：
/// - Continue: 不干预，继续后续钩子
/// - ModifyOutput: 替换工具输出内容
/// - BlockingError: 将成功转为错误，阻止结果传递给 Agent
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HookResult {
    /// 不干预结果
    Continue,
    /// 修改输出内容
    ModifyOutput(String),
    /// 阻塞错误（即使工具执行成功也转为失败）
    BlockingError(String),
}

// ---------------------------------------------------------------------------
// 钩子 trait
// ---------------------------------------------------------------------------

/// 工具执行后钩子
///
/// 实现此 trait 可以在工具执行完成后介入执行流程。
pub trait PostToolHook: Send + Sync {
    /// 工具执行完成后调用
    fn on_tool_complete(&self, ctx: &HookContext) -> HookResult;
}

// ---------------------------------------------------------------------------
// 钩子管道
// ---------------------------------------------------------------------------

/// 工具执行后钩子管道
///
/// 按注册顺序执行所有钩子。第一个返回非 Continue 的结果生效。
pub struct PostToolPipeline {
    hooks: Vec<Box<dyn PostToolHook>>,
}

impl PostToolPipeline {
    /// 创建空管道
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// 注册钩子
    pub fn add_hook(&mut self, hook: impl PostToolHook + 'static) {
        self.hooks.push(Box::new(hook));
    }

    /// 执行钩子管道
    ///
    /// 按注册顺序依次调用钩子：
    /// - Continue: 继续执行下一个钩子
    /// - ModifyOutput: 更新 output 后继续执行后续钩子（链式修改）
    /// - BlockingError: 立即返回，不执行后续钩子
    pub fn run(&self, ctx: &mut HookContext) -> HookResult {
        let mut last_result = HookResult::Continue;

        for hook in &self.hooks {
            let result = hook.on_tool_complete(ctx);
            match &result {
                HookResult::Continue => {
                    // 不干预，继续
                }
                HookResult::ModifyOutput(new_output) => {
                    // 更新输出，后续钩子看到的是修改后的结果
                    ctx.output = new_output.clone();
                    last_result = result;
                }
                HookResult::BlockingError(_) => {
                    // 立即终止管道
                    return result;
                }
            }
        }

        last_result
    }

    /// 钩子数量
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// 管道是否为空
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

impl Default for PostToolPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 内置钩子
// ---------------------------------------------------------------------------

/// 执行耗时警告钩子
///
/// 当工具执行时间超过阈值时，在输出中附加警告信息
pub struct SlowToolWarningHook {
    threshold: Duration,
}

impl SlowToolWarningHook {
    pub fn new(threshold: Duration) -> Self {
        Self { threshold }
    }
}

impl PostToolHook for SlowToolWarningHook {
    fn on_tool_complete(&self, ctx: &HookContext) -> HookResult {
        if ctx.duration >= self.threshold {
            let warning = format!(
                "{}\n\n⚠️ 工具 '{}' 执行耗时 {:.1}s（阈值 {:.1}s）",
                ctx.output,
                ctx.tool_name,
                ctx.duration.as_secs_f64(),
                self.threshold.as_secs_f64(),
            );
            HookResult::ModifyOutput(warning)
        } else {
            HookResult::Continue
        }
    }
}

/// 输出长度截断钩子
///
/// 当工具输出超过指定字符数时进行截断
pub struct OutputTruncationHook {
    max_chars: usize,
}

impl OutputTruncationHook {
    pub fn new(max_chars: usize) -> Self {
        Self { max_chars }
    }
}

impl PostToolHook for OutputTruncationHook {
    fn on_tool_complete(&self, ctx: &HookContext) -> HookResult {
        if ctx.output.len() > self.max_chars {
            let truncated = format!(
                "{}...\n[输出已截断: {} -> {} 字符]",
                &ctx.output[..self.max_chars],
                ctx.output.len(),
                self.max_chars
            );
            HookResult::ModifyOutput(truncated)
        } else {
            HookResult::Continue
        }
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(tool: &str, output: &str, duration_ms: u64) -> HookContext {
        HookContext {
            tool_name: tool.to_string(),
            input_summary: "{}".to_string(),
            output: output.to_string(),
            duration: Duration::from_millis(duration_ms),
            is_error: false,
        }
    }

    // --- 无钩子时行为测试 ---

    #[test]
    fn test_empty_pipeline_returns_continue() {
        let pipeline = PostToolPipeline::new();
        let mut ctx = make_ctx("test", "hello", 100);
        let result = pipeline.run(&mut ctx);
        assert_eq!(result, HookResult::Continue);
        assert_eq!(ctx.output, "hello");
    }

    #[test]
    fn test_pipeline_is_empty() {
        let pipeline = PostToolPipeline::new();
        assert!(pipeline.is_empty());
        assert_eq!(pipeline.len(), 0);
    }

    // --- Continue 钩子测试 ---

    struct NoopHook;
    impl PostToolHook for NoopHook {
        fn on_tool_complete(&self, _ctx: &HookContext) -> HookResult {
            HookResult::Continue
        }
    }

    #[test]
    fn test_continue_hook_does_not_modify() {
        let mut pipeline = PostToolPipeline::new();
        pipeline.add_hook(NoopHook);
        let mut ctx = make_ctx("test", "hello", 100);
        let result = pipeline.run(&mut ctx);
        assert_eq!(result, HookResult::Continue);
        assert_eq!(ctx.output, "hello");
    }

    // --- ModifyOutput 测试 ---

    struct UppercaseHook;
    impl PostToolHook for UppercaseHook {
        fn on_tool_complete(&self, ctx: &HookContext) -> HookResult {
            HookResult::ModifyOutput(ctx.output.to_uppercase())
        }
    }

    #[test]
    fn test_modify_output_changes_result() {
        let mut pipeline = PostToolPipeline::new();
        pipeline.add_hook(UppercaseHook);
        let mut ctx = make_ctx("test", "hello", 100);
        let result = pipeline.run(&mut ctx);
        assert_eq!(result, HookResult::ModifyOutput("HELLO".into()));
        assert_eq!(ctx.output, "HELLO");
    }

    // --- BlockingError 测试 ---

    struct BlockerHook;
    impl PostToolHook for BlockerHook {
        fn on_tool_complete(&self, _ctx: &HookContext) -> HookResult {
            HookResult::BlockingError("审计不通过".into())
        }
    }

    #[test]
    fn test_blocking_error_stops_pipeline() {
        let mut pipeline = PostToolPipeline::new();
        pipeline.add_hook(BlockerHook);
        let mut ctx = make_ctx("test", "success", 100);
        let result = pipeline.run(&mut ctx);
        assert_eq!(
            result,
            HookResult::BlockingError("审计不通过".into())
        );
    }

    // --- 多钩子顺序执行测试 ---

    struct PrefixHook(String);
    impl PostToolHook for PrefixHook {
        fn on_tool_complete(&self, ctx: &HookContext) -> HookResult {
            HookResult::ModifyOutput(format!("{}{}", self.0, ctx.output))
        }
    }

    #[test]
    fn test_multiple_hooks_chain() {
        let mut pipeline = PostToolPipeline::new();
        pipeline.add_hook(PrefixHook("[A]".into()));
        pipeline.add_hook(PrefixHook("[B]".into()));
        let mut ctx = make_ctx("test", "data", 100);
        pipeline.run(&mut ctx);
        // 第一个钩子: "[A]data" → 第二个钩子: "[B][A]data"
        assert_eq!(ctx.output, "[B][A]data");
    }

    #[test]
    fn test_blocking_error_skips_remaining() {
        let mut pipeline = PostToolPipeline::new();
        pipeline.add_hook(PrefixHook("[1]".into()));
        pipeline.add_hook(BlockerHook);
        pipeline.add_hook(PrefixHook("[3]".into())); // 不应执行
        let mut ctx = make_ctx("test", "data", 100);
        let result = pipeline.run(&mut ctx);
        assert!(matches!(result, HookResult::BlockingError(_)));
        // 第一个钩子修改了 output, 然后 blocker 阻止了后续
        assert_eq!(ctx.output, "[1]data");
    }

    // --- SlowToolWarningHook 测试 ---

    #[test]
    fn test_slow_tool_warning_below_threshold() {
        let hook = SlowToolWarningHook::new(Duration::from_secs(5));
        let ctx = make_ctx("read_file", "contents", 1000);
        let result = hook.on_tool_complete(&ctx);
        assert_eq!(result, HookResult::Continue);
    }

    #[test]
    fn test_slow_tool_warning_above_threshold() {
        let hook = SlowToolWarningHook::new(Duration::from_secs(5));
        let ctx = make_ctx("bash", "output", 6000);
        let result = hook.on_tool_complete(&ctx);
        match result {
            HookResult::ModifyOutput(s) => {
                assert!(s.contains("output"));
                assert!(s.contains("⚠️"));
                assert!(s.contains("bash"));
            }
            other => panic!("Expected ModifyOutput, got {:?}", other),
        }
    }

    // --- OutputTruncationHook 测试 ---

    #[test]
    fn test_truncation_below_limit() {
        let hook = OutputTruncationHook::new(100);
        let ctx = make_ctx("test", "short", 100);
        let result = hook.on_tool_complete(&ctx);
        assert_eq!(result, HookResult::Continue);
    }

    #[test]
    fn test_truncation_above_limit() {
        let hook = OutputTruncationHook::new(10);
        let ctx = make_ctx("test", "this is a very long output string", 100);
        let result = hook.on_tool_complete(&ctx);
        match result {
            HookResult::ModifyOutput(s) => {
                assert!(s.contains("this is a "));
                assert!(s.contains("[输出已截断"));
            }
            other => panic!("Expected ModifyOutput, got {:?}", other),
        }
    }

    #[test]
    fn test_hook_context_is_error() {
        let mut ctx = make_ctx("test", "error msg", 100);
        ctx.is_error = true;
        assert!(ctx.is_error);
    }

    #[test]
    fn test_pipeline_len() {
        let mut pipeline = PostToolPipeline::new();
        pipeline.add_hook(NoopHook);
        pipeline.add_hook(NoopHook);
        assert_eq!(pipeline.len(), 2);
        assert!(!pipeline.is_empty());
    }
}
