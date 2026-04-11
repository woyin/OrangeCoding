//! Self-Evolving: 自进化模块
//!
//! 分析违规报告、回滚日志中的模式，生成优化策略，追踪系统进化。

use crate::rules::InvariantCategory;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 失败模式 — 从历史数据中识别的重复失败
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// 模式 ID
    pub id: String,
    /// 关联的不变量类别
    pub category: InvariantCategory,
    /// 模式描述
    pub description: String,
    /// 出现频率（次数）
    pub frequency: usize,
    /// 首次出现时间
    pub first_seen: DateTime<Utc>,
    /// 最近出现时间
    pub last_seen: DateTime<Utc>,
    /// 相关的规则 ID 列表
    pub related_rules: Vec<String>,
}

/// 优化策略类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyType {
    /// 改进 prompt/指令
    PromptImprovement,
    /// 新增不变量规则
    NewInvariant,
    /// 调整路由/调度策略
    RoutingAdjust,
    /// 工具链优化
    ToolchainOptimize,
    /// 架构改进
    ArchitectureChange,
}

/// 优化策略的状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyStatus {
    /// 待评估
    Proposed,
    /// 已采纳
    Accepted,
    /// 执行中
    Executing,
    /// 已完成
    Completed,
    /// 已回滚（无效）
    Reverted,
    /// 已拒绝
    Rejected,
}

/// 单条优化策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionStrategy {
    /// 策略 ID
    pub id: String,
    /// 策略类型
    pub strategy_type: StrategyType,
    /// 描述
    pub description: String,
    /// 关联的失败模式 ID
    pub pattern_id: String,
    /// 预期改善
    pub expected_improvement: String,
    /// 当前状态
    pub status: StrategyStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 进化数据快照 — 某一时刻的系统状态指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionSnapshot {
    /// 快照时间
    pub timestamp: DateTime<Utc>,
    /// 总违规数
    pub total_violations: usize,
    /// 各类别违规数
    pub violations_by_category: HashMap<String, usize>,
    /// 总回滚数
    pub total_rollbacks: usize,
    /// 成功修复数
    pub healed_count: usize,
    /// 活跃策略数
    pub active_strategies: usize,
}

/// 快照对比结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionDelta {
    /// 违规变化（负数=改善）
    pub violation_delta: i64,
    /// 回滚变化
    pub rollback_delta: i64,
    /// 修复变化（正数=改善）
    pub healed_delta: i64,
    /// 是否总体改善
    pub improved: bool,
}

/// 自进化引擎
pub struct EvolutionEngine {
    /// 识别的失败模式
    patterns: Vec<FailurePattern>,
    /// 优化策略
    strategies: Vec<EvolutionStrategy>,
    /// 历史快照
    snapshots: Vec<EvolutionSnapshot>,
    /// 自增 ID 计数器
    next_pattern_id: usize,
    next_strategy_id: usize,
}

