//! # 工具使用摘要生成器
//!
//! 跟踪和汇总工具调用情况，为验证 Agent 和上下文管理提供数据。
//!
//! # 设计思想
//! 参考 reference 中工具调用的统计跟踪：
//! - 记录每次工具调用的名称、耗时、成功/失败
//! - 生成结构化摘要供 Verification Agent 检查
//! - 支持按工具名称聚合统计

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 工具调用记录
// ---------------------------------------------------------------------------

/// 单次工具调用的记录
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// 工具名称
    pub tool_name: String,
    /// 调用轮次
    pub turn_index: usize,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 是否成功
    pub success: bool,
    /// 错误信息（仅失败时有值）
    pub error: Option<String>,
}

/// 工具聚合统计
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolStats {
    /// 总调用次数
    pub call_count: usize,
    /// 成功次数
    pub success_count: usize,
    /// 失败次数
    pub failure_count: usize,
    /// 总耗时（毫秒）
    pub total_duration_ms: u64,
    /// 最近的错误信息
    pub last_error: Option<String>,
}

impl ToolStats {
    /// 成功率
    pub fn success_rate(&self) -> f64 {
        if self.call_count == 0 {
            return 1.0;
        }
        self.success_count as f64 / self.call_count as f64
    }

    /// 平均耗时（毫秒）
    pub fn avg_duration_ms(&self) -> u64 {
        if self.call_count == 0 {
            return 0;
        }
        self.total_duration_ms / self.call_count as u64
    }
}

// ---------------------------------------------------------------------------
// 摘要生成器
// ---------------------------------------------------------------------------

/// 工具使用摘要跟踪器
///
/// 收集所有工具调用记录，并提供聚合分析能力。
pub struct ToolUsageSummary {
    /// 调用记录列表
    records: Vec<ToolCallRecord>,
    /// 按工具名称聚合的统计
    stats_cache: HashMap<String, ToolStats>,
    /// 缓存是否过期
    cache_dirty: bool,
}

