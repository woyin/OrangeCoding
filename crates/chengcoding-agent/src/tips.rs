//! # 上下文感知 Tips 系统
//!
//! 根据当前对话上下文生成有用的提示建议。
//!
//! # 设计思想
//! 参考 reference 中的 Tips 系统：
//! - 根据工具使用模式、错误频率、上下文状态生成提示
//! - 提示是被动触发的（不主动打断用户）
//! - 提示内容来自可扩展的规则集
//! - 每个提示有优先级和去重机制

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// 上下文状态
// ---------------------------------------------------------------------------

/// 对话上下文快照 — Tips 引擎的输入
///
/// 收集决策所需的关键指标，不携带完整对话历史
#[derive(Clone, Debug, Default)]
pub struct ContextSnapshot {
    /// 当前轮次
    pub turn_index: usize,
    /// 累计工具调用次数
    pub total_tool_calls: usize,
    /// 累计错误次数
    pub total_errors: usize,
    /// 最近 N 轮使用的工具名称
    pub recent_tools: Vec<String>,
    /// 当前工作目录
    pub working_dir: Option<String>,
    /// 是否在 git 仓库中
    pub in_git_repo: bool,
    /// 累计 token 使用量
    pub tokens_used: usize,
    /// 上下文窗口大小
    pub context_window: usize,
}

// ---------------------------------------------------------------------------
// Tip 定义
// ---------------------------------------------------------------------------

/// 上下文提示
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    /// 提示 ID（用于去重）
    pub id: &'static str,
    /// 提示类别
    pub category: TipCategory,
    /// 提示内容
    pub message: String,
    /// 优先级（越小越重要）
    pub priority: i32,
}

/// 提示类别
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TipCategory {
    /// 效率优化建议
    Efficiency,
    /// 安全相关提醒
    Safety,
    /// 功能发现
    Discovery,
    /// 上下文管理
    Context,
}

impl TipCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            TipCategory::Efficiency => "效率",
            TipCategory::Safety => "安全",
            TipCategory::Discovery => "发现",
            TipCategory::Context => "上下文",
        }
    }
}

// ---------------------------------------------------------------------------
// Tips 引擎
// ---------------------------------------------------------------------------

/// Tips 引擎 — 管理规则评估和去重
pub struct TipsEngine {
    /// 已展示过的 tip ID 集合（去重用）
    shown: HashSet<&'static str>,
    /// 是否启用
    enabled: bool,
}

