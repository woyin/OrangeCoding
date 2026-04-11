use crate::rules::Severity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 验证检查项类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationCheck {
    /// 是否符合 analysis.md 设计模式
    DesignConformance,
    /// 是否违反不变量
    InvariantCompliance,
    /// 是否存在绕过路径
    BypassDetection,
    /// 是否引入新 bug（基于测试结果）
    RegressionCheck,
    /// 安全性检查
    SecurityCheck,
}

/// 单项验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationItem {
    /// 检查类型
    pub check: VerificationCheck,
    /// 是否通过
    pub passed: bool,
    /// 详细说明
    pub details: String,
    /// 严重性（仅在未通过时有意义）
    pub severity: Option<Severity>,
}

/// 验证结果 — 通过或失败
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationVerdict {
    /// 所有检查通过
    Approved,
    /// 存在问题需要修复
    NeedsWork,
    /// 存在严重问题，必须回滚
    Rejected,
}

/// 完整验证报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    /// 被验证的 TODO ID
    pub todo_id: String,
    /// 被验证的 TODO 描述
    pub todo_title: String,
    /// 验证时间
    pub verified_at: DateTime<Utc>,
    /// 各项检查结果
    pub items: Vec<VerificationItem>,
    /// 最终裁决
    pub verdict: VerificationVerdict,
    /// 总结说明
    pub summary: String,
}

/// 验证代理 — 对已完成的 TODO 进行全面校验
pub struct VerificationAgent {
    /// 不变量检查器引用
    checker: crate::checker::InvariantChecker,
}

impl VerificationAgent {
    /// 使用系统默认规则创建验证代理
    pub fn new() -> Self {
        Self {
            checker: crate::checker::InvariantChecker::with_system_rules(),
        }
    }

    /// 使用自定义检查器创建
    pub fn with_checker(checker: crate::checker::InvariantChecker) -> Self {
        Self { checker }
    }

    /// 对一个已完成的 TODO 执行完整验证
    ///
    /// - `todo_id`: TODO 标识
    /// - `todo_title`: TODO 描述
    /// - `check_context`: 当前系统运行状态
    /// - `test_passed`: 相关测试是否通过
    /// - `diff`: 变更的 diff（用于 bypass / security / design 检测）
    pub fn verify(
        &self,
        todo_id: &str,
        todo_title: &str,
        check_context: &crate::checker::CheckContext,
        test_passed: bool,
        diff: Option<&str>,
    ) -> VerificationReport {
        let items = vec![
            self.check_invariant_compliance(check_context),
            Self::check_regression(test_passed),
            Self::check_bypass(diff),
            Self::check_security(diff),
            Self::check_design_conformance(diff),
        ];

        let verdict = Self::determine_verdict(&items);

        // 生成总结
        let failed_count = items.iter().filter(|i| !i.passed).count();
        let summary = if failed_count == 0 {
            "所有验证检查通过，实现符合要求".to_string()
        } else {
            let failed_names: Vec<&str> = items
                .iter()
                .filter(|i| !i.passed)
                .map(|i| match i.check {
                    VerificationCheck::DesignConformance => "设计一致性",
                    VerificationCheck::InvariantCompliance => "不变量合规",
                    VerificationCheck::BypassDetection => "绕过检测",
                    VerificationCheck::RegressionCheck => "回归测试",
                    VerificationCheck::SecurityCheck => "安全检查",
                })
                .collect();
            format!(
                "共 {} 项检查未通过: {}",
                failed_count,
                failed_names.join(", ")
            )
        };

        VerificationReport {
            todo_id: todo_id.to_string(),
            todo_title: todo_title.to_string(),
            verified_at: Utc::now(),
            items,
            verdict,
            summary,
        }
    }

    /// 检查不变量合规性 — 运行内置 InvariantChecker
    fn check_invariant_compliance(
        &self,
        ctx: &crate::checker::CheckContext,
    ) -> VerificationItem {
        let report = self.checker.check(ctx);
        if report.is_clean() {
            VerificationItem {
                check: VerificationCheck::InvariantCompliance,
                passed: true,
                details: "所有不变量检查通过".to_string(),
                severity: None,
            }
        } else {
            // 收集违规描述
            let violation_list: Vec<String> = report
                .violations
                .iter()
                .map(|v| format!("{}: {}", v.rule_id, v.message))
                .collect();
            // 取最高严重性
            let max_severity = report
                .violations
                .iter()
                .map(|v| v.severity)
                .max_by_key(|s| match s {
                    Severity::Critical => 4,
                    Severity::High => 3,
                    Severity::Medium => 2,
                    Severity::Low => 1,
                });
            VerificationItem {
                check: VerificationCheck::InvariantCompliance,
                passed: false,
                details: format!("不变量违规: {}", violation_list.join("; ")),
                severity: max_severity,
            }
        }
    }