impl ToolUsageSummary {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            stats_cache: HashMap::new(),
            cache_dirty: false,
        }
    }

    /// 记录一次工具调用
    pub fn record(
        &mut self,
        tool_name: impl Into<String>,
        turn_index: usize,
        duration: Duration,
        result: Result<(), String>,
    ) {
        let success = result.is_ok();
        let error = result.err();

        self.records.push(ToolCallRecord {
            tool_name: tool_name.into(),
            turn_index,
            duration_ms: duration.as_millis() as u64,
            success,
            error,
        });
        self.cache_dirty = true;
    }

    /// 获取总调用次数
    pub fn total_calls(&self) -> usize {
        self.records.len()
    }

    /// 获取总失败次数
    pub fn total_failures(&self) -> usize {
        self.records.iter().filter(|r| !r.success).count()
    }

    /// 获取总成功次数
    pub fn total_successes(&self) -> usize {
        self.records.iter().filter(|r| r.success).count()
    }

    /// 获取全局成功率
    pub fn overall_success_rate(&self) -> f64 {
        if self.records.is_empty() {
            return 1.0;
        }
        self.total_successes() as f64 / self.records.len() as f64
    }

    /// 获取按工具名称聚合的统计
    pub fn stats_by_tool(&mut self) -> &HashMap<String, ToolStats> {
        if self.cache_dirty {
            self.rebuild_cache();
        }
        &self.stats_cache
    }

    /// 获取指定工具的统计
    pub fn stats_for(&mut self, tool_name: &str) -> Option<ToolStats> {
        if self.cache_dirty {
            self.rebuild_cache();
        }
        self.stats_cache.get(tool_name).cloned()
    }

    /// 获取使用频率最高的 N 个工具
    pub fn top_tools(&mut self, n: usize) -> Vec<(String, usize)> {
        if self.cache_dirty {
            self.rebuild_cache();
        }
        let mut tools: Vec<(String, usize)> = self
            .stats_cache
            .iter()
            .map(|(name, stats)| (name.clone(), stats.call_count))
            .collect();
        tools.sort_by(|a, b| b.1.cmp(&a.1));
        tools.truncate(n);
        tools
    }

    /// 获取失败率最高的工具
    pub fn most_failing_tools(&mut self) -> Vec<(String, f64)> {
        if self.cache_dirty {
            self.rebuild_cache();
        }
        let mut tools: Vec<(String, f64)> = self
            .stats_cache
            .iter()
            .filter(|(_, stats)| stats.failure_count > 0)
            .map(|(name, stats)| {
                let fail_rate = stats.failure_count as f64 / stats.call_count as f64;
                (name.clone(), fail_rate)
            })
            .collect();
        tools.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        tools
    }

    /// 生成文本摘要
    ///
    /// 格式化的摘要报告，适合注入到 Verification Agent 的上下文中
    pub fn generate_summary(&mut self) -> String {
        if self.cache_dirty {
            self.rebuild_cache();
        }

        let mut lines = Vec::new();
        lines.push(format!("## 工具使用摘要"));
        lines.push(format!(
            "- 总调用: {} 次 (成功 {}, 失败 {})",
            self.total_calls(),
            self.total_successes(),
            self.total_failures()
        ));
        lines.push(format!(
            "- 成功率: {:.1}%",
            self.overall_success_rate() * 100.0
        ));

        let top = self.top_tools(5);
        if !top.is_empty() {
            lines.push(format!("\n### 使用频率 Top 5"));
            for (name, count) in &top {
                let stats = self.stats_cache.get(name).unwrap();
                lines.push(format!(
                    "- {}: {} 次, 成功率 {:.0}%, 平均耗时 {}ms",
                    name,
                    count,
                    stats.success_rate() * 100.0,
                    stats.avg_duration_ms()
                ));
            }
        }

        let failing = self.most_failing_tools();
        if !failing.is_empty() {
            lines.push(format!("\n### 失败工具"));
            for (name, rate) in &failing {
                let stats = self.stats_cache.get(name).unwrap();
                lines.push(format!(
                    "- {}: 失败率 {:.0}% ({}/{})",
                    name,
                    rate * 100.0,
                    stats.failure_count,
                    stats.call_count
                ));
                if let Some(err) = &stats.last_error {
                    lines.push(format!("  最近错误: {}", err));
                }
            }
        }

        lines.join("\n")
    }

    /// 清除所有记录
    pub fn clear(&mut self) {
        self.records.clear();
        self.stats_cache.clear();
        self.cache_dirty = false;
    }

    /// 重建聚合缓存
    fn rebuild_cache(&mut self) {
        self.stats_cache.clear();
        for record in &self.records {
            let stats = self
                .stats_cache
                .entry(record.tool_name.clone())
                .or_default();
            stats.call_count += 1;
            stats.total_duration_ms += record.duration_ms;
            if record.success {
                stats.success_count += 1;
            } else {
                stats.failure_count += 1;
                stats.last_error = record.error.clone();
            }
        }
        self.cache_dirty = false;
    }
}

