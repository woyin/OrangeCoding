//! # Junior Agent — 任务执行者
//!
//! Junior（即 Sisyphus-Junior）是系统的通用任务执行 Agent，
//! 由 Category 决定具体使用的模型。默认使用 Claude Sonnet 4-6。
//! 它专注于单一任务的执行，不可委派子任务。

use super::{AgentDefinition, AgentKind};
use std::collections::HashSet;

/// 系统提示词常量 — 描述 Junior 的角色和行为准则
const SYSTEM_PROMPT: &str = "\
你是 Junior（Sisyphus-Junior），系统的通用任务执行 Agent。你的核心职责是：

1. **专注执行**：接收来自上级 Agent（如 Sisyphus 或 Atlas）分配的
   具体任务，高效、准确地完成执行。
2. **代码实现**：编写符合项目标准的代码，遵循现有的编码风格、
   命名约定和架构模式。
3. **测试编写**：为实现的功能编写单元测试和集成测试，
   确保代码的正确性和可回归性。
4. **错误修复**：定位和修复代码中的 bug，进行根因分析
   并实施经过验证的修复方案。
5. **重构执行**：按照上级指定的方案执行代码重构，
   确保重构后的功能等价性。

**工作方式**：
- 仔细阅读任务描述，确保理解要求
- 先探索相关代码，理解上下文
- 制定实现方案，然后逐步执行
- 每步验证结果，运行测试确认正确性
- 完成后报告执行结果

**约束**：
- 你不能委派任务给其他 Agent（task 和 call_omo_agent 被阻止）
- 你必须亲自完成分配给你的任务
- 不要超出任务范围进行额外修改
- 遇到不确定的情况时，保守处理并报告

**代码质量要求**：
- 变量和函数命名清晰准确
- 添加必要的错误处理
- 遵循项目的 lint 规则
- 保持代码简洁，避免过度设计";

/// Junior Agent — 任务执行者实例
///
/// 通用执行 Agent，默认使用 Claude Sonnet 4-6。不可委派。
pub struct JuniorAgent;

impl JuniorAgent {
    /// 创建新的 Junior Agent 实例
    pub fn new() -> Self {
        Self
    }
}

impl AgentDefinition for JuniorAgent {
    fn kind(&self) -> AgentKind {
        AgentKind::Junior
    }

    /// 默认使用 Claude Sonnet 4-6 — 实际模型由 Category 在运行时决定
    fn default_model(&self) -> &str {
        "claude-sonnet-4-6"
    }

    fn fallback_models(&self) -> Vec<String> {
        vec!["gpt-5.4".to_string(), "claude-opus-4-6".to_string()]
    }

    /// 阻止委派相关工具 — Junior 必须亲自执行任务
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
        "Junior"
    }

    fn description(&self) -> &str {
        "任务执行者 — 由 Category 决定模型的执行 Agent"
    }

    /// Junior 不可委派任务
    fn can_delegate(&self) -> bool {
        false
    }
}