    /// 检查回归（基于测试结果）
    fn check_regression(test_passed: bool) -> VerificationItem {
        if test_passed {
            VerificationItem {
                check: VerificationCheck::RegressionCheck,
                passed: true,
                details: "相关测试全部通过".to_string(),
                severity: None,
            }
        } else {
            VerificationItem {
                check: VerificationCheck::RegressionCheck,
                passed: false,
                details: "相关测试未通过".to_string(),
                severity: Some(Severity::Critical),
            }
        }
    }

    /// 检查绕过路径（分析 diff 中的危险模式）
    fn check_bypass(diff: Option<&str>) -> VerificationItem {
        let Some(diff_text) = diff else {
            return VerificationItem {
                check: VerificationCheck::BypassDetection,
                passed: true,
                details: "无 diff 提供，跳过绕过检测".to_string(),
                severity: None,
            };
        };

        let bypass_patterns: &[(&str, &str)] = &[
            ("#[allow(unused)]", "安全函数上使用 #[allow(unused)]"),
            ("unsafe", "使用了 unsafe 代码块"),
            (".unwrap()", "对可能涉及权限/认证结果使用 unwrap()"),
            ("#[ignore]", "测试标记了 #[ignore]"),
            ("skip", "存在 skip 标记"),
        ];

        let mut found: Vec<String> = Vec::new();
        for (pattern, description) in bypass_patterns {
            if diff_text.contains(pattern) {
                found.push(description.to_string());
            }
        }

        if found.is_empty() {
            VerificationItem {
                check: VerificationCheck::BypassDetection,
                passed: true,
                details: "未检测到绕过路径".to_string(),
                severity: None,
            }
        } else {
            VerificationItem {
                check: VerificationCheck::BypassDetection,
                passed: false,
                details: format!("检测到潜在绕过: {}", found.join("; ")),
                severity: Some(Severity::High),
            }
        }
    }

    /// 检查安全性（分析 diff 中的安全风险模式）
    fn check_security(diff: Option<&str>) -> VerificationItem {
        let Some(diff_text) = diff else {
            return VerificationItem {
                check: VerificationCheck::SecurityCheck,
                passed: true,
                details: "无 diff 提供，跳过安全检查".to_string(),
                severity: None,
            };
        };

        let security_patterns: &[(&str, &str)] = &[
            ("password = \"", "硬编码密码"),
            ("token = \"", "硬编码 token"),
            ("secret = \"", "硬编码 secret"),
            ("TODO: security", "存在安全相关 TODO"),
            ("FIXME: security", "存在安全相关 FIXME"),
        ];

        let mut found: Vec<String> = Vec::new();
        for (pattern, description) in security_patterns {
            if diff_text.contains(pattern) {
                found.push(description.to_string());
            }
        }

        if found.is_empty() {
            VerificationItem {
                check: VerificationCheck::SecurityCheck,
                passed: true,
                details: "未检测到安全风险".to_string(),
                severity: None,
            }
        } else {
            VerificationItem {
                check: VerificationCheck::SecurityCheck,
                passed: false,
                details: format!("安全风险: {}", found.join("; ")),
                severity: Some(Severity::Critical),
            }
        }
    }

    /// 检查设计一致性（pub fn 是否带有注释）
    fn check_design_conformance(diff: Option<&str>) -> VerificationItem {
        let Some(diff_text) = diff else {
            return VerificationItem {
                check: VerificationCheck::DesignConformance,
                passed: true,
                details: "无 diff 提供，跳过设计一致性检查".to_string(),
                severity: None,
            };
        };

        // 检查新增的 pub fn 是否有注释：
        // 在 diff 中，新增行以 '+' 开头，如果出现 pub fn 但前一行不是注释行则警告
        let lines: Vec<&str> = diff_text.lines().collect();
        let mut uncommented_fns: Vec<String> = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            // 只看新增行
            if !trimmed.starts_with('+') {
                continue;
            }
            let content = trimmed.trim_start_matches('+').trim();
            if content.starts_with("pub fn ") {
                // 检查前一行是否是注释（/// 或 //）
                let has_comment = if i > 0 {
                    let prev = lines[i - 1].trim().trim_start_matches('+').trim();
                    prev.starts_with("///") || prev.starts_with("//")
                } else {
                    false
                };
                if !has_comment {
                    // 提取函数名
                    if let Some(fn_name) = content
                        .strip_prefix("pub fn ")
                        .and_then(|s| s.split('(').next())
                    {
                        uncommented_fns.push(fn_name.to_string());
                    }
                }
            }
        }

