//! Self-Healing: 自我修复模块
//!
//! 检测不变量违规并生成修复建议。

use crate::report::Violation;
use crate::rules::Severity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 修复建议的类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixType {
    /// 配置变更（如启用认证）
    ConfigChange,
    /// 代码修改
    CodeFix,
    /// 添加缺失的测试
    AddTest,
    /// 策略调整（如权限规则）
    PolicyAdjust,
    /// 需要人工干预
    ManualIntervention,
}

/// 修复建议的优先级
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FixPriority {
    /// 必须立即修复
    Immediate,
    /// 尽快修复
    Soon,
    /// 可以稍后修复
    Later,
}

/// 单条修复建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSuggestion {
    /// 建议 ID
    pub id: String,
    /// 关联的违规规则 ID
    pub violation_rule_id: String,
    /// 修复类型
    pub fix_type: FixType,
    /// 优先级
    pub priority: FixPriority,
    /// 修复描述
    pub description: String,
    /// 建议的具体操作步骤
    pub steps: Vec<String>,
    /// 预期效果
    pub expected_outcome: String,
}

/// 修复状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealingStatus {
    /// 已检测到问题
    Detected,
    /// 已生成修复建议
    Suggested,
    /// 修复中
    InProgress,
    /// 修复完成，待验证
    PendingVerification,
    /// 验证通过
    Healed,
    /// 修复失败
    Failed(String),
}

/// 修复工作项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingTask {
    /// 任务 ID
    pub id: String,
    /// 关联的违规
    pub violation: Violation,
    /// 修复建议
    pub suggestion: Option<FixSuggestion>,
    /// 当前状态
    pub status: HealingStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
}

/// 根据违规生成对应的修复建议
fn suggestion_for_violation(violation: &Violation) -> FixSuggestion {
    let suggestion_id = format!("fix-{}", violation.rule_id.to_lowercase());

    match violation.rule_id.as_str() {
        "INV-AUTH-01" | "INV-AUTH-02" => FixSuggestion {
            id: suggestion_id,
            violation_rule_id: violation.rule_id.clone(),
            fix_type: FixType::ConfigChange,
            priority: FixPriority::Immediate,
            description: "启用认证中间件".into(),
            steps: vec![
                "检查 auth 配置".into(),
                "启用 token 验证".into(),
                "重启服务".into(),
            ],
            expected_outcome: "所有请求需通过认证".into(),
        },
        "INV-AUTH-03" => FixSuggestion {
            id: suggestion_id,
            violation_rule_id: violation.rule_id.clone(),
            fix_type: FixType::CodeFix,
            priority: FixPriority::Immediate,
            description: "移除日志中的 token 明文".into(),
            steps: vec![
                "审查日志输出".into(),
                "使用 SecretObfuscator".into(),
                "添加测试验证".into(),
            ],
            expected_outcome: "日志中不再包含明文 token".into(),
        },
        "INV-TOOL-01" | "INV-TOOL-02" | "INV-TOOL-03" => FixSuggestion {
            id: suggestion_id,
            violation_rule_id: violation.rule_id.clone(),
            fix_type: FixType::PolicyAdjust,
            priority: FixPriority::Immediate,
            description: "强制执行工具权限检查".into(),
            steps: vec![
                "检查 Tool trait 实现".into(),
                "确保 check_permissions 在 execute 前调用".into(),
            ],
            expected_outcome: "所有工具执行前完成权限检查".into(),
        },
        "INV-CANCEL-01" | "INV-CANCEL-02" => FixSuggestion {
            id: suggestion_id,
            violation_rule_id: violation.rule_id.clone(),
            fix_type: FixType::CodeFix,
            priority: FixPriority::Soon,
            description: "修复取消信号传播".into(),
            steps: vec![
                "检查 CancellationToken 实现".into(),
                "验证父子传播链".into(),
            ],
            expected_outcome: "取消信号正确向下传播".into(),
        },
        "INV-AUDIT-02" => FixSuggestion {
            id: suggestion_id,
            violation_rule_id: violation.rule_id.clone(),
            fix_type: FixType::CodeFix,
            priority: FixPriority::Soon,
            description: "修复审计链哈希连续性".into(),
            steps: vec![
                "检查 HashChain 实现".into(),
                "验证 previous_hash 传递".into(),
            ],
            expected_outcome: "审计链哈希连续无断裂".into(),
        },
        _ => FixSuggestion {
            id: suggestion_id,
            violation_rule_id: violation.rule_id.clone(),
            fix_type: FixType::ManualIntervention,
            priority: priority_from_severity(violation.severity),
            description: format!("需要人工检查: {}", violation.rule_name),
            steps: vec![
                "查看违规详情".into(),
                "评估影响范围".into(),
                "制定修复方案".into(),
            ],
            expected_outcome: "问题得到人工评估和处理".into(),
        },
    }
}

