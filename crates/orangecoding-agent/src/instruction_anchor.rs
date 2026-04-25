/// 定期把代理拉回原始用户指令的锚点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionAnchor {
    original_instruction: String,
    interval: u32,
    step_counter: u32,
}

impl InstructionAnchor {
    /// 创建新的指令回锚器。
    pub fn new(instruction: &str, interval: u32) -> Self {
        Self {
            original_instruction: instruction.trim().to_string(),
            interval,
            step_counter: 0,
        }
    }

    /// 记录一步执行，并在到达间隔时返回系统回锚消息。
    pub fn on_step(&mut self) -> Option<String> {
        if self.interval == 0 || self.original_instruction.is_empty() {
            return None;
        }

        self.step_counter = self.step_counter.saturating_add(1);
        if self.step_counter % self.interval != 0 {
            return None;
        }

        Some(format!(
            "[指令回锚]\n原始用户指令：{}\n请静默检查当前执行是否偏离原始指令；如有漂移，立即纠正并继续执行，不要向用户说明此回锚过程。",
            self.original_instruction
        ))
    }

    /// 替换原始指令并重置步数。
    pub fn reset(&mut self, new_instruction: &str) {
        self.original_instruction = new_instruction.trim().to_string();
        self.step_counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::InstructionAnchor;

    #[test]
    fn 测试按间隔触发回锚消息() {
        let mut anchor = InstructionAnchor::new("  保持专注完成任务  ", 3);

        assert_eq!(anchor.on_step(), None);
        assert_eq!(anchor.on_step(), None);
        let message = anchor.on_step().expect("第三步应触发回锚消息");

        assert!(message.contains("[指令回锚]"));
        assert!(message.contains("保持专注完成任务"));
    }

    #[test]
    fn 测试零间隔不会触发回锚() {
        let mut anchor = InstructionAnchor::new("保持专注完成任务", 0);

        for _ in 0..5 {
            assert_eq!(anchor.on_step(), None);
        }
    }

    #[test]
    fn 测试重置会替换原始指令并清零计数() {
        let mut anchor = InstructionAnchor::new("旧指令", 2);
        assert_eq!(anchor.on_step(), None);

        anchor.reset("新指令");

        assert_eq!(anchor.on_step(), None);
        let message = anchor.on_step().expect("重置后第二步应触发回锚消息");

        assert!(message.contains("新指令"));
        assert!(!message.contains("旧指令"));
        assert!(message.contains("[指令回锚]"));
    }
}
