use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewGatePolicy {
    Never,
    MajorPlanChange,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub checkpoint_interval: u32,
    pub max_segment_steps: u32,
    pub major_plan_change_threshold: u32,
    pub drift_sensitivity: u8,
    pub escalate_on_low_confidence: bool,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            checkpoint_interval: 1,
            max_segment_steps: 5,
            major_plan_change_threshold: 1,
            drift_sensitivity: 2,
            escalate_on_low_confidence: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionContract {
    pub objective: String,
    pub success_criteria: Vec<String>,
    pub accepted_plan_summary: String,
    pub forbidden_detours: Vec<String>,
    pub review_gate_policy: ReviewGatePolicy,
    pub harness_config: HarnessConfig,
}

impl MissionContract {
    pub fn new(
        objective: String,
        success_criteria: Vec<String>,
        review_gate_policy: ReviewGatePolicy,
        harness_config: HarnessConfig,
    ) -> Self {
        Self {
            objective,
            success_criteria,
            accepted_plan_summary: String::new(),
            forbidden_detours: Vec::new(),
            review_gate_policy,
            harness_config,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepOutcome {
    pub summary: String,
    pub touched_files: Vec<String>,
    pub decisions: Vec<String>,
    pub rationale: String,
    pub blockers: Vec<String>,
    pub proposed_plan_change: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessAction {
    Continue,
    Replan { reason: String },
    Escalate { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 任务契约可往返序列化() {
        let contract = MissionContract::new(
            "实现 Harness 对齐层".into(),
            vec!["出现检查点".into(), "重大变更触发门控".into()],
            ReviewGatePolicy::MajorPlanChange,
            HarnessConfig::default(),
        );

        let json = serde_json::to_string_pretty(&contract).unwrap();
        let roundtrip: MissionContract = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip.objective, "实现 Harness 对齐层");
        assert_eq!(roundtrip.success_criteria.len(), 2);
        assert_eq!(roundtrip.review_gate_policy, ReviewGatePolicy::MajorPlanChange);
    }

    #[test]
    fn 步骤结果保留对齐证据() {
        let outcome = StepOutcome {
            summary: "更新了 autopilot 状态机".into(),
            touched_files: vec!["crates/orangecoding-agent/src/workflows/autopilot.rs".into()],
            decisions: vec!["引入有界执行段".into()],
            rationale: "为了在每段结束后执行检查点".into(),
            blockers: vec![],
            proposed_plan_change: None,
        };

        assert!(outcome.rationale.contains("检查点"));
        assert_eq!(outcome.touched_files.len(), 1);
    }
}
