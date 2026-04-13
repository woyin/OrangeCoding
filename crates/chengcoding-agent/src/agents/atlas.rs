//! # Atlas Agent — 任务编排器
//!
//! Atlas 负责执行已验证的计划，管理 todo 列表的执行流程。
//! 它不能委派子任务（task 和 call_omo_agent 被阻止），
//! 但可以读取、验证和搜索代码库来确保任务执行的正确性。

use super::{AgentDefinition, AgentKind};
use std::collections::HashSet;

/// 系统提示词常量 — 描述 Atlas 的角色和行为准则
const SYSTEM_PROMPT: &str = "\
你是 Atlas，系统的任务编排 Agent。你的核心职责是：

1. **计划执行**：接收 Prometheus 生成的已验证计划，按序执行每个任务项。
   严格遵循计划中定义的任务顺序和依赖关系。
2. **进度追踪**：维护 todo 列表的状态，标记已完成、进行中和阻塞的任务。
   提供清晰的进度报告。
3. **质量验证**：每完成一个任务后，验证其是否满足计划中定义的验收标准。
   运行相关测试，检查构建状态。
4. **代码搜索**：使用 grep、glob 等搜索工具查找相关代码，
   理解修改的影响范围。
5. **文件读取**：阅读项目文件以理解上下文，确保修改的准确性。
6. **命令执行**：运行构建、测试、lint 等命令，验证任务执行结果。

**约束**：
- 你不能委派任务给其他 Agent（task 和 call_omo_agent 被阻止）
- 你必须亲自完成所有分配给你的任务
- 严格按照计划执行，不要偏离已批准的方案
- 遇到阻塞时报告问题，等待上级 Agent 决策

**工作原则**：
- 一次专注一个任务，完成后再进入下一个
- 每个步骤都要验证，不要假设成功
- 保持执行日志清晰可追溯
- 遇到计划中未覆盖的情况时，保守处理";

/// Atlas Agent — 任务编排器实例
///
/// 执行已验证计划，不可委派。阻止 `task` 和 `call_omo_agent` 工具。
pub struct AtlasAgent;

impl AtlasAgent {
    /// 创建新的 Atlas Agent 实例
    pub fn new() -> Self {
        Self
    }
}

impl AgentDefinition for AtlasAgent {
    fn kind(&self) -> AgentKind {
        AgentKind::Atlas
    }

    /// 默认使用 Claude Sonnet 4-6 — 平衡推理能力和执行效率
    fn default_model(&self) -> &str {
        "claude-sonnet-4-6"
    }

    fn fallback_models(&self) -> Vec<String> {
        vec!["gpt-5.4".to_string(), "claude-opus-4-6".to_string()]
    }

    /// 阻止委派相关工具 — Atlas 必须亲自执行任务
    fn blocked_tools(&self) -> HashSet<String> {
        let mut blocked = HashSet::new();
        blocked.insert("task".to_string());
        blocked.insert("call_omo_agent".to_string());
        blocked
    }

    fn system_prompt(&self) -> &str {
        SYSTEM_PROMPT
    }

    fn display_name(&self) -> &str {
        "Atlas"
    }

    fn description(&self) -> &str {
        "任务编排器 — 执行已验证计划、管理 todo"
    }

    /// Atlas 不可委派任务
    fn can_delegate(&self) -> bool {
        false
    }
}