/// 从严重性映射到修复优先级
fn priority_from_severity(severity: Severity) -> FixPriority {
    match severity {
        Severity::Critical => FixPriority::Immediate,
        Severity::High => FixPriority::Soon,
        Severity::Medium | Severity::Low => FixPriority::Later,
    }
}

/// 自我修复引擎
pub struct SelfHealer {
    /// 修复任务队列
    tasks: Vec<HealingTask>,
}

impl SelfHealer {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    /// 从违规报告中检测问题并生成修复任务
    pub fn detect_from_report(&mut self, report: &crate::report::ViolationReport) -> usize {
        let mut count = 0;
        for violation in &report.violations {
            let now = Utc::now();
            let task = HealingTask {
                id: format!("heal-{}", violation.rule_id.to_lowercase()),
                violation: violation.clone(),
                suggestion: None,
                status: HealingStatus::Detected,
                created_at: now,
                updated_at: now,
            };
            self.tasks.push(task);
            count += 1;
        }
        count
    }

    /// 为指定任务生成修复建议
    pub fn generate_suggestion(&mut self, task_id: &str) -> Option<&FixSuggestion> {
        let task = self.tasks.iter_mut().find(|t| t.id == task_id)?;
        if task.status != HealingStatus::Detected {
            return None;
        }
        let suggestion = suggestion_for_violation(&task.violation);
        task.suggestion = Some(suggestion);
        task.status = HealingStatus::Suggested;
        task.updated_at = Utc::now();
        task.suggestion.as_ref()
    }

    /// 为所有 Detected 状态的任务生成建议
    pub fn generate_all_suggestions(&mut self) -> usize {
        // 先收集需要处理的索引，避免借用冲突
        let indices: Vec<usize> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.status == HealingStatus::Detected)
            .map(|(i, _)| i)
            .collect();