        if uncommented_fns.is_empty() {
            VerificationItem {
                check: VerificationCheck::DesignConformance,
                passed: true,
                details: "设计一致性检查通过".to_string(),
                severity: None,
            }
        } else {
            VerificationItem {
                check: VerificationCheck::DesignConformance,
                passed: false,
                details: format!(
                    "以下 pub fn 缺少注释: {}",
                    uncommented_fns.join(", ")
                ),
                severity: Some(Severity::Low),
            }
        }
    }

    /// 根据检查结果确定最终裁决
    fn determine_verdict(items: &[VerificationItem]) -> VerificationVerdict {
        let has_critical_failure = items
            .iter()
            .any(|i| !i.passed && i.severity == Some(Severity::Critical));

        if has_critical_failure {
            VerificationVerdict::Rejected
        } else if items.iter().any(|i| !i.passed) {
            VerificationVerdict::NeedsWork
        } else {
            VerificationVerdict::Approved
        }
    }
}

impl VerificationReport {
    /// 是否所有检查通过
    pub fn is_approved(&self) -> bool {
        self.verdict == VerificationVerdict::Approved
    }

    /// 生成 markdown 报告
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# 验证报告: {}\n\n", self.todo_id));
        md.push_str(&format!("**任务**: {}\n", self.todo_title));
        md.push_str(&format!(
            "**时间**: {}\n",
            self.verified_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        let verdict_str = match &self.verdict {
            VerificationVerdict::Approved => "✅ Approved",
            VerificationVerdict::NeedsWork => "⚠️ NeedsWork",
            VerificationVerdict::Rejected => "❌ Rejected",
        };
        md.push_str(&format!("**裁决**: {}\n\n", verdict_str));

        md.push_str("## 检查结果\n\n");
        md.push_str("| 检查项 | 结果 | 说明 |\n");
        md.push_str("|--------|------|------|\n");

        for item in &self.items {
            let check_name = match &item.check {
                VerificationCheck::DesignConformance => "设计一致性",
                VerificationCheck::InvariantCompliance => "不变量合规",
                VerificationCheck::BypassDetection => "绕过检测",
                VerificationCheck::RegressionCheck => "回归测试",
                VerificationCheck::SecurityCheck => "安全检查",
            };
            let status = if item.passed { "✅" } else { "❌" };
            md.push_str(&format!(
                "| {} | {} | {} |\n",
                check_name, status, item.details
            ));
        }

        md.push_str(&format!("\n## 总结\n\n{}\n", self.summary));

        md
    }
}

impl Default for VerificationAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::CheckContext;

    /// 构造一个健康的系统上下文
    fn healthy_context() -> CheckContext {
        CheckContext {
            active_sessions: 1,
            auth_enabled: true,
            pending_approvals: 0,
            events_monotonic: true,
            tool_permission_enforced: true,
            audit_chain_valid: true,
            custom_checks: vec![],
        }
    }

    #[test]
    fn verify_all_pass() {
        let agent = VerificationAgent::new();
        let ctx = healthy_context();
        let clean_diff = Some("+ /// 新增功能\n+ pub fn do_work() {}");

        let report = agent.verify("TODO-001", "实现基础功能", &ctx, true, clean_diff);

        assert_eq!(report.verdict, VerificationVerdict::Approved);
        assert!(report.is_approved());
        assert!(report.items.iter().all(|i| i.passed));
    }

    #[test]
    fn verify_invariant_violation() {
        let agent = VerificationAgent::new();
        let mut ctx = healthy_context();
        ctx.auth_enabled = false; // 触发 Critical 不变量违规

        let report = agent.verify("TODO-002", "修改认证模块", &ctx, true, None);

        assert!(!report.is_approved());
        // auth_enabled=false 触发 Critical 级别违规，应被 Rejected
        assert_eq!(report.verdict, VerificationVerdict::Rejected);

        let inv_item = report
            .items
            .iter()
            .find(|i| i.check == VerificationCheck::InvariantCompliance)
            .unwrap();
        assert!(!inv_item.passed);
    }