impl TipsEngine {
    pub fn new() -> Self {
        Self {
            shown: HashSet::new(),
            enabled: true,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 评估当前上下文，返回最相关的一个 Tip（如果有）
    ///
    /// 返回优先级最高（数值最小）的、未展示过的 tip
    pub fn evaluate(&mut self, ctx: &ContextSnapshot) -> Option<Tip> {
        if !self.enabled {
            return None;
        }

        let mut tips = self.generate_tips(ctx);

        // 过滤已展示的
        tips.retain(|t| !self.shown.contains(t.id));

        // 按优先级排序
        tips.sort_by_key(|t| t.priority);

        // 返回第一个并标记已展示
        if let Some(tip) = tips.into_iter().next() {
            self.shown.insert(tip.id);
            Some(tip)
        } else {
            None
        }
    }

    /// 重置已展示记录
    pub fn reset(&mut self) {
        self.shown.clear();
    }

    /// 获取已展示的 tip 数量
    pub fn shown_count(&self) -> usize {
        self.shown.len()
    }

    /// 根据上下文状态生成候选 tips
    ///
    /// 每条规则独立检查，返回所有命中的 tip
    fn generate_tips(&self, ctx: &ContextSnapshot) -> Vec<Tip> {
        let mut tips = Vec::new();

        // 规则 1: 高错误率提示
        if ctx.total_errors > 3 && ctx.total_tool_calls > 0 {
            let error_rate = ctx.total_errors as f64 / ctx.total_tool_calls as f64;
            if error_rate > 0.3 {
                tips.push(Tip {
                    id: "high_error_rate",
                    category: TipCategory::Efficiency,
                    message: format!(
                        "错误率较高 ({:.0}%)，建议检查工具参数或切换策略。",
                        error_rate * 100.0
                    ),
                    priority: 10,
                });
            }
        }

        // 规则 2: 重复工具使用提示
        if ctx.recent_tools.len() >= 5 {
            let last_5 = &ctx.recent_tools[ctx.recent_tools.len() - 5..];
            let unique: HashSet<&String> = last_5.iter().collect();
            if unique.len() == 1 {
                tips.push(Tip {
                    id: "repetitive_tool_use",
                    category: TipCategory::Efficiency,
                    message: format!("连续 5 次使用 '{}'，考虑是否有更高效的方法。", last_5[0]),
                    priority: 20,
                });
            }
        }

        // 规则 3: 上下文接近满载
        if ctx.context_window > 0 {
            let usage_ratio = ctx.tokens_used as f64 / ctx.context_window as f64;
            if usage_ratio > 0.8 {
                tips.push(Tip {
                    id: "context_near_full",
                    category: TipCategory::Context,
                    message: format!(
                        "上下文使用 {:.0}%，接近上限。考虑压缩或开始新会话。",
                        usage_ratio * 100.0
                    ),
                    priority: 5,
                });
            }
        }

        // 规则 4: 长对话没有使用版本控制
        if ctx.turn_index > 10 && ctx.in_git_repo {
            let uses_git = ctx
                .recent_tools
                .iter()
                .any(|t| t.contains("git") || t == "bash");
            if !uses_git {
                tips.push(Tip {
                    id: "suggest_git_commit",
                    category: TipCategory::Safety,
                    message: "已进行多轮修改，建议执行 git commit 保存进度。".to_string(),
                    priority: 15,
                });
            }
        }

        // 规则 5: 首次使用提示
        if ctx.turn_index == 0 {
            tips.push(Tip {
                id: "welcome_tip",
                category: TipCategory::Discovery,
                message: "提示：可以使用 /help 查看可用命令。".to_string(),
                priority: 50,
            });
        }

        tips
    }
}

impl Default for TipsEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建默认上下文
    fn default_ctx() -> ContextSnapshot {
        ContextSnapshot {
            context_window: 100000,
            ..Default::default()
        }
    }

    // -----------------------------------------------------------------------
    // TipCategory 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_category_display_name() {
        assert_eq!(TipCategory::Efficiency.display_name(), "效率");
        assert_eq!(TipCategory::Safety.display_name(), "安全");
        assert_eq!(TipCategory::Discovery.display_name(), "发现");
        assert_eq!(TipCategory::Context.display_name(), "上下文");
    }

    // -----------------------------------------------------------------------
    // 规则测试
    // -----------------------------------------------------------------------

