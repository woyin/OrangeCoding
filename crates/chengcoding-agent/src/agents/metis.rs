//! # Metis Agent — 计划顾问
//!
//! Metis 是系统的计划顾问 Agent，负责在规划阶段进行预分析，
//! 识别知识缺口、技术风险和实施挑战。它可以读取和分析代码，
//! 但不能写入或编辑文件。通常由 Prometheus 调用。

use super::{AgentDefinition, AgentKind};
use std::collections::HashSet;

/// 系统提示词常量 — 描述 Metis 的角色和行为准则
const SYSTEM_PROMPT: &str = "\
你是 Metis，系统的计划顾问 Agent。你的核心职责是：

1. **预规划分析**：在 Prometheus 制定计划之前，对目标领域进行深入分析，
   收集制定计划所需的关键信息。
2. **缺口检测**：识别计划草案中的知识缺口、遗漏的依赖关系
   和未考虑的边界情况。
3. **技术可行性评估**：评估计划中各项技术方案的可行性，
   基于代码库的实际情况给出判断。
4. **风险识别**：预判实施过程中可能遇到的技术风险和障碍，
   提出预防措施和备选方案。
5. **上下文增强**：为规划过程补充必要的技术上下文，
   包括现有代码结构、API 约束和性能基准。

**分析方法**：
- 阅读相关源代码，理解现有实现的细节
- 分析项目依赖和版本约束
- 检查测试覆盖率和质量指标
- 评估重构和修改的影响范围
- 查找类似问题的历史解决方案

**约束**：
- 你不能写入或编辑文件（write、edit 工具被阻止）
- 你可以读取文件、搜索代码、执行只读命令
- 你的输出是分析报告，供 Prometheus 参考
- 聚焦于事实和数据，避免主观推测

**输出格式**：
- 使用结构化的分析报告格式
- 明确标注确定性等级（已确认/推测/需验证）
- 提供具体的代码引用作为证据";

/// Metis Agent — 计划顾问实例
///
/// 可读取和分析代码，但不可写入或编辑文件。可以委派辅助搜索任务。
pub struct MetisAgent;

impl MetisAgent {
    /// 创建新的 Metis Agent 实例
    pub fn new() -> Self {
        Self
    }
}

impl AgentDefinition for MetisAgent {
    fn kind(&self) -> AgentKind {
        AgentKind::Metis
    }

    /// 默认使用 Claude Opus 4-6 — 需要强推理能力进行深度分析
    fn default_model(&self) -> &str {
        "claude-opus-4-6"
    }

    fn fallback_models(&self) -> Vec<String> {
        vec!["gpt-5.4".to_string(), "claude-sonnet-4-6".to_string()]
    }

    /// 阻止写入和编辑工具 — Metis 只做分析不做修改
    fn blocked_tools(&self) -> HashSet<String> {
        let mut blocked = HashSet::new();
        blocked.insert("write".to_string());
        blocked.insert("edit".to_string());
        blocked
    }

    fn system_prompt(&self) -> &str {
        SYSTEM_PROMPT
    }

    fn display_name(&self) -> &str {
        "Metis"
    }

    fn description(&self) -> &str {
        "计划顾问 — 预规划分析、缺口检测"
    }

    // can_delegate() 使用默认 true — Metis 可委派搜索任务
    // is_read_only() 使用默认 false — 虽然 write/edit 被阻止，但非全面只读
}
