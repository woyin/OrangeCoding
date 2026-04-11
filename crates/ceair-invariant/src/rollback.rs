//! Auto Rollback: 自动回滚模块
//!
//! 当测试失败、不变量违规或运行时违规时自动 git revert。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// 回滚触发原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackTrigger {
    /// 测试失败
    TestFailure { test_name: String, output: String },
    /// 不变量违规
    InvariantViolation { rule_id: String, message: String },
    /// 运行时守卫拦截
    RuntimeViolation {
        guard_action: String,
        context: String,
    },
}

/// 回滚执行结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackResult {
    /// 成功回滚
    Success { reverted_commit: String },
    /// 回滚失败
    Failure { reason: String },
    /// 干运行（未实际执行）
    DryRun { would_revert: String },
}

/// 单条回滚记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackEntry {
    /// 回滚 ID
    pub id: String,
    /// 触发原因
    pub trigger: RollbackTrigger,
    /// 回滚结果
    pub result: RollbackResult,
    /// 回滚时间
    pub timestamp: DateTime<Utc>,
    /// 当前 HEAD commit（回滚前）
    pub head_before: String,
}

/// 回滚日志 — 记录所有回滚操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackLog {
    pub entries: Vec<RollbackEntry>,
}

impl RollbackLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, entry: RollbackEntry) {
        self.entries.push(entry);
    }

    pub fn total_rollbacks(&self) -> usize {
        self.entries.len()
    }

    pub fn successful_rollbacks(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e.result, RollbackResult::Success { .. }))
            .count()
    }
}

/// 自动回滚管理器
pub struct AutoRollback {
    /// 是否干运行模式（不实际执行 git 命令）
    dry_run: bool,
    /// 回滚日志
    log: RollbackLog,
    /// 日志输出路径
    log_path: Option<PathBuf>,
}

impl AutoRollback {
    /// 创建新的回滚管理器
    pub fn new(dry_run: bool) -> Self {
        Self {
            dry_run,
            log: RollbackLog::new(),
            log_path: None,
        }
    }

    /// 设置日志输出路径
    pub fn with_log_path(mut self, path: PathBuf) -> Self {
        self.log_path = Some(path);
        self
    }

    /// 根据不变量违规报告触发回滚
    ///
    /// 仅当存在 critical 违规时才触发回滚
    pub fn trigger_from_violations(
        &mut self,
        report: &crate::report::ViolationReport,
    ) -> Vec<RollbackResult> {
        if report.is_clean() {
            return vec![];
        }

        // 仅对 critical 违规执行回滚
        if !report.has_critical {
            return vec![];
        }

        let head = Self::get_head_commit();
        let result = self.execute_revert();

        let mut results = Vec::new();
        for violation in &report.violations {
            let entry = RollbackEntry {
                id: format!("rollback-{}", self.log.total_rollbacks() + 1),
                trigger: RollbackTrigger::InvariantViolation {
                    rule_id: violation.rule_id.clone(),
                    message: violation.message.clone(),
                },
                result: result.clone(),
                timestamp: Utc::now(),
                head_before: head.clone(),
            };
            results.push(entry.result.clone());
            self.log.add(entry);
        }

        results
    }

    /// 根据测试失败触发回滚
    pub fn trigger_from_test_failure(&mut self, test_name: &str, output: &str) -> RollbackResult {
        let head = Self::get_head_commit();
        let result = self.execute_revert();

        let entry = RollbackEntry {
            id: format!("rollback-{}", self.log.total_rollbacks() + 1),
            trigger: RollbackTrigger::TestFailure {
                test_name: test_name.to_string(),
                output: output.to_string(),
            },
            result: result.clone(),
            timestamp: Utc::now(),
            head_before: head,
        };
        self.log.add(entry);

        result
    }

    /// 根据运行时违规触发回滚
    pub fn trigger_from_runtime_violation(
        &mut self,
        action: &str,
        context: &str,
    ) -> RollbackResult {
        let head = Self::get_head_commit();
        let result = self.execute_revert();

        let entry = RollbackEntry {
            id: format!("rollback-{}", self.log.total_rollbacks() + 1),
            trigger: RollbackTrigger::RuntimeViolation {
                guard_action: action.to_string(),
                context: context.to_string(),
            },
            result: result.clone(),
            timestamp: Utc::now(),
            head_before: head,
        };
        self.log.add(entry);

        result
    }

