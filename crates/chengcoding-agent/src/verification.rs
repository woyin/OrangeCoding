//! # 验证 Agent 框架
//!
//! 自动化验证 Agent，在实现完成后检查代码质量。
//!
//! # 设计思想
//! 参考 reference 中 Verification Agent 的设计：
//! - 基于 Fork Agent 模式，以只读方式检查代码
//! - 检查维度覆盖设计合规、破坏性变更、设计缺陷、测试有效性
//! - 结果结构化，包含问题列表和建议
//! - 失败不自动阻止，由调用方决定处理策略

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 验证检查类型
// ---------------------------------------------------------------------------

/// 验证检查类型
///
/// 每种检查类型对应一个验证维度
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationCheck {
    /// 是否符合设计文档（docs/analysis.md）
    DesignCompliance,
    /// 是否破坏现有模块的功能
    BreakingChanges,
    /// 是否存在明显的设计缺陷
    DesignDefects,
    /// 测试是否有效（覆盖正常/异常/边界）
    TestEffectiveness,
    /// 是否存在无意义测试（空测试、无断言测试）
    MeaninglessTests,
}

impl VerificationCheck {
    /// 所有检查类型
    pub fn all() -> Vec<Self> {
        vec![
            Self::DesignCompliance,
            Self::BreakingChanges,
            Self::DesignDefects,
            Self::TestEffectiveness,
            Self::MeaninglessTests,
        ]
    }

    /// 检查类型的中文描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::DesignCompliance => "设计合规性检查",
            Self::BreakingChanges => "破坏性变更检查",
            Self::DesignDefects => "设计缺陷检查",
            Self::TestEffectiveness => "测试有效性检查",
            Self::MeaninglessTests => "无意义测试检查",
        }
    }
}

// ---------------------------------------------------------------------------
// 验证问题
// ---------------------------------------------------------------------------

/// 问题严重级别
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// 错误 — 必须修复
    Error,
    /// 警告 — 建议修复
    Warning,
    /// 信息 — 供参考
    Info,
}

/// 验证发现的问题
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationIssue {
    /// 问题所属的检查类型
    pub check: VerificationCheck,
    /// 严重级别
    pub severity: Severity,
    /// 问题描述
    pub message: String,
    /// 相关文件路径（可选）
    pub file_path: Option<String>,
}

// ---------------------------------------------------------------------------
// 验证结果
// ---------------------------------------------------------------------------

/// 验证结果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    /// 是否通过（无 Error 级别问题）
    pub passed: bool,
    /// 发现的问题列表
    pub issues: Vec<VerificationIssue>,
    /// 改进建议
    pub suggestions: Vec<String>,
    /// 执行的检查类型
    pub checks_performed: Vec<VerificationCheck>,
}

impl VerificationResult {
    /// 创建通过的结果
    pub fn pass(checks: Vec<VerificationCheck>) -> Self {
        Self {
            passed: true,
            issues: Vec::new(),
            suggestions: Vec::new(),
            checks_performed: checks,
        }
    }

    /// 从问题列表构建结果
    ///
    /// 如果存在 Error 级别问题，标记为未通过
    pub fn from_issues(
        issues: Vec<VerificationIssue>,
        suggestions: Vec<String>,
        checks: Vec<VerificationCheck>,
    ) -> Self {
        let passed = !issues.iter().any(|i| i.severity == Severity::Error);
        Self {
            passed,
            issues,
            suggestions,
            checks_performed: checks,
        }
    }

