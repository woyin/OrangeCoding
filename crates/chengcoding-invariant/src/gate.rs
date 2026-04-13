//! Pre-Check Gate: 提交前不变量影响分析
//!
//! 分析 git diff，判断变更是否影响不变量，高风险时阻止提交。

use crate::rules::{system_invariants, InvariantCategory, InvariantRule, Severity};
use serde::{Deserialize, Serialize};

/// 门控决策结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateDecision {
    /// 允许提交
    Allow,
    /// 警告但允许提交
    Warn,
    /// 阻止提交
    Block,
}

/// 文件变更影响分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileImpact {
    /// 变更文件路径
    pub file_path: String,
    /// 受影响的不变量类别
    pub affected_categories: Vec<InvariantCategory>,
    /// 变更行数
    pub lines_changed: usize,
}

/// 门控检查报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateReport {
    /// 门控决策
    pub decision: GateDecision,
    /// 受影响的文件
    pub impacts: Vec<FileImpact>,
    /// 受影响的不变量规则
    pub affected_rules: Vec<String>,
    /// 最高严重性
    pub max_severity: Option<Severity>,
    /// 决策原因
    pub reason: String,
}

/// 预提交门控检查器
pub struct PreCheckGate {
    rules: Vec<InvariantRule>,
}

impl PreCheckGate {
    pub fn new(rules: Vec<InvariantRule>) -> Self {
        Self { rules }
    }

    pub fn with_system_rules() -> Self {
        Self::new(system_invariants())
    }

    /// 分析 diff 文本，返回门控报告
    pub fn analyze_diff(&self, diff: &str) -> GateReport {
        let changed_files = Self::parse_changed_files(diff);

        let mut impacts = Vec::new();
        let mut all_categories = Vec::new();

        for (path, lines_changed) in changed_files {
            let categories = Self::map_file_to_categories(&path);
            all_categories.extend_from_slice(&categories);
            impacts.push(FileImpact {
                file_path: path,
                affected_categories: categories,
                lines_changed,
            });
        }

        let affected_rules = self.find_affected_rules(&all_categories);
        let affected_rule_ids: Vec<String> = affected_rules.iter().map(|r| r.id.clone()).collect();

        // 确定最高严重性
        let max_severity = affected_rules
            .iter()
            .map(|r| r.severity)
            .min_by_key(|s| match s {
                Severity::Critical => 0,
                Severity::High => 1,
                Severity::Medium => 2,
                Severity::Low => 3,
            });

        let decision = Self::decide(max_severity);

        let reason = match &decision {
            GateDecision::Block => format!(
                "变更影响 Critical 级别不变量: {}",
                affected_rule_ids.join(", ")
            ),
            GateDecision::Warn => {
                format!("变更影响 High 级别不变量: {}", affected_rule_ids.join(", "))
            }
            GateDecision::Allow => "变更未影响高风险不变量".to_string(),
        };

        GateReport {
            decision,
            impacts,
            affected_rules: affected_rule_ids,
            max_severity,
            reason,
        }
    }

    /// 从 diff 提取变更文件列表（路径 + 变更行数）
    fn parse_changed_files(diff: &str) -> Vec<(String, usize)> {
        let mut result: Vec<(String, usize)> = Vec::new();
        let mut current_file: Option<String> = None;
        let mut current_lines: usize = 0;

        for line in diff.lines() {
            if let Some(rest) = line.strip_prefix("diff --git ") {
                // 保存上一个文件的结果
                if let Some(path) = current_file.take() {
                    result.push((path, current_lines));
                }
                // 解析 "a/path b/path" 格式，取 b/ 路径
                if let Some(b_part) = rest.split(" b/").last() {
                    current_file = Some(b_part.to_string());
                }
                current_lines = 0;
            } else if current_file.is_some() {
                // 统计变更行：以 + 或 - 开头，但排除 +++ 和 ---
                if (line.starts_with('+') && !line.starts_with("+++"))
                    || (line.starts_with('-') && !line.starts_with("---"))
                {
                    current_lines += 1;
                }
            }
        }

        // 最后一个文件
        if let Some(path) = current_file {
            result.push((path, current_lines));
        }

        result
    }