impl EvolutionEngine {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            strategies: Vec::new(),
            snapshots: Vec::new(),
            next_pattern_id: 1,
            next_strategy_id: 1,
        }
    }

    /// 从违规报告中学习失败模式
    pub fn learn_from_violations(&mut self, report: &crate::report::ViolationReport) {
        for violation in &report.violations {
            let now = violation.timestamp;
            // 查找已有模式（按 rule_id 匹配）
            if let Some(pattern) = self
                .patterns
                .iter_mut()
                .find(|p| p.related_rules.contains(&violation.rule_id))
            {
                pattern.frequency += 1;
                pattern.last_seen = now;
            } else {
                // 从 rule_id 前缀推导类别
                let category = category_from_rule_id(&violation.rule_id);
                let pattern = FailurePattern {
                    id: format!("PAT-{:04}", self.next_pattern_id),
                    category,
                    description: violation.message.clone(),
                    frequency: 1,
                    first_seen: now,
                    last_seen: now,
                    related_rules: vec![violation.rule_id.clone()],
                };
                self.next_pattern_id += 1;
                self.patterns.push(pattern);
            }
        }
    }

    /// 从回滚日志中学习失败模式
    pub fn learn_from_rollbacks(&mut self, log: &crate::rollback::RollbackLog) {
        use crate::rollback::RollbackTrigger;

        for entry in &log.entries {
            let now = entry.timestamp;
            match &entry.trigger {
                RollbackTrigger::InvariantViolation { rule_id, message } => {
                    if let Some(pattern) = self
                        .patterns
                        .iter_mut()
                        .find(|p| p.related_rules.contains(rule_id))
                    {
                        pattern.frequency += 1;
                        pattern.last_seen = now;
                    } else {
                        let category = category_from_rule_id(rule_id);
                        let pattern = FailurePattern {
                            id: format!("PAT-{:04}", self.next_pattern_id),
                            category,
                            description: message.clone(),
                            frequency: 1,
                            first_seen: now,
                            last_seen: now,
                            related_rules: vec![rule_id.clone()],
                        };
                        self.next_pattern_id += 1;
                        self.patterns.push(pattern);
                    }
                }
                RollbackTrigger::TestFailure {
                    test_name, output, ..
                } => {
                    // 测试失败归入 Context 类别
                    let desc = format!("测试失败: {} — {}", test_name, output);
                    if let Some(pattern) = self
                        .patterns
                        .iter_mut()
                        .find(|p| p.description == desc)
                    {
                        pattern.frequency += 1;
                        pattern.last_seen = now;
                    } else {
                        let pattern = FailurePattern {
                            id: format!("PAT-{:04}", self.next_pattern_id),
                            category: InvariantCategory::Context,
                            description: desc,
                            frequency: 1,
                            first_seen: now,
                            last_seen: now,
                            related_rules: vec![],
                        };
                        self.next_pattern_id += 1;
                        self.patterns.push(pattern);
                    }
                }
                RollbackTrigger::RuntimeViolation {
                    guard_action,
                    context,
                } => {
                    let desc = format!("运行时违规: {} — {}", guard_action, context);
                    if let Some(pattern) = self
                        .patterns
                        .iter_mut()
                        .find(|p| p.description == desc)
                    {
                        pattern.frequency += 1;
                        pattern.last_seen = now;
                    } else {
                        // 运行时违规归入 ToolPermission 类别
                        let pattern = FailurePattern {
                            id: format!("PAT-{:04}", self.next_pattern_id),
                            category: InvariantCategory::ToolPermission,
                            description: desc,
                            frequency: 1,
                            first_seen: now,
                            last_seen: now,
                            related_rules: vec![],
                        };
                        self.next_pattern_id += 1;
                        self.patterns.push(pattern);
                    }
                }
            }
        }
    }

    /// 获取所有已识别的模式
    pub fn patterns(&self) -> &[FailurePattern] {
        &self.patterns
    }

    /// 获取出现频率最高的 N 个模式
    pub fn top_patterns(&self, n: usize) -> Vec<&FailurePattern> {
        let mut sorted: Vec<&FailurePattern> = self.patterns.iter().collect();
        sorted.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        sorted.truncate(n);
        sorted
    }

    /// 为指定模式生成优化策略
    pub fn generate_strategy(&mut self, pattern_id: &str) -> Option<&EvolutionStrategy> {
        // 确认模式存在
        let pattern = self.patterns.iter().find(|p| p.id == pattern_id)?;

        // 如果已有关联策略则不重复生成
        if self.strategies.iter().any(|s| s.pattern_id == pattern_id) {
            return self
                .strategies
                .iter()
                .find(|s| s.pattern_id == pattern_id);
        }

        let (strategy_type, description, expected) =
            strategy_for_category(pattern.category, &pattern.description);

        let strategy = EvolutionStrategy {
            id: format!("STRAT-{:04}", self.next_strategy_id),
            strategy_type,
            description,
            pattern_id: pattern_id.to_string(),
            expected_improvement: expected,
            status: StrategyStatus::Proposed,
            created_at: Utc::now(),
        };
        self.next_strategy_id += 1;
        self.strategies.push(strategy);

        self.strategies.last()
    }

    /// 为所有无策略的模式生成策略
    pub fn generate_all_strategies(&mut self) -> usize {
        // 收集需要策略的模式 ID
        let pattern_ids: Vec<String> = self
            .patterns
            .iter()
            .filter(|p| !self.strategies.iter().any(|s| s.pattern_id == p.id))
            .map(|p| p.id.clone())
            .collect();

        let mut count = 0;
        for pid in pattern_ids {
            if self.generate_strategy(&pid).is_some() {
                count += 1;
            }
        }
        count
    }

    /// 更新策略状态
    pub fn update_strategy_status(&mut self, strategy_id: &str, status: StrategyStatus) -> bool {
        if let Some(strategy) = self.strategies.iter_mut().find(|s| s.id == strategy_id) {
            strategy.status = status;
            true
        } else {
            false
        }
    }

    /// 记录当前状态快照
    pub fn take_snapshot(&mut self) {
        let mut violations_by_category: HashMap<String, usize> = HashMap::new();
        for pattern in &self.patterns {
            *violations_by_category
                .entry(format!("{:?}", pattern.category))
                .or_insert(0) += pattern.frequency;
        }

        let total_violations: usize = self.patterns.iter().map(|p| p.frequency).sum();
        let active_strategies = self
            .strategies
            .iter()
            .filter(|s| {
                matches!(
                    s.status,
                    StrategyStatus::Proposed | StrategyStatus::Accepted | StrategyStatus::Executing
                )
            })
            .count();

        let snapshot = EvolutionSnapshot {
            timestamp: Utc::now(),
            total_violations,
            violations_by_category,
            total_rollbacks: 0,
            healed_count: 0,
            active_strategies,
        };
        self.snapshots.push(snapshot);
    }

    /// 获取历史快照
    pub fn snapshots(&self) -> &[EvolutionSnapshot] {
        &self.snapshots
    }

    /// 比较两个快照的差异（改善/退化）
    pub fn compare_snapshots(
        before: &EvolutionSnapshot,
        after: &EvolutionSnapshot,
    ) -> EvolutionDelta {
        let violation_delta = after.total_violations as i64 - before.total_violations as i64;
        let rollback_delta = after.total_rollbacks as i64 - before.total_rollbacks as i64;
        let healed_delta = after.healed_count as i64 - before.healed_count as i64;
        let improved = violation_delta <= 0 && healed_delta >= 0;

        EvolutionDelta {
            violation_delta,
            rollback_delta,
            healed_delta,
            improved,
        }
    }

    /// 导出进化数据为 markdown
    pub fn export_patterns_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# 失败模式分析\n\n");
        md.push_str("## 模式列表\n\n");

        for pattern in &self.patterns {
            md.push_str(&format!(
                "### {} — {}\n",
                pattern.id, pattern.description
            ));
            md.push_str(&format!("- **类别**: {:?}\n", pattern.category));
            md.push_str(&format!("- **频率**: {} 次\n", pattern.frequency));
            md.push_str(&format!(
                "- **首次出现**: {}\n",
                pattern.first_seen.format("%Y-%m-%d %H:%M:%S UTC")
            ));
            md.push_str(&format!(
                "- **最近出现**: {}\n",
                pattern.last_seen.format("%Y-%m-%d %H:%M:%S UTC")
            ));
            md.push_str(&format!(
                "- **相关规则**: {}\n\n",
                if pattern.related_rules.is_empty() {
                    "无".to_string()
                } else {
                    pattern.related_rules.join(", ")
                }
            ));
        }

        md.push_str("## 统计\n\n");
        md.push_str(&format!("- 总模式数: {}\n", self.patterns.len()));
        let max_freq = self.patterns.iter().map(|p| p.frequency).max().unwrap_or(0);
        md.push_str(&format!("- 最高频率: {}\n", max_freq));

        md
    }

    /// 导出策略为 markdown
    pub fn export_strategies_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# 优化策略\n\n");
        md.push_str("## 策略列表\n\n");

        for strategy in &self.strategies {
            md.push_str(&format!(
                "### {} — {}\n",
                strategy.id, strategy.description
            ));
            md.push_str(&format!("- **类型**: {:?}\n", strategy.strategy_type));
            md.push_str(&format!("- **关联模式**: {}\n", strategy.pattern_id));
            md.push_str(&format!("- **状态**: {:?}\n", strategy.status));
            md.push_str(&format!(
                "- **预期改善**: {}\n\n",
                strategy.expected_improvement
            ));
        }

        md.push_str("## 统计\n\n");
        md.push_str(&format!("- 总策略数: {}\n", self.strategies.len()));
        let completed = self
            .strategies
            .iter()
            .filter(|s| s.status == StrategyStatus::Completed)
            .count();
        md.push_str(&format!("- 已完成: {}\n", completed));
        let reverted = self
            .strategies
            .iter()
            .filter(|s| s.status == StrategyStatus::Reverted)
            .count();
        md.push_str(&format!("- 已回滚: {}\n", reverted));

        md
    }
}