    /// Error 级别问题数量
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }

    /// Warning 级别问题数量
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }

    /// 生成文本报告
    pub fn report(&self) -> String {
        let mut lines = Vec::new();

        if self.passed {
            lines.push("✅ 验证通过".to_string());
        } else {
            lines.push(format!(
                "❌ 验证失败 ({} 个错误, {} 个警告)",
                self.error_count(),
                self.warning_count()
            ));
        }

        if !self.issues.is_empty() {
            lines.push(String::new());
            lines.push("问题:".to_string());
            for issue in &self.issues {
                let severity_icon = match issue.severity {
                    Severity::Error => "🔴",
                    Severity::Warning => "🟡",
                    Severity::Info => "🔵",
                };
                let file_info = issue
                    .file_path
                    .as_deref()
                    .map(|f| format!(" ({})", f))
                    .unwrap_or_default();
                lines.push(format!(
                    "  {} [{}]{}: {}",
                    severity_icon,
                    issue.check.description(),
                    file_info,
                    issue.message
                ));
            }
        }

        if !self.suggestions.is_empty() {
            lines.push(String::new());
            lines.push("建议:".to_string());
            for s in &self.suggestions {
                lines.push(format!("  💡 {}", s));
            }
        }

        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// 验证 Agent 配置
// ---------------------------------------------------------------------------

/// 验证 Agent 配置
#[derive(Clone, Debug)]
pub struct VerificationConfig {
    /// 要执行的检查列表
    pub checks: Vec<VerificationCheck>,
    /// 变更描述
    pub change_description: String,
    /// 设计文档引用
    pub design_doc_ref: Option<String>,
    /// 变更的文件列表
    pub changed_files: Vec<String>,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            checks: VerificationCheck::all(),
            change_description: String::new(),
            design_doc_ref: None,
            changed_files: Vec::new(),
        }
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_check_types() {
        let all = VerificationCheck::all();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_check_descriptions() {
        assert!(!VerificationCheck::DesignCompliance.description().is_empty());
        assert!(!VerificationCheck::BreakingChanges.description().is_empty());
        assert!(!VerificationCheck::DesignDefects.description().is_empty());
        assert!(!VerificationCheck::TestEffectiveness
            .description()
            .is_empty());
        assert!(!VerificationCheck::MeaninglessTests.description().is_empty());
    }

    #[test]
    fn test_pass_result() {
        let result = VerificationResult::pass(VerificationCheck::all());
        assert!(result.passed);
        assert!(result.issues.is_empty());
        assert_eq!(result.checks_performed.len(), 5);
    }

    #[test]
    fn test_fail_with_errors() {
        let issues = vec![VerificationIssue {
            check: VerificationCheck::DesignDefects,
            severity: Severity::Error,
            message: "缺少错误处理".into(),
            file_path: Some("src/main.rs".into()),
        }];
        let result = VerificationResult::from_issues(
            issues,
            vec!["添加 Result 返回值".into()],
            VerificationCheck::all(),
        );
        assert!(!result.passed);
        assert_eq!(result.error_count(), 1);
    }

    #[test]
    fn test_pass_with_warnings_only() {
        let issues = vec![VerificationIssue {
            check: VerificationCheck::TestEffectiveness,
            severity: Severity::Warning,
            message: "测试覆盖率偏低".into(),
            file_path: None,
        }];
        let result = VerificationResult::from_issues(issues, vec![], VerificationCheck::all());
        assert!(result.passed); // Warning 不算失败
        assert_eq!(result.warning_count(), 1);
    }

    #[test]
    fn test_report_pass() {
        let result = VerificationResult::pass(vec![]);
        let report = result.report();
        assert!(report.contains("✅"));
    }

    #[test]
    fn test_report_fail() {
        let issues = vec![VerificationIssue {
            check: VerificationCheck::BreakingChanges,
            severity: Severity::Error,
            message: "破坏了 API".into(),
            file_path: Some("lib.rs".into()),
        }];
        let result = VerificationResult::from_issues(issues, vec!["回滚修改".into()], vec![]);
        let report = result.report();
        assert!(report.contains("❌"));
        assert!(report.contains("破坏了 API"));
        assert!(report.contains("lib.rs"));
        assert!(report.contains("回滚修改"));
    }

    #[test]
    fn test_severity_icons_in_report() {
        let issues = vec![
            VerificationIssue {
                check: VerificationCheck::DesignDefects,
                severity: Severity::Error,
                message: "错误".into(),
                file_path: None,
            },
            VerificationIssue {
                check: VerificationCheck::TestEffectiveness,
                severity: Severity::Warning,
                message: "警告".into(),
                file_path: None,
            },
            VerificationIssue {
                check: VerificationCheck::MeaninglessTests,
                severity: Severity::Info,
                message: "信息".into(),
                file_path: None,
            },
        ];
        let result = VerificationResult::from_issues(issues, vec![], vec![]);
        let report = result.report();
        assert!(report.contains("🔴"));
        assert!(report.contains("🟡"));
        assert!(report.contains("🔵"));
    }

    #[test]
    fn test_default_config() {
        let config = VerificationConfig::default();
        assert_eq!(config.checks.len(), 5);
        assert!(config.change_description.is_empty());
        assert!(config.design_doc_ref.is_none());
    }

    #[test]
    fn test_mixed_severity_counts() {
        let issues = vec![
            VerificationIssue {
                check: VerificationCheck::DesignDefects,
                severity: Severity::Error,
                message: "e1".into(),
                file_path: None,
            },
            VerificationIssue {
                check: VerificationCheck::DesignDefects,
                severity: Severity::Error,
                message: "e2".into(),
                file_path: None,
            },
            VerificationIssue {
                check: VerificationCheck::TestEffectiveness,
                severity: Severity::Warning,
                message: "w1".into(),
                file_path: None,
            },
        ];
        let result = VerificationResult::from_issues(issues, vec![], vec![]);
        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warning_count(), 1);
    }
}