    /// 执行 git revert（或干运行）
    fn execute_revert(&self) -> RollbackResult {
        if self.dry_run {
            return RollbackResult::DryRun {
                would_revert: Self::get_head_commit(),
            };
        }

        let head_before = Self::get_head_commit();
        match Command::new("git")
            .args(["revert", "HEAD", "--no-edit"])
            .output()
        {
            Ok(output) if output.status.success() => RollbackResult::Success {
                reverted_commit: head_before,
            },
            Ok(output) => RollbackResult::Failure {
                reason: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            },
            Err(e) => RollbackResult::Failure {
                reason: e.to_string(),
            },
        }
    }

    /// 获取当前 HEAD commit hash
    fn get_head_commit() -> String {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// 获取回滚日志
    pub fn get_log(&self) -> &RollbackLog {
        &self.log
    }

    /// 将日志导出为 markdown
    pub fn export_log_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# 回滚日志\n\n");
        md.push_str("| ID | 触发类型 | 结果 | 时间 |\n");
        md.push_str("|----|----------|------|------|\n");

        for entry in &self.log.entries {
            let trigger_type = match &entry.trigger {
                RollbackTrigger::TestFailure { test_name, .. } => {
                    format!("测试失败: {}", test_name)
                }
                RollbackTrigger::InvariantViolation { rule_id, .. } => {
                    format!("不变量违规: {}", rule_id)
                }
                RollbackTrigger::RuntimeViolation { guard_action, .. } => {
                    format!("运行时违规: {}", guard_action)
                }
            };

            let result_str = match &entry.result {
                RollbackResult::Success { reverted_commit } => {
                    format!("✅ 成功 ({})", reverted_commit)
                }
                RollbackResult::Failure { reason } => format!("❌ 失败 ({})", reason),
                RollbackResult::DryRun { would_revert } => {
                    format!("🔍 干运行 ({})", would_revert)
                }
            };

            let time = entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC");
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                entry.id, trigger_type, result_str, time
            ));
        }

        // 统计
        let total = self.log.total_rollbacks();
        let success = self.log.successful_rollbacks();
        let failed = self
            .log
            .entries
            .iter()
            .filter(|e| matches!(e.result, RollbackResult::Failure { .. }))
            .count();

        md.push_str("\n## 统计\n\n");
        md.push_str(&format!("- 总回滚次数: {}\n", total));
        md.push_str(&format!("- 成功: {}\n", success));
        md.push_str(&format!("- 失败: {}\n", failed));

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Violation, ViolationReport};
    use crate::rules::Severity;
    use chrono::Utc;

    fn clean_report() -> ViolationReport {
        ViolationReport {
            total_rules: 18,
            passed: 18,
            failed: 0,
            skipped: 0,
            violations: vec![],
            checked_at: Utc::now(),
            has_critical: false,
        }
    }

    fn critical_report() -> ViolationReport {
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
                    rule_id: "INV-AUTH-02".into(),
                    rule_name: "HTTP API 必须通过认证中间件".into(),
                    severity: Severity::Critical,
                    message: "认证中间件未配置".into(),
                    timestamp: Utc::now(),
                },
            ],
            checked_at: Utc::now(),
            has_critical: true,
        }
    }

    #[test]
    fn new_rollback_manager() {
        let mgr = AutoRollback::new(true);
        assert!(mgr.dry_run);
        assert!(mgr.log.entries.is_empty());
        assert!(mgr.log_path.is_none());
    }

    #[test]
    fn dry_run_revert() {
        let mgr = AutoRollback::new(true);
        let result = mgr.execute_revert();
        match result {
            RollbackResult::DryRun { would_revert } => {
                // 干运行模式下应返回当前 HEAD 或 "unknown"
                assert!(!would_revert.is_empty());
            }
            _ => panic!("dry_run 模式应返回 DryRun 结果"),
        }
    }

    #[test]
    fn trigger_from_test_failure_dry_run() {
        let mut mgr = AutoRollback::new(true);
        let result = mgr.trigger_from_test_failure("test_auth", "assertion failed");

        assert!(matches!(result, RollbackResult::DryRun { .. }));
        assert_eq!(mgr.get_log().total_rollbacks(), 1);

        let entry = &mgr.get_log().entries[0];
        assert!(matches!(
            &entry.trigger,
            RollbackTrigger::TestFailure { test_name, output }
            if test_name == "test_auth" && output == "assertion failed"
        ));
    }

    #[test]
    fn trigger_from_runtime_violation_dry_run() {
        let mut mgr = AutoRollback::new(true);
        let result = mgr.trigger_from_runtime_violation("block_exec", "危险命令被拦截");

        assert!(matches!(result, RollbackResult::DryRun { .. }));
        assert_eq!(mgr.get_log().total_rollbacks(), 1);

        let entry = &mgr.get_log().entries[0];
        assert!(matches!(
            &entry.trigger,
            RollbackTrigger::RuntimeViolation { guard_action, context }
            if guard_action == "block_exec" && context == "危险命令被拦截"
        ));
    }

    #[test]
    fn trigger_from_clean_report() {
        let mut mgr = AutoRollback::new(true);
        let results = mgr.trigger_from_violations(&clean_report());

        assert!(results.is_empty());
        assert_eq!(mgr.get_log().total_rollbacks(), 0);
    }

    #[test]
    fn trigger_from_violations_with_critical() {
        let mut mgr = AutoRollback::new(true);
        let results = mgr.trigger_from_violations(&critical_report());

        // 每个违规都应产生一条回滚记录
        assert_eq!(results.len(), 2);
        assert_eq!(mgr.get_log().total_rollbacks(), 2);

        for result in &results {
            assert!(matches!(result, RollbackResult::DryRun { .. }));
        }
    }

    #[test]
    fn rollback_log_counts() {
        let mut log = RollbackLog::new();
        assert_eq!(log.total_rollbacks(), 0);
        assert_eq!(log.successful_rollbacks(), 0);

        log.add(RollbackEntry {
            id: "rollback-1".into(),
            trigger: RollbackTrigger::TestFailure {
                test_name: "test_a".into(),
                output: "fail".into(),
            },
            result: RollbackResult::Success {
                reverted_commit: "abc123".into(),
            },
            timestamp: Utc::now(),
            head_before: "abc123".into(),
        });

        log.add(RollbackEntry {
            id: "rollback-2".into(),
            trigger: RollbackTrigger::TestFailure {
                test_name: "test_b".into(),
                output: "fail".into(),
            },
            result: RollbackResult::Failure {
                reason: "merge conflict".into(),
            },
            timestamp: Utc::now(),
            head_before: "def456".into(),
        });

        log.add(RollbackEntry {
            id: "rollback-3".into(),
            trigger: RollbackTrigger::RuntimeViolation {
                guard_action: "block".into(),
                context: "ctx".into(),
            },
            result: RollbackResult::DryRun {
                would_revert: "ghi789".into(),
            },
            timestamp: Utc::now(),
            head_before: "ghi789".into(),
        });

        assert_eq!(log.total_rollbacks(), 3);
        assert_eq!(log.successful_rollbacks(), 1);
    }

    #[test]
    fn export_log_markdown_format() {
        let mut mgr = AutoRollback::new(true);
        mgr.trigger_from_test_failure("test_auth", "failed");
        mgr.trigger_from_runtime_violation("block_exec", "拦截");

        let md = mgr.export_log_markdown();

        assert!(md.contains("# 回滚日志"));
        assert!(md.contains("| ID | 触发类型 | 结果 | 时间 |"));
        assert!(md.contains("rollback-1"));
        assert!(md.contains("rollback-2"));
        assert!(md.contains("测试失败: test_auth"));
        assert!(md.contains("运行时违规: block_exec"));
        assert!(md.contains("🔍 干运行"));
        assert!(md.contains("## 统计"));
        assert!(md.contains("- 总回滚次数: 2"));
        assert!(md.contains("- 成功: 0"));
        assert!(md.contains("- 失败: 0"));
    }
}