impl Default for ToolUsageSummary {
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

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    // -----------------------------------------------------------------------
    // ToolStats 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_tool_stats_success_rate() {
        let stats = ToolStats {
            call_count: 10,
            success_count: 8,
            failure_count: 2,
            ..Default::default()
        };
        assert!((stats.success_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_tool_stats_empty() {
        let stats = ToolStats::default();
        assert_eq!(stats.success_rate(), 1.0);
        assert_eq!(stats.avg_duration_ms(), 0);
    }

    #[test]
    fn test_tool_stats_avg_duration() {
        let stats = ToolStats {
            call_count: 4,
            total_duration_ms: 400,
            ..Default::default()
        };
        assert_eq!(stats.avg_duration_ms(), 100);
    }

    // -----------------------------------------------------------------------
    // ToolUsageSummary 测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_summary() {
        let mut summary = ToolUsageSummary::new();
        assert_eq!(summary.total_calls(), 0);
        assert_eq!(summary.total_failures(), 0);
        assert_eq!(summary.overall_success_rate(), 1.0);
    }

    #[test]
    fn test_record_success() {
        let mut summary = ToolUsageSummary::new();
        summary.record("read_file", 0, ms(50), Ok(()));

        assert_eq!(summary.total_calls(), 1);
        assert_eq!(summary.total_successes(), 1);
        assert_eq!(summary.total_failures(), 0);
    }

    #[test]
    fn test_record_failure() {
        let mut summary = ToolUsageSummary::new();
        summary.record("bash", 0, ms(100), Err("命令失败".into()));

        assert_eq!(summary.total_calls(), 1);
        assert_eq!(summary.total_successes(), 0);
        assert_eq!(summary.total_failures(), 1);
    }

    #[test]
    fn test_mixed_records() {
        let mut summary = ToolUsageSummary::new();
        summary.record("read_file", 0, ms(10), Ok(()));
        summary.record("read_file", 1, ms(20), Ok(()));
        summary.record("bash", 2, ms(100), Err("error".into()));
        summary.record("read_file", 3, ms(15), Ok(()));

        assert_eq!(summary.total_calls(), 4);
        assert_eq!(summary.total_successes(), 3);
        assert_eq!(summary.total_failures(), 1);
        assert!((summary.overall_success_rate() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_stats_by_tool() {
        let mut summary = ToolUsageSummary::new();
        summary.record("read_file", 0, ms(10), Ok(()));
        summary.record("read_file", 1, ms(20), Ok(()));
        summary.record("bash", 2, ms(100), Err("error".into()));

        let stats = summary.stats_by_tool();
        assert_eq!(stats.len(), 2);

        let rf = stats.get("read_file").unwrap();
        assert_eq!(rf.call_count, 2);
        assert_eq!(rf.success_count, 2);
        assert_eq!(rf.avg_duration_ms(), 15);

        let bash = stats.get("bash").unwrap();
        assert_eq!(bash.call_count, 1);
        assert_eq!(bash.failure_count, 1);
    }

    #[test]
    fn test_stats_for_specific_tool() {
        let mut summary = ToolUsageSummary::new();
        summary.record("grep", 0, ms(30), Ok(()));

        let stats = summary.stats_for("grep");
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().call_count, 1);

        let none = summary.stats_for("nonexistent");
        assert!(none.is_none());
    }

    #[test]
    fn test_top_tools() {
        let mut summary = ToolUsageSummary::new();
        for _ in 0..5 {
            summary.record("read_file", 0, ms(10), Ok(()));
        }
        for _ in 0..3 {
            summary.record("bash", 0, ms(50), Ok(()));
        }
        summary.record("grep", 0, ms(20), Ok(()));

        let top = summary.top_tools(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "read_file");
        assert_eq!(top[0].1, 5);
        assert_eq!(top[1].0, "bash");
        assert_eq!(top[1].1, 3);
    }

    #[test]
    fn test_most_failing_tools() {
        let mut summary = ToolUsageSummary::new();
        summary.record("read_file", 0, ms(10), Ok(()));
        summary.record("bash", 0, ms(50), Err("err1".into()));
        summary.record("bash", 1, ms(50), Ok(()));
        summary.record("grep", 0, ms(20), Err("err2".into()));

        let failing = summary.most_failing_tools();
        assert_eq!(failing.len(), 2);
        // grep: 100% 失败率应排第一
        assert_eq!(failing[0].0, "grep");
        assert!((failing[0].1 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_generate_summary() {
        let mut summary = ToolUsageSummary::new();
        summary.record("read_file", 0, ms(10), Ok(()));
        summary.record("bash", 0, ms(100), Err("命令失败".into()));

        let text = summary.generate_summary();
        assert!(text.contains("工具使用摘要"));
        assert!(text.contains("总调用: 2 次"));
        assert!(text.contains("成功 1"));
        assert!(text.contains("失败 1"));
    }

    #[test]
    fn test_clear() {
        let mut summary = ToolUsageSummary::new();
        summary.record("read_file", 0, ms(10), Ok(()));
        assert_eq!(summary.total_calls(), 1);

        summary.clear();
        assert_eq!(summary.total_calls(), 0);
    }

    #[test]
    fn test_empty_generate_summary() {
        let mut summary = ToolUsageSummary::new();
        let text = summary.generate_summary();
        assert!(text.contains("总调用: 0 次"));
        assert!(text.contains("成功率: 100.0%"));
    }
}