    /// 将文件路径映射到不变量类别
    fn map_file_to_categories(path: &str) -> Vec<InvariantCategory> {
        let lower = path.to_lowercase();
        let mut categories = Vec::new();

        // 按关键词匹配文件路径到不变量类别
        if lower.contains("auth") || lower.contains("token") || lower.contains("credential") {
            categories.push(InvariantCategory::Auth);
        }
        if lower.contains("cancel") {
            categories.push(InvariantCategory::Cancellation);
        }
        if lower.contains("session") {
            categories.push(InvariantCategory::Session);
        }
        if lower.contains("tool") || lower.contains("permission") {
            categories.push(InvariantCategory::ToolPermission);
        }
        if lower.contains("context") || lower.contains("compact") {
            categories.push(InvariantCategory::Context);
        }
        if lower.contains("audit") || lower.contains("log") {
            categories.push(InvariantCategory::Audit);
        }
        if lower.contains("approval") || lower.contains("approve") {
            categories.push(InvariantCategory::Approval);
        }
        if lower.contains("event") || lower.contains("stream") {
            categories.push(InvariantCategory::Event);
        }

        categories
    }

    /// 查找受影响的不变量规则
    fn find_affected_rules(&self, categories: &[InvariantCategory]) -> Vec<&InvariantRule> {
        self.rules
            .iter()
            .filter(|rule| categories.contains(&rule.category))
            .collect()
    }

    /// 根据最高严重性决定门控动作
    fn decide(max_severity: Option<Severity>) -> GateDecision {
        match max_severity {
            Some(Severity::Critical) => GateDecision::Block,
            Some(Severity::High) => GateDecision::Warn,
            _ => GateDecision::Allow,
        }
    }
}

impl GateReport {
    /// 生成人类可读的 Markdown 报告
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# 预提交门控报告\n\n");

        // 决策状态
        let icon = match self.decision {
            GateDecision::Allow => "✅ Allow",
            GateDecision::Warn => "⚠️ Warn",
            GateDecision::Block => "🚫 Block",
        };
        md.push_str(&format!("**决策**: {}\n\n", icon));
        md.push_str(&format!("**原因**: {}\n\n", self.reason));

        if let Some(ref sev) = self.max_severity {
            md.push_str(&format!("**最高严重性**: {:?}\n\n", sev));
        }