        let count = indices.len();
        for i in indices {
            let suggestion = suggestion_for_violation(&self.tasks[i].violation);
            self.tasks[i].suggestion = Some(suggestion);
            self.tasks[i].status = HealingStatus::Suggested;
            self.tasks[i].updated_at = Utc::now();
        }
        count
    }

    /// 将任务标记为修复中
    pub fn start_healing(&mut self, task_id: &str) -> bool {
        self.transition(
            task_id,
            |s| s == &HealingStatus::Suggested,
            HealingStatus::InProgress,
        )
    }

    /// 将任务标记为待验证
    pub fn mark_pending_verification(&mut self, task_id: &str) -> bool {
        self.transition(
            task_id,
            |s| s == &HealingStatus::InProgress,
            HealingStatus::PendingVerification,
        )
    }

    /// 将任务标记为已修复
    pub fn mark_healed(&mut self, task_id: &str) -> bool {
        self.transition(
            task_id,
            |s| s == &HealingStatus::PendingVerification,
            HealingStatus::Healed,
        )
    }

    /// 将任务标记为失败（仅允许从非终态转换）
    pub fn mark_failed(&mut self, task_id: &str, reason: &str) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            // Prevent transition from terminal states
            if matches!(
                task.status,
                HealingStatus::Healed | HealingStatus::Failed(_)
            ) {
                return false;
            }
            task.status = HealingStatus::Failed(reason.to_string());
            task.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// 获取所有任务
    pub fn tasks(&self) -> &[HealingTask] {
        &self.tasks
    }

    /// 获取按优先级排序的待处理任务（Detected 或 Suggested 状态）
    pub fn pending_tasks(&self) -> Vec<&HealingTask> {
        let mut pending: Vec<&HealingTask> = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, HealingStatus::Detected | HealingStatus::Suggested))
            .collect();

        pending.sort_by(|a, b| {
            let pa = a
                .suggestion
                .as_ref()
                .map(|s| &s.priority)
                .cloned()
                .unwrap_or_else(|| priority_from_severity(a.violation.severity));
            let pb = b
                .suggestion
                .as_ref()
                .map(|s| &s.priority)
                .cloned()
                .unwrap_or_else(|| priority_from_severity(b.violation.severity));
            pa.cmp(&pb)
        });

        pending
    }

    /// 导出修复建议为 markdown
    pub fn export_suggestions_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# 自我修复建议\n\n");
        md.push_str("## 待处理任务\n\n");

        for task in &self.tasks {
            md.push_str(&format!(
                "### [{}] — {}\n",
                task.id, task.violation.rule_name
            ));
            md.push_str(&format!("- **状态**: {:?}\n", task.status));

            if let Some(ref suggestion) = task.suggestion {
                md.push_str(&format!("- **优先级**: {:?}\n", suggestion.priority));
                md.push_str(&format!("- **修复类型**: {:?}\n", suggestion.fix_type));
                md.push_str(&format!("- **描述**: {}\n", suggestion.description));
                md.push_str("- **步骤**:\n");
                for (i, step) in suggestion.steps.iter().enumerate() {
                    md.push_str(&format!("  {}. {}\n", i + 1, step));
                }
                md.push_str(&format!(
                    "- **预期效果**: {}\n",
                    suggestion.expected_outcome
                ));
            }

            md.push('\n');
        }

        // 统计
        let total = self.tasks.len();
        let healed = self
            .tasks
            .iter()
            .filter(|t| t.status == HealingStatus::Healed)
            .count();
        let pending = self
            .tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    HealingStatus::Detected
                        | HealingStatus::Suggested
                        | HealingStatus::InProgress
                        | HealingStatus::PendingVerification
                )
            })
            .count();
        let failed = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, HealingStatus::Failed(_)))
            .count();

        md.push_str("## 统计\n\n");
        md.push_str(&format!("- 总任务数: {}\n", total));
        md.push_str(&format!("- 已修复: {}\n", healed));
        md.push_str(&format!("- 待处理: {}\n", pending));
        md.push_str(&format!("- 失败: {}\n", failed));

        md
    }

    /// 内部状态转换辅助
    fn transition(
        &mut self,
        task_id: &str,
        guard: impl Fn(&HealingStatus) -> bool,
        next: HealingStatus,
    ) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            if guard(&task.status) {
                task.status = next;
                task.updated_at = Utc::now();
                return true;
            }
        }
        false
    }
}