    #[test]
    fn verify_test_failure() {
        let agent = VerificationAgent::new();
        let ctx = healthy_context();

        let report = agent.verify("TODO-003", "新功能开发", &ctx, false, None);

        // 测试失败 → Critical → Rejected
        assert_eq!(report.verdict, VerificationVerdict::Rejected);
        assert!(!report.is_approved());

        let reg_item = report
            .items
            .iter()
            .find(|i| i.check == VerificationCheck::RegressionCheck)
            .unwrap();
        assert!(!reg_item.passed);
        assert_eq!(reg_item.severity, Some(Severity::Critical));
    }

    #[test]
    fn verify_bypass_detected() {
        let agent = VerificationAgent::new();
        let ctx = healthy_context();
        let diff_with_bypass = "fn check_auth() {\n    auth_result.unwrap()\n}";

        let report = agent.verify("TODO-004", "权限调整", &ctx, true, Some(diff_with_bypass));

        // unwrap() 触发 bypass 检测（High 级别）→ NeedsWork
        assert_eq!(report.verdict, VerificationVerdict::NeedsWork);

        let bypass_item = report
            .items
            .iter()
            .find(|i| i.check == VerificationCheck::BypassDetection)
            .unwrap();
        assert!(!bypass_item.passed);
        assert!(bypass_item.details.contains("unwrap()"));
    }

    #[test]
    fn verify_security_risk() {
        let agent = VerificationAgent::new();
        let ctx = healthy_context();
        let diff_with_secret = "let password = \"hunter2\";";

        let report = agent.verify(
            "TODO-005",
            "配置管理",
            &ctx,
            true,
            Some(diff_with_secret),
        );

        // 硬编码密码 → Critical → Rejected
        assert_eq!(report.verdict, VerificationVerdict::Rejected);

        let sec_item = report
            .items
            .iter()
            .find(|i| i.check == VerificationCheck::SecurityCheck)
            .unwrap();
        assert!(!sec_item.passed);
        assert_eq!(sec_item.severity, Some(Severity::Critical));
    }

    #[test]
    fn verify_no_diff() {
        let agent = VerificationAgent::new();
        let ctx = healthy_context();

        let report = agent.verify("TODO-006", "文档更新", &ctx, true, None);

        // 无 diff 时所有基于 diff 的检查均应通过
        assert_eq!(report.verdict, VerificationVerdict::Approved);
        assert!(report.is_approved());
    }

    #[test]
    fn verify_design_conformance() {
        let agent = VerificationAgent::new();
        let ctx = healthy_context();
        // pub fn 前一行不是注释
        let diff_no_comment = "+ \n+ pub fn handle_request() {}";

        let report = agent.verify(
            "TODO-007",
            "新增 API",
            &ctx,
            true,
            Some(diff_no_comment),
        );

        // 缺少注释 → Low 级别 → NeedsWork
        assert_eq!(report.verdict, VerificationVerdict::NeedsWork);

        let design_item = report
            .items
            .iter()
            .find(|i| i.check == VerificationCheck::DesignConformance)
            .unwrap();
        assert!(!design_item.passed);
        assert!(design_item.details.contains("handle_request"));
    }

    #[test]
    fn report_markdown_output() {
        let agent = VerificationAgent::new();
        let ctx = healthy_context();
        let report = agent.verify("TODO-008", "Markdown 测试", &ctx, true, None);

        let md = report.to_markdown();

        assert!(md.contains("# 验证报告: TODO-008"));
        assert!(md.contains("**任务**: Markdown 测试"));
        assert!(md.contains("**裁决**:"));
        assert!(md.contains("| 检查项 | 结果 | 说明 |"));
        assert!(md.contains("## 总结"));
    }

    #[test]
    fn verdict_approved_is_approved() {
        let report = VerificationReport {
            todo_id: "T-1".to_string(),
            todo_title: "test".to_string(),
            verified_at: Utc::now(),
            items: vec![],
            verdict: VerificationVerdict::Approved,
            summary: String::new(),
        };
        assert!(report.is_approved());
    }

    #[test]
    fn verdict_rejected_is_not_approved() {
        let report = VerificationReport {
            todo_id: "T-2".to_string(),
            todo_title: "test".to_string(),
            verified_at: Utc::now(),
            items: vec![],
            verdict: VerificationVerdict::Rejected,
            summary: String::new(),
        };
        assert!(!report.is_approved());
    }
}