impl Default for EvolutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 从 rule_id 前缀推导不变量类别
fn category_from_rule_id(rule_id: &str) -> InvariantCategory {
    if rule_id.starts_with("INV-AUTH") {
        InvariantCategory::Auth
    } else if rule_id.starts_with("INV-CANCEL") {
        InvariantCategory::Cancellation
    } else if rule_id.starts_with("INV-SESSION") {
        InvariantCategory::Session
    } else if rule_id.starts_with("INV-TOOL") {
        InvariantCategory::ToolPermission
    } else if rule_id.starts_with("INV-CTX") {
        InvariantCategory::Context
    } else if rule_id.starts_with("INV-AUDIT") {
        InvariantCategory::Audit
    } else if rule_id.starts_with("INV-APPROVAL") {
        InvariantCategory::Approval
    } else if rule_id.starts_with("INV-EVENT") {
        InvariantCategory::Event
    } else {
        InvariantCategory::Context
    }
}

/// 根据类别生成对应的策略类型、描述和预期改善
fn strategy_for_category(
    category: InvariantCategory,
    pattern_desc: &str,
) -> (StrategyType, String, String) {
    match category {
        InvariantCategory::Auth => (
            StrategyType::PromptImprovement,
            format!("改进认证检查: {}", pattern_desc),
            "减少认证相关违规".to_string(),
        ),
        InvariantCategory::ToolPermission => (
            StrategyType::NewInvariant,
            format!("新增工具权限不变量: {}", pattern_desc),
            "增强工具权限覆盖率".to_string(),
        ),
        InvariantCategory::Cancellation => (
            StrategyType::ToolchainOptimize,
            format!("优化取消传播链: {}", pattern_desc),
            "改善取消信号可靠性".to_string(),
        ),
        InvariantCategory::Session => (
            StrategyType::ArchitectureChange,
            format!("改进会话架构: {}", pattern_desc),
            "提升会话管理稳定性".to_string(),
        ),
        _ => (
            StrategyType::RoutingAdjust,
            format!("调整路由策略: {}", pattern_desc),
            "改善整体路由可靠性".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Violation, ViolationReport};
    use crate::rollback::{RollbackEntry, RollbackLog, RollbackResult, RollbackTrigger};
    use crate::rules::Severity;
    use chrono::Utc;

    fn sample_report() -> ViolationReport {
        ViolationReport {
            total_rules: 18,
            passed: 16,
            failed: 2,
            skipped: 0,
            violations: vec![
                Violation {
                    rule_id: "INV-AUTH-01".into(),
                    rule_name: "WebSocket 连接必须鉴权".into(),
                    severity: Severity::Critical,
                    message: "认证未启用".into(),
                    timestamp: Utc::now(),
                },
                Violation {
                    rule_id: "INV-TOOL-01".into(),
                    rule_name: "高危工具执行前必须权限检查".into(),
                    severity: Severity::High,
                    message: "工具权限检查未强制执行".into(),
                    timestamp: Utc::now(),
                },
            ],
            checked_at: Utc::now(),
            has_critical: true,
        }
    }

    fn sample_rollback_log() -> RollbackLog {
        let mut log = RollbackLog::new();
        log.add(RollbackEntry {
            id: "rollback-1".into(),
            trigger: RollbackTrigger::InvariantViolation {
                rule_id: "INV-SESSION-01".into(),
                message: "会话上下文丢失".into(),
            },
            result: RollbackResult::DryRun {
                would_revert: "abc123".into(),
            },
            timestamp: Utc::now(),
            head_before: "abc123".into(),
        });
        log.add(RollbackEntry {
            id: "rollback-2".into(),
            trigger: RollbackTrigger::TestFailure {
                test_name: "test_session".into(),
                output: "assertion failed".into(),
            },
            result: RollbackResult::DryRun {
                would_revert: "def456".into(),
            },
            timestamp: Utc::now(),
            head_before: "def456".into(),
        });
        log.add(RollbackEntry {
            id: "rollback-3".into(),
            trigger: RollbackTrigger::RuntimeViolation {
                guard_action: "block_exec".into(),
                context: "危险命令".into(),
            },
            result: RollbackResult::DryRun {
                would_revert: "ghi789".into(),
            },
            timestamp: Utc::now(),
            head_before: "ghi789".into(),
        });
        log
    }

    #[test]
    fn new_engine_empty() {
        let engine = EvolutionEngine::new();
        assert!(engine.patterns().is_empty());
        assert!(engine.strategies.is_empty());
        assert!(engine.snapshots().is_empty());
    }

    #[test]
    fn learn_from_violations() {
        let mut engine = EvolutionEngine::new();
        let report = sample_report();
        engine.learn_from_violations(&report);

        assert_eq!(engine.patterns().len(), 2);
        assert_eq!(engine.patterns()[0].category, InvariantCategory::Auth);
        assert_eq!(engine.patterns()[1].category, InvariantCategory::ToolPermission);
        assert_eq!(engine.patterns()[0].frequency, 1);
    }

    #[test]
    fn learn_increments_frequency() {
        let mut engine = EvolutionEngine::new();
        let report = sample_report();
        engine.learn_from_violations(&report);
        engine.learn_from_violations(&report);

        assert_eq!(engine.patterns().len(), 2);
        // 每个模式应被学习两次
        assert_eq!(engine.patterns()[0].frequency, 2);
        assert_eq!(engine.patterns()[1].frequency, 2);
    }

    #[test]
    fn learn_from_rollbacks() {
        let mut engine = EvolutionEngine::new();
        let log = sample_rollback_log();
        engine.learn_from_rollbacks(&log);

        // 三条回滚记录 → 三个不同模式
        assert_eq!(engine.patterns().len(), 3);
        // InvariantViolation → Session
        assert_eq!(engine.patterns()[0].category, InvariantCategory::Session);
        // TestFailure → Context
        assert_eq!(engine.patterns()[1].category, InvariantCategory::Context);
        // RuntimeViolation → ToolPermission
        assert_eq!(engine.patterns()[2].category, InvariantCategory::ToolPermission);
    }

    #[test]
    fn top_patterns_sorted() {
        let mut engine = EvolutionEngine::new();
        let report = sample_report();
        // 学习两次，然后再学习一份只有 AUTH 违规的报告
        engine.learn_from_violations(&report);
        engine.learn_from_violations(&report);

        // 再加一次 AUTH 违规
        let auth_only = ViolationReport {
            total_rules: 18,
            passed: 17,
            failed: 1,
            skipped: 0,
            violations: vec![Violation {
                rule_id: "INV-AUTH-01".into(),
                rule_name: "WebSocket 连接必须鉴权".into(),
                severity: Severity::Critical,
                message: "认证未启用".into(),
                timestamp: Utc::now(),
            }],
            checked_at: Utc::now(),
            has_critical: true,
        };
        engine.learn_from_violations(&auth_only);

        // AUTH: 3次, TOOL: 2次
        let top = engine.top_patterns(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].category, InvariantCategory::Auth);
        assert_eq!(top[0].frequency, 3);

        let top2 = engine.top_patterns(2);
        assert_eq!(top2.len(), 2);
        assert!(top2[0].frequency >= top2[1].frequency);
    }

    #[test]
    fn generate_strategy_for_pattern() {
        let mut engine = EvolutionEngine::new();
        engine.learn_from_violations(&sample_report());

        let pattern_id = engine.patterns()[0].id.clone();
        let strategy = engine.generate_strategy(&pattern_id).unwrap();

        assert_eq!(strategy.pattern_id, pattern_id);
        assert_eq!(strategy.strategy_type, StrategyType::PromptImprovement);
        assert_eq!(strategy.status, StrategyStatus::Proposed);
    }

    #[test]
    fn generate_strategy_unknown_pattern() {
        let mut engine = EvolutionEngine::new();
        assert!(engine.generate_strategy("PAT-9999").is_none());
    }

    #[test]
    fn generate_all_strategies() {
        let mut engine = EvolutionEngine::new();
        engine.learn_from_violations(&sample_report());

        let count = engine.generate_all_strategies();
        assert_eq!(count, 2);
        assert_eq!(engine.strategies.len(), 2);

        // 再次调用不应重复生成
        let count2 = engine.generate_all_strategies();
        assert_eq!(count2, 0);
    }

    #[test]
    fn strategy_lifecycle() {
        let mut engine = EvolutionEngine::new();
        engine.learn_from_violations(&sample_report());
        let pattern_id = engine.patterns()[0].id.clone();
        engine.generate_strategy(&pattern_id);

        let strategy_id = engine.strategies[0].id.clone();

        assert!(engine.update_strategy_status(&strategy_id, StrategyStatus::Accepted));
        assert_eq!(engine.strategies[0].status, StrategyStatus::Accepted);

        assert!(engine.update_strategy_status(&strategy_id, StrategyStatus::Executing));
        assert_eq!(engine.strategies[0].status, StrategyStatus::Executing);

        assert!(engine.update_strategy_status(&strategy_id, StrategyStatus::Completed));
        assert_eq!(engine.strategies[0].status, StrategyStatus::Completed);
    }

    #[test]
    fn strategy_revert() {
        let mut engine = EvolutionEngine::new();
        engine.learn_from_violations(&sample_report());
        let pattern_id = engine.patterns()[0].id.clone();
        engine.generate_strategy(&pattern_id);

        let strategy_id = engine.strategies[0].id.clone();
        assert!(engine.update_strategy_status(&strategy_id, StrategyStatus::Reverted));
        assert_eq!(engine.strategies[0].status, StrategyStatus::Reverted);

        // 更新不存在的策略返回 false
        assert!(!engine.update_strategy_status("STRAT-9999", StrategyStatus::Reverted));
    }

    #[test]
    fn compare_snapshots_improved() {
        let before = EvolutionSnapshot {
            timestamp: Utc::now(),
            total_violations: 10,
            violations_by_category: HashMap::new(),
            total_rollbacks: 5,
            healed_count: 2,
            active_strategies: 3,
        };
        let after = EvolutionSnapshot {
            timestamp: Utc::now(),
            total_violations: 5,
            violations_by_category: HashMap::new(),
            total_rollbacks: 3,
            healed_count: 4,
            active_strategies: 2,
        };

        let delta = EvolutionEngine::compare_snapshots(&before, &after);
        assert_eq!(delta.violation_delta, -5);
        assert_eq!(delta.rollback_delta, -2);
        assert_eq!(delta.healed_delta, 2);
        assert!(delta.improved);
    }

    #[test]
    fn compare_snapshots_degraded() {
        let before = EvolutionSnapshot {
            timestamp: Utc::now(),
            total_violations: 5,
            violations_by_category: HashMap::new(),
            total_rollbacks: 2,
            healed_count: 4,
            active_strategies: 1,
        };
        let after = EvolutionSnapshot {
            timestamp: Utc::now(),
            total_violations: 10,
            violations_by_category: HashMap::new(),
            total_rollbacks: 5,
            healed_count: 3,
            active_strategies: 2,
        };

        let delta = EvolutionEngine::compare_snapshots(&before, &after);
        assert_eq!(delta.violation_delta, 5);
        assert!(delta.violation_delta > 0);
        assert!(!delta.improved);
    }

    #[test]
    fn export_markdown() {
        let mut engine = EvolutionEngine::new();
        engine.learn_from_violations(&sample_report());
        engine.generate_all_strategies();

        let patterns_md = engine.export_patterns_markdown();
        assert!(patterns_md.contains("# 失败模式分析"));
        assert!(patterns_md.contains("## 模式列表"));
        assert!(patterns_md.contains("PAT-0001"));
        assert!(patterns_md.contains("- **类别**:"));
        assert!(patterns_md.contains("- **频率**: 1 次"));
        assert!(patterns_md.contains("## 统计"));
        assert!(patterns_md.contains("- 总模式数: 2"));

        let strategies_md = engine.export_strategies_markdown();
        assert!(strategies_md.contains("# 优化策略"));
        assert!(strategies_md.contains("## 策略列表"));
        assert!(strategies_md.contains("STRAT-0001"));
        assert!(strategies_md.contains("- **类型**:"));
        assert!(strategies_md.contains("- **状态**: Proposed"));
        assert!(strategies_md.contains("## 统计"));
        assert!(strategies_md.contains("- 总策略数: 2"));
    }

    #[test]
    fn snapshot_captures_state() {
        let mut engine = EvolutionEngine::new();
        engine.learn_from_violations(&sample_report());
        engine.generate_all_strategies();
        engine.take_snapshot();

        assert_eq!(engine.snapshots().len(), 1);
        let snap = &engine.snapshots()[0];
        assert_eq!(snap.total_violations, 2);
        assert_eq!(snap.active_strategies, 2);
    }
}