impl Default for SelfHealer {
    fn default() -> Self {
        Self::new()
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

    fn report_with_violations() -> ViolationReport {
        ViolationReport {
            total_rules: 18,
            passed: 15,
            failed: 3,
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
                    severity: Severity::Critical,
                    message: "工具权限检查未强制执行".into(),
                    timestamp: Utc::now(),
                },
                Violation {
                    rule_id: "INV-CANCEL-01".into(),
                    rule_name: "取消信号必须向下传播".into(),
                    severity: Severity::High,
                    message: "取消信号未传播".into(),
                    timestamp: Utc::now(),
                },
            ],
            checked_at: Utc::now(),
            has_critical: true,
        }
    }

    #[test]
    fn new_healer_empty() {
        let healer = SelfHealer::new();
        assert!(healer.tasks().is_empty());
    }

    #[test]
    fn detect_from_clean_report() {
        let mut healer = SelfHealer::new();
        let count = healer.detect_from_report(&clean_report());
        assert_eq!(count, 0);
        assert!(healer.tasks().is_empty());
    }

    #[test]
    fn detect_from_violations() {
        let mut healer = SelfHealer::new();
        let count = healer.detect_from_report(&report_with_violations());
        assert_eq!(count, 3);
        assert_eq!(healer.tasks().len(), 3);
        assert!(healer
            .tasks()
            .iter()
            .all(|t| t.status == HealingStatus::Detected));
    }

    #[test]
    fn generate_suggestion_auth() {
        let mut healer = SelfHealer::new();
        healer.detect_from_report(&report_with_violations());

        let suggestion = healer.generate_suggestion("heal-inv-auth-01");
        assert!(suggestion.is_some());
        let s = suggestion.unwrap();
        assert_eq!(s.fix_type, FixType::ConfigChange);
        assert_eq!(s.priority, FixPriority::Immediate);
        assert!(s.description.contains("认证中间件"));
    }

    #[test]
    fn generate_all_suggestions() {
        let mut healer = SelfHealer::new();
        healer.detect_from_report(&report_with_violations());
        let count = healer.generate_all_suggestions();
        assert_eq!(count, 3);
        assert!(healer
            .tasks()
            .iter()
            .all(|t| t.status == HealingStatus::Suggested));
        assert!(healer.tasks().iter().all(|t| t.suggestion.is_some()));
    }

    #[test]
    fn healing_lifecycle() {
        let mut healer = SelfHealer::new();
        healer.detect_from_report(&report_with_violations());

        let task_id = "heal-inv-auth-01";

        // Detected → Suggested
        healer.generate_suggestion(task_id);
        assert_eq!(
            healer
                .tasks()
                .iter()
                .find(|t| t.id == task_id)
                .unwrap()
                .status,
            HealingStatus::Suggested
        );

        // Suggested → InProgress
        assert!(healer.start_healing(task_id));
        assert_eq!(
            healer
                .tasks()
                .iter()
                .find(|t| t.id == task_id)
                .unwrap()
                .status,
            HealingStatus::InProgress
        );

        // InProgress → PendingVerification
        assert!(healer.mark_pending_verification(task_id));
        assert_eq!(
            healer
                .tasks()
                .iter()
                .find(|t| t.id == task_id)
                .unwrap()
                .status,
            HealingStatus::PendingVerification
        );

        // PendingVerification → Healed
        assert!(healer.mark_healed(task_id));
        assert_eq!(
            healer
                .tasks()
                .iter()
                .find(|t| t.id == task_id)
                .unwrap()
                .status,
            HealingStatus::Healed
        );
    }

    #[test]
    fn healing_failure() {
        let mut healer = SelfHealer::new();
        healer.detect_from_report(&report_with_violations());

        let task_id = "heal-inv-auth-01";
        assert!(healer.mark_failed(task_id, "无法自动修复"));
        assert_eq!(
            healer
                .tasks()
                .iter()
                .find(|t| t.id == task_id)
                .unwrap()
                .status,
            HealingStatus::Failed("无法自动修复".into())
        );
    }

    #[test]
    fn pending_tasks_sorted() {
        let mut healer = SelfHealer::new();
        healer.detect_from_report(&report_with_violations());
        healer.generate_all_suggestions();

        let pending = healer.pending_tasks();
        assert_eq!(pending.len(), 3);

        // Immediate 优先级排在前面
        let priorities: Vec<_> = pending
            .iter()
            .map(|t| t.suggestion.as_ref().unwrap().priority.clone())
            .collect();
        assert_eq!(priorities[0], FixPriority::Immediate);
        assert_eq!(priorities[1], FixPriority::Immediate);
        assert_eq!(priorities[2], FixPriority::Soon);
    }

    #[test]
    fn export_markdown() {
        let mut healer = SelfHealer::new();
        healer.detect_from_report(&report_with_violations());
        healer.generate_all_suggestions();

        let md = healer.export_suggestions_markdown();

        assert!(md.contains("# 自我修复建议"));
        assert!(md.contains("## 待处理任务"));
        assert!(md.contains("heal-inv-auth-01"));
        assert!(md.contains("- **状态**:"));
        assert!(md.contains("- **优先级**:"));
        assert!(md.contains("- **修复类型**:"));
        assert!(md.contains("- **描述**:"));
        assert!(md.contains("- **步骤**:"));
        assert!(md.contains("- **预期效果**:"));
        assert!(md.contains("## 统计"));
        assert!(md.contains("- 总任务数: 3"));
        assert!(md.contains("- 待处理: 3"));
    }
}
