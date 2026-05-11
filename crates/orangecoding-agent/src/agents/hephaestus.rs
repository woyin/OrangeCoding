//! # Hephaestus Agent — 深度工作者
//!
//! Hephaestus 是系统的深度工作 Agent，负责自主探索代码库、
//! 深度推理和执行复杂的实现任务。它拥有完整的工具访问权限，
//! 可以委派子任务，并以精确、深入的方式完成工作。

use super::{AgentDefinition, AgentKind};

/// 系统提示词常量 — 描述 Hephaestus 的角色和行为准则
const SYSTEM_PROMPT: &str = "\
你是 Hephaestus，系统的深度工作 Agent。你的核心职责是：

1. **自主探索**：独立探索代码库，理解架构、模式和依赖关系，
   不需要外部指导即可深入理解项目结构。
2. **深度推理**：对复杂问题进行多层次分析，考虑边界情况、
   性能影响、安全隐患和可维护性。
3. **精确实现**：编写高质量代码，遵循项目现有的编码风格和约定，
   确保实现的正确性和完整性。
4. **测试驱动**：在实现功能时同步编写测试，确保代码的可靠性。
5. **重构优化**：识别代码中的坏味道，提出并执行安全的重构方案。
6. **文档同步**：在修改代码时同步更新相关文档，保持文档与代码的一致性。

**工作方式**：
- 接收到任务后，先进行充分的代码库探索和上下文收集
- 制定实现方案，考虑可能的风险和替代方案
- 逐步实现，每个步骤都验证正确性
- 完成后进行自我审查，确保代码质量

**特殊能力**：
- 可以执行长时间运行的复杂任务
- 支持多文件并行修改
- 能够理解和维护复杂的依赖关系图
- 可以委派辅助任务给其他 Agent

你是系统中最强大的实现者，负责处理最复杂、最具挑战性的编码任务。";

/// Hephaestus Agent — 深度工作者实例
///
/// 自主探索、深度推理、精确实现。无工具限制，温度 0.1。
pub struct HephaestusAgent;

impl HephaestusAgent {
    /// 创建新的 Hephaestus Agent 实例
    pub fn new() -> Self {
        Self
    }
}

impl AgentDefinition for HephaestusAgent {
    fn kind(&self) -> AgentKind {
        AgentKind::Hephaestus
    }

    /// 默认使用 GPT-5.4 — 强大的推理和代码生成能力
    fn default_model(&self) -> &str {
        "gpt-5.4"
    }

    fn fallback_models(&self) -> Vec<String> {
        vec![
            "claude-opus-4-6".to_string(),
            "claude-sonnet-4-6".to_string(),
        ]
    }

    fn system_prompt(&self) -> &str {
        SYSTEM_PROMPT
    }

    fn display_name(&self) -> &str {
        "Hephaestus"
    }

    fn description(&self) -> &str {
        "深度工作者 — 自主探索、深度推理"
    }

    /// 温度 0.1 — 低温确保输出的确定性和一致性
    fn default_temperature(&self) -> f32 {
        0.1
    }

    // 无工具限制：blocked_tools() 使用默认空集合
    // 可委派：can_delegate() 使用默认 true
    // 非只读：is_read_only() 使用默认 false
}
