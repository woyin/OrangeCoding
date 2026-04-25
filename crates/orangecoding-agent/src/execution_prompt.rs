/// 代理执行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 直接执行模式。
    Exec,
    /// 计划确认模式。
    Plan,
    /// 自动推进执行模式。
    Autopilot,
    /// UltraWork 模式。
    UltraWork,
}

const SHARED_PROMPT: &str = r#"[MISSION LOCK]
保持对用户原始任务的锁定，所有输出与行动都必须服务于当前任务。

TASK DIFFICULTY SIGNAL: 用户可以显式指定任务难度 easy/medium/hard/epic；如果用户没有指定，则根据任务范围、风险、依赖和验证成本自行推断。"#;

const EXEC_PROMPT: &str = r#"[EXEC MODE - 严格执行]
立即执行用户请求，不要把执行请求改写成计划讨论。
严格按用户字面指令执行；不要擅自扩展范围、替换目标或改变验收标准。
只有遇到真实且不可逆的决策分叉时才询问用户；其他情况根据上下文作出合理选择并继续推进。
持续验证结果，直到请求完成。"#;

const PLAN_PROMPT: &str = r#"[PLAN MODE - 结构化计划]
使用 Plan mode / 结构化计划语言输出方案。
计划必须包含：
- Goal / 目标
- Phases / 阶段
- Steps / 步骤
- Acceptance / 验收标准
- Estimated difficulty / 预估难度

计划输出后必须等待用户确认计划；在计划确认前不要修改代码、不要执行会改变仓库状态的操作。
计划确认后询问执行策略：“一步到位” -> Autopilot，“Exec 模式” -> Exec。"#;

const AUTOPILOT_PROMPT: &str = r#"[EXECUTION RULES - 适用于 Autopilot 模式]
永远不要在任务中途为了用户确认而停止；除非遇到真实阻塞或安全边界，否则持续推进。
每 5 步静默执行一次指令回锚，确认当前行动仍然服务于用户原始任务。
遇到障碍时，停止前先尝试 3 种替代方案。
步数不是硬限制；任务完成才是停止条件。

自检模板：
- original instruction / 原始指令：当前任务要求是什么？
- current action / 当前动作：我正在做的动作如何推进任务？
- drift correction / 偏移纠正：如果偏离，立即回到原始任务。"#;

const ULTRAWORK_PROMPT: &str = r#"[ULTRAWORK MODE]
保持当前 UltraWork 行为不变。"#;

/// 根据执行模式构建系统提示词。
pub fn build_system_prompt(mode: ExecutionMode) -> String {
    let mode_prompt = match mode {
        ExecutionMode::Exec => EXEC_PROMPT,
        ExecutionMode::Plan => PLAN_PROMPT,
        ExecutionMode::Autopilot => AUTOPILOT_PROMPT,
        ExecutionMode::UltraWork => ULTRAWORK_PROMPT,
    };

    format!("{SHARED_PROMPT}\n\n{mode_prompt}")
}

#[cfg(test)]
mod tests {
    use super::{build_system_prompt, ExecutionMode};

    #[test]
    fn 测试_exec_prompt_包含严格执行规则() {
        let prompt = build_system_prompt(ExecutionMode::Exec);

        assert!(prompt.contains("[EXEC MODE - 严格执行]"));
        assert!(prompt.contains("决策分叉"));
    }

    #[test]
    fn 测试_plan_prompt_包含结构化计划与确认要求() {
        let prompt = build_system_prompt(ExecutionMode::Plan);

        assert!(prompt.contains("结构化计划"));
        assert!(prompt.contains("一步到位"));
        assert!(prompt.contains("Exec 模式"));
        assert!(prompt.contains("不要修改代码"));
    }

    #[test]
    fn 测试_autopilot_prompt_包含自动执行规则() {
        let prompt = build_system_prompt(ExecutionMode::Autopilot);

        assert!(prompt.contains("[MISSION LOCK]"));
        assert!(prompt.contains("指令回锚"));
        assert!(prompt.contains("步数不是硬限制"));
    }

    #[test]
    fn 测试_shared_prompt_包含任务难度信号() {
        let prompt = build_system_prompt(ExecutionMode::Exec);

        assert!(prompt.contains("TASK DIFFICULTY SIGNAL"));
        assert!(prompt.contains("easy/medium/hard/epic"));
    }

    #[test]
    fn 测试_ultrawork_prompt_保持当前行为() {
        let prompt = build_system_prompt(ExecutionMode::UltraWork);

        assert!(prompt.contains("UltraWork"));
        assert!(prompt.contains("保持当前 UltraWork 行为不变"));
    }
}
