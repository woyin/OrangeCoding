//! # Prometheus Agent — 战略规划器
//!
//! Prometheus 负责战略级别的规划工作，采用访谈式需求分析，
//! 生成结构化的实施计划。它是只读 Agent（仅可在 `.sisyphus/` 目录
//! 创建和修改 Markdown 文件），但可以委派分析任务给 Metis 和 Momus。

use super::{AgentDefinition, AgentKind};
use std::collections::HashSet;

/// 系统提示词常量 — 描述 Prometheus 的角色和行为准则
const SYSTEM_PROMPT: &str = "\
你是 Prometheus，系统的战略规划 Agent。你的核心职责是：

1. **访谈式需求分析**：通过结构化的提问，深入理解用户的真实需求。
   不要急于提出方案，先确保完整理解问题空间。
2. **上下文收集**：分析现有代码库的架构、约定和约束条件，
   为规划提供坚实的事实基础。
3. **计划生成**：生成详细的、可执行的实施计划，包括：
   - 任务分解和优先级排序
   - 依赖关系图
   - 风险评估和缓解策略
   - 预期的验收标准
4. **调用顾问**：在规划过程中调用 Metis 进行预分析，
   调用 Momus 对计划草案进行严格审核。
5. **计划迭代**：根据审核反馈修订计划，直到达到质量标准。

**输出格式**：
- 所有计划文档输出到 `.sisyphus/` 目录
- 使用 Markdown 格式，结构清晰
- 包含可操作的 todo 列表
- 每个任务附带验收标准

**约束**：
- 你是只读 Agent，不能直接修改项目代码
- 仅可在 `.sisyphus/` 目录下创建和修改 Markdown 计划文件
- 可以委派分析任务给 Metis（预分析）和 Momus（计划审核）
- 不要跳过需求分析直接给出方案

**工作流程**：
理解需求 → 收集上下文 → 草拟计划 → Metis 分析 → Momus 审核 → 修订 → 输出最终计划";

/// Prometheus Agent — 战略规划器实例
///
/// 只读的规划 Agent，可委派给 Metis/Momus 进行计划辅助分析和审核。
pub struct PrometheusAgent;

impl PrometheusAgent {
    /// 创建新的 Prometheus Agent 实例
    pub fn new() -> Self {
        Self
    }
}

impl AgentDefinition for PrometheusAgent {
    fn kind(&self) -> AgentKind {
        AgentKind::Prometheus
    }

    /// 默认使用 Claude Opus 4-6 — 需要强推理能力进行战略规划
    fn default_model(&self) -> &str {
        "claude-opus-4-6"
    }

    fn fallback_models(&self) -> Vec<String> {
        vec!["gpt-5.4".to_string(), "claude-sonnet-4-6".to_string()]
    }

    fn system_prompt(&self) -> &str {
        SYSTEM_PROMPT
    }

    fn display_name(&self) -> &str {
        "Prometheus"
    }

    fn description(&self) -> &str {
        "战略规划器 — 访谈式需求分析、计划生成"
    }

    /// Prometheus 是只读 Agent（仅可在 .sisyphus/ 中创建 Markdown）
    fn is_read_only(&self) -> bool {
        true
    }

    /// 可委派 — Prometheus 需要调用 Metis 和 Momus 协助规划
    fn can_delegate(&self) -> bool {
        true
    }
}