        // 文件影响列表
        if !self.impacts.is_empty() {
            md.push_str("## 受影响文件\n\n");
            md.push_str("| 文件 | 变更行数 | 影响类别 |\n");
            md.push_str("|------|----------|----------|\n");
            for impact in &self.impacts {
                let cats: Vec<String> = impact
                    .affected_categories
                    .iter()
                    .map(|c| format!("{:?}", c))
                    .collect();
                let cats_str = if cats.is_empty() {
                    "无".to_string()
                } else {
                    cats.join(", ")
                };
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    impact.file_path, impact.lines_changed, cats_str
                ));
            }
            md.push('\n');
        }

        // 受影响规则
        if !self.affected_rules.is_empty() {
            md.push_str("## 受影响的不变量规则\n\n");
            for rule_id in &self.affected_rules {
                md.push_str(&format!("- {}\n", rule_id));
            }
            md.push('\n');
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_diff() {
        let gate = PreCheckGate::with_system_rules();
        let report = gate.analyze_diff("");

        assert_eq!(report.decision, GateDecision::Allow);
        assert!(report.impacts.is_empty());
        assert!(report.affected_rules.is_empty());
        assert!(report.max_severity.is_none());
    }

    #[test]
    fn parse_auth_file_change() {
        let diff = "\
diff --git a/crates/chengcoding-control-server/src/auth.rs b/crates/chengcoding-control-server/src/auth.rs
--- a/crates/chengcoding-control-server/src/auth.rs
+++ b/crates/chengcoding-control-server/src/auth.rs
@@ -10,3 +10,5 @@
+added line 1
+added line 2
";
        let gate = PreCheckGate::with_system_rules();
        let report = gate.analyze_diff(diff);

        // Auth 规则是 Critical → Block
        assert_eq!(report.decision, GateDecision::Block);
        assert_eq!(report.impacts.len(), 1);
        assert_eq!(
            report.impacts[0].file_path,
            "crates/chengcoding-control-server/src/auth.rs"
        );
        assert!(report.impacts[0]
            .affected_categories
            .contains(&InvariantCategory::Auth));
        assert_eq!(report.impacts[0].lines_changed, 2);
        assert_eq!(report.max_severity, Some(Severity::Critical));
    }

    #[test]
    fn parse_session_file_change() {
        let diff = "\
diff --git a/src/session.rs b/src/session.rs
--- a/src/session.rs
+++ b/src/session.rs
@@ -1,3 +1,4 @@
+new session logic
-old session logic
";
        let gate = PreCheckGate::with_system_rules();
        let report = gate.analyze_diff(diff);

        // Session 规则最高是 High → Warn
        assert_eq!(report.decision, GateDecision::Warn);
        assert!(report.impacts[0]
            .affected_categories
            .contains(&InvariantCategory::Session));
        assert_eq!(report.impacts[0].lines_changed, 2);
        assert_eq!(report.max_severity, Some(Severity::High));
    }

    #[test]
    fn parse_unrelated_file_change() {
        let diff = "\
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,2 +1,3 @@
+updated docs
";
        let gate = PreCheckGate::with_system_rules();
        let report = gate.analyze_diff(diff);

        assert_eq!(report.decision, GateDecision::Allow);
        assert_eq!(report.impacts.len(), 1);
        assert!(report.impacts[0].affected_categories.is_empty());
        assert!(report.affected_rules.is_empty());
        assert!(report.max_severity.is_none());
    }

    #[test]
    fn multiple_files_max_severity() {
        // 同时修改 session（High）和 auth（Critical），最终应为 Block
        let diff = "\
diff --git a/src/session.rs b/src/session.rs
--- a/src/session.rs
+++ b/src/session.rs
@@ -1,2 +1,3 @@
+session change
diff --git a/src/auth.rs b/src/auth.rs
--- a/src/auth.rs
+++ b/src/auth.rs
@@ -5,2 +5,4 @@
+auth change 1
+auth change 2
";
        let gate = PreCheckGate::with_system_rules();
        let report = gate.analyze_diff(diff);

        assert_eq!(report.decision, GateDecision::Block);
        assert_eq!(report.impacts.len(), 2);
        assert_eq!(report.max_severity, Some(Severity::Critical));
        // 确认两类规则都被检出
        assert!(report
            .affected_rules
            .iter()
            .any(|id| id.starts_with("INV-AUTH")));
        assert!(report
            .affected_rules
            .iter()
            .any(|id| id.starts_with("INV-SESSION")));
    }

    #[test]
    fn gate_report_markdown() {
        let diff = "\
diff --git a/src/auth.rs b/src/auth.rs
--- a/src/auth.rs
+++ b/src/auth.rs
@@ -1,2 +1,3 @@
+auth change
";
        let gate = PreCheckGate::with_system_rules();
        let report = gate.analyze_diff(diff);
        let md = report.to_markdown();

        assert!(md.contains("# 预提交门控报告"));
        assert!(md.contains("🚫 Block"));
        assert!(md.contains("受影响文件"));
        assert!(md.contains("auth.rs"));
        assert!(md.contains("INV-AUTH"));
        assert!(md.contains("Critical"));
    }
}
