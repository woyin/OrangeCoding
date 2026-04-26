use crate::harness::drift::classify_outcome;
use crate::harness::types::{HarnessAction, MissionContract, StepOutcome};

#[derive(Debug, Clone)]
pub struct HarnessSupervisor {
    contract: MissionContract,
    accepted_checkpoints: Vec<StepOutcome>,
}

impl HarnessSupervisor {
    pub fn new(contract: MissionContract) -> Self {
        Self {
            contract,
            accepted_checkpoints: Vec::new(),
        }
    }

    pub fn evaluate_checkpoint(&mut self, outcome: &StepOutcome) -> HarnessAction {
        let action = classify_outcome(&self.contract, outcome);
        if matches!(action, HarnessAction::Continue) {
            self.accepted_checkpoints.push(outcome.clone());
        }
        action
    }

    pub fn accepted_checkpoints(&self) -> &[StepOutcome] {
        &self.accepted_checkpoints
    }

    pub fn contract(&self) -> &MissionContract {
        &self.contract
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::types::{HarnessConfig, MissionContract, ReviewGatePolicy, StepOutcome};
    use crate::harness::HarnessAction;

    fn contract() -> MissionContract {
        let mut contract = MissionContract::new(
            "实现 Harness 对齐层".into(),
            vec!["保持主目标".into()],
            ReviewGatePolicy::MajorPlanChange,
            HarnessConfig::default(),
        );
        contract.forbidden_detours = vec!["无关".into(), "先解决 UI 再回来".into()];
        contract
    }

    #[test]
    fn 重大计划变更时进入受控重规划() {
        let mut supervisor = HarnessSupervisor::new(contract());
        let outcome = StepOutcome {
            summary: "需要把目标从 harness 改为全量 workflow 重写".into(),
            touched_files: vec![],
            decisions: vec!["放弃渐进接入".into()],
            rationale: "当前方案需要完全改写".into(),
            blockers: vec![],
            proposed_plan_change: Some("将首版范围扩大到重写所有 workflow".into()),
        };

        let decision = supervisor.evaluate_checkpoint(&outcome);
        assert!(matches!(decision, HarnessAction::Replan { .. }));
    }

    #[test]
    fn 仅在继续时接受检查点() {
        let mut supervisor = HarnessSupervisor::new(contract());
        let accepted = StepOutcome {
            summary: "完成了计划中的 Harness 对齐检查点".into(),
            touched_files: vec!["crates/orangecoding-agent/src/harness/supervisor.rs".into()],
            decisions: vec!["保持主目标".into()],
            rationale: "没有偏航".into(),
            blockers: vec![],
            proposed_plan_change: None,
        };
        let rejected = StepOutcome {
            summary: "顺手去修一个无关 UI bug".into(),
            touched_files: vec!["crates/orangecoding-tui/src/app.rs".into()],
            decisions: vec!["先解决 UI 再回来".into()],
            rationale: "这个问题也挺重要".into(),
            blockers: vec![],
            proposed_plan_change: None,
        };

        assert!(matches!(
            supervisor.evaluate_checkpoint(&accepted),
            HarnessAction::Continue
        ));
        assert_eq!(supervisor.accepted_checkpoints().len(), 1);

        assert!(matches!(
            supervisor.evaluate_checkpoint(&rejected),
            HarnessAction::Escalate { .. }
        ));
        assert_eq!(supervisor.accepted_checkpoints().len(), 1);
    }
}