    /// 测试首轮提示
    #[test]
    fn test_welcome_tip() {
        let mut engine = TipsEngine::new();
        let ctx = default_ctx();
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_some());
        assert_eq!(tip.unwrap().id, "welcome_tip");
    }

    /// 测试高错误率提示
    #[test]
    fn test_high_error_rate() {
        let mut engine = TipsEngine::new();
        let ctx = ContextSnapshot {
            turn_index: 5,
            total_tool_calls: 10,
            total_errors: 5,
            context_window: 100000,
            ..Default::default()
        };
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_some());
        let tip = tip.unwrap();
        assert_eq!(tip.id, "high_error_rate");
        assert_eq!(tip.category, TipCategory::Efficiency);
    }

    /// 测试低错误率不触发
    #[test]
    fn test_low_error_rate_no_tip() {
        let mut engine = TipsEngine::new();
        let ctx = ContextSnapshot {
            turn_index: 5,
            total_tool_calls: 10,
            total_errors: 1,
            context_window: 100000,
            ..Default::default()
        };
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_none());
    }

    /// 测试重复工具使用提示
    #[test]
    fn test_repetitive_tool_use() {
        let mut engine = TipsEngine::new();
        let ctx = ContextSnapshot {
            turn_index: 5,
            recent_tools: vec![
                "read_file".into(),
                "read_file".into(),
                "read_file".into(),
                "read_file".into(),
                "read_file".into(),
            ],
            context_window: 100000,
            ..Default::default()
        };
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_some());
        assert_eq!(tip.unwrap().id, "repetitive_tool_use");
    }

    /// 测试不同工具不触发重复提示
    #[test]
    fn test_varied_tools_no_repetitive_tip() {
        let mut engine = TipsEngine::new();
        let ctx = ContextSnapshot {
            turn_index: 5,
            recent_tools: vec![
                "read_file".into(),
                "bash".into(),
                "read_file".into(),
                "grep".into(),
                "read_file".into(),
            ],
            context_window: 100000,
            ..Default::default()
        };
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_none());
    }

    /// 测试上下文接近满载提示
    #[test]
    fn test_context_near_full() {
        let mut engine = TipsEngine::new();
        let ctx = ContextSnapshot {
            turn_index: 5,
            tokens_used: 85000,
            context_window: 100000,
            ..Default::default()
        };
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_some());
        assert_eq!(tip.unwrap().id, "context_near_full");
    }

    /// 测试上下文使用率低不触发
    #[test]
    fn test_context_low_usage_no_tip() {
        let mut engine = TipsEngine::new();
        let ctx = ContextSnapshot {
            turn_index: 5,
            tokens_used: 10000,
            context_window: 100000,
            ..Default::default()
        };
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_none());
    }

    /// 测试 git commit 建议
    #[test]
    fn test_suggest_git_commit() {
        let mut engine = TipsEngine::new();
        let ctx = ContextSnapshot {
            turn_index: 15,
            in_git_repo: true,
            recent_tools: vec!["edit_file".into(), "read_file".into()],
            context_window: 100000,
            ..Default::default()
        };
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_some());
        assert_eq!(tip.unwrap().id, "suggest_git_commit");
    }

    // -----------------------------------------------------------------------
    // 去重测试
    // -----------------------------------------------------------------------

    /// 测试 tip 去重
    #[test]
    fn test_deduplication() {
        let mut engine = TipsEngine::new();
        let ctx = default_ctx(); // turn_index=0 触发 welcome_tip

        let tip1 = engine.evaluate(&ctx);
        assert!(tip1.is_some());
        assert_eq!(tip1.unwrap().id, "welcome_tip");

        // 同上下文再次评估不应重复返回
        let tip2 = engine.evaluate(&ctx);
        assert!(tip2.is_none());
    }

    /// 测试重置去重记录
    #[test]
    fn test_reset() {
        let mut engine = TipsEngine::new();
        let ctx = default_ctx();

        engine.evaluate(&ctx);
        assert_eq!(engine.shown_count(), 1);

        engine.reset();
        assert_eq!(engine.shown_count(), 0);

        // 重置后应可再次获取
        let tip = engine.evaluate(&ctx);
        assert!(tip.is_some());
    }

    // -----------------------------------------------------------------------
    // 引擎控制测试
    // -----------------------------------------------------------------------

    /// 测试禁用引擎
    #[test]
    fn test_disabled_engine() {
        let mut engine = TipsEngine::new();
        engine.set_enabled(false);

        let ctx = default_ctx();
        assert!(engine.evaluate(&ctx).is_none());
    }

    /// 测试优先级排序（context_near_full priority=5 < high_error_rate priority=10）
    #[test]
    fn test_priority_order() {
        let mut engine = TipsEngine::new();
        let ctx = ContextSnapshot {
            turn_index: 5,
            total_tool_calls: 10,
            total_errors: 5,
            tokens_used: 90000,
            context_window: 100000,
            ..Default::default()
        };

        let tip = engine.evaluate(&ctx);
        assert!(tip.is_some());
        // context_near_full (priority=5) 应先于 high_error_rate (priority=10)
        assert_eq!(tip.unwrap().id, "context_near_full");
    }
}
