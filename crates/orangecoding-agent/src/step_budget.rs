use std::collections::VecDeque;

/// 步骤预算检查结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetDecision {
    /// 继续执行下一步。
    Continue,
    /// 检测到必须停止的情况。
    HardStop { reason: String },
}

/// 追踪执行步数预算并检测重复动作循环。
#[derive(Debug, Clone)]
pub struct StepBudgetGuard {
    budget: u32,
    current: u32,
    loop_threshold: usize,
    recent_signatures: VecDeque<String>,
}

impl StepBudgetGuard {
    /// 创建步骤预算守卫，预算至少为 1，重复阈值至少为 2。
    pub fn new(budget: u32, loop_threshold: usize) -> Self {
        Self {
            budget: budget.max(1),
            current: 0,
            loop_threshold: loop_threshold.max(2),
            recent_signatures: VecDeque::new(),
        }
    }

    /// 记录一次动作并返回预算决策。
    pub fn tick(&mut self, action_signature: &str) -> BudgetDecision {
        self.current = self.current.saturating_add(1);
        self.recent_signatures
            .push_back(action_signature.to_string());

        while self.recent_signatures.len() > self.loop_threshold {
            self.recent_signatures.pop_front();
        }

        if self.recent_signatures.len() == self.loop_threshold
            && self
                .recent_signatures
                .iter()
                .all(|signature| signature == action_signature)
        {
            return BudgetDecision::HardStop {
                reason: format!("检测到连续重复动作：{action_signature}"),
            };
        }

        if self.current > self.budget {
            let extension = (self.budget.saturating_add(1) / 2).max(1);
            self.budget = self.budget.saturating_add(extension);
        }

        BudgetDecision::Continue
    }

    /// 当前已记录步数。
    pub fn current(&self) -> u32 {
        self.current
    }

    /// 当前允许预算。
    pub fn budget(&self) -> u32 {
        self.budget
    }
}

#[cfg(test)]
mod tests {
    use super::{BudgetDecision, StepBudgetGuard};

    #[test]
    fn 测试预算耗尽时自动扩展而不是停止() {
        let mut guard = StepBudgetGuard::new(2, 3);

        assert_eq!(guard.tick("read:file_a"), BudgetDecision::Continue);
        assert_eq!(guard.tick("write:file_b"), BudgetDecision::Continue);
        assert_eq!(guard.tick("run:test_c"), BudgetDecision::Continue);
        assert_eq!(guard.current(), 3);
        assert_eq!(guard.budget(), 3);
    }

    #[test]
    fn 测试奇数预算按半数向上扩展() {
        let mut guard = StepBudgetGuard::new(3, 4);

        assert_eq!(guard.tick("a"), BudgetDecision::Continue);
        assert_eq!(guard.tick("b"), BudgetDecision::Continue);
        assert_eq!(guard.tick("c"), BudgetDecision::Continue);
        assert_eq!(guard.tick("d"), BudgetDecision::Continue);
        assert_eq!(guard.budget(), 5);
    }

    #[test]
    fn 测试重复动作达到阈值时硬停止() {
        let mut guard = StepBudgetGuard::new(10, 3);

        assert_eq!(guard.tick("same-action"), BudgetDecision::Continue);
        assert_eq!(guard.tick("same-action"), BudgetDecision::Continue);
        let decision = guard.tick("same-action");

        match decision {
            BudgetDecision::HardStop { reason } => assert!(reason.contains("重复")),
            other => panic!("expected hard stop, got {other:?}"),
        }
    }

    #[test]
    fn 测试不同动作会重置重复检测() {
        let mut guard = StepBudgetGuard::new(10, 3);

        assert_eq!(guard.tick("a"), BudgetDecision::Continue);
        assert_eq!(guard.tick("a"), BudgetDecision::Continue);
        assert_eq!(guard.tick("b"), BudgetDecision::Continue);
        assert_eq!(guard.tick("a"), BudgetDecision::Continue);
    }
}
