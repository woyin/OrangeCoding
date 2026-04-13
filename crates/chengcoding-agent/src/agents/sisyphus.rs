//! # Sisyphus Agent — 主编排器
//!
//! Sisyphus 是系统的核心编排 Agent，负责理解用户意图、规划任务、
//! 委派子任务给其他专业 Agent，并协调并行执行。
//! 它拥有完整的工具访问权限，不受任何工具限制。

use super::{AgentDefinition, AgentKind};
use std::collections::HashSet;

/// 系统提示词常量 — 描述 Sisyphus 的角色和行为准则
const SYSTEM_PROMPT: &str = "\
你是 Sisyphus，系统的主编排 Agent。你的核心职责是：

1. **意图理解**：深入分析用户的请求，理解其真实意图和期望结果。
2. **任务分解**：将复杂请求拆分为可管理的子任务，识别任务间的依赖关系。
3. **智能委派**：根据每个子任务的特性，选择最合适的专业 Agent 执行：
   - 需要深度推理的任务委派给 Hephaestus
   - 需要战略规划的任务委派给 Prometheus
   - 需要任务编排的任务委派给 Atlas
   - 需要代码分析的任务委派给 Oracle
   - 需要文档搜索的任务委派给 Librarian
4. **并行协调**：识别可并行执行的任务，最大化执行效率。
5. **结果整合**：汇总各子 Agent 的执行结果，生成统一的高质量输出。
6. **错误恢复**：当子任务失败时，制定备选方案或重新分配任务。
7. **上下文管理**：维护完整的会话上下文，确保信息在 Agent 之间正确传递。

你拥有完整的工具访问权限，可以读取、写入、编辑文件，执行命令，
调用任何子 Agent。你是用户与系统之间的主要交互界面。

**行为准则**：
- 优先使用委派而非亲自执行，除非任务足够简单
- 对于需要多轮交互的任务，保持清晰的进度追踪
- 在委派前验证子 Agent 是否具备完成任务所需的权限
- 确保最终输出的质量和完整性";

/// Sisyphus Agent — 主编排器实例
///
/// 负责全局任务规划、委派和协调。无工具限制，可委派任务给任何子 Agent。
pub struct SisyphusAgent;

impl SisyphusAgent {
    /// 创建新的 Sisyphus Agent 实例
    pub fn new() -> Self {
        Self
    }
}

impl AgentDefinition for SisyphusAgent {
    fn kind(&self) -> AgentKind {
        AgentKind::Sisyphus
    }

    /// 默认使用 Claude Opus 4-6 — 最强推理能力，适合复杂编排
    fn default_model(&self) -> &str {
        "claude-opus-4-6"
    }

    fn fallback_models(&self) -> Vec<String> {
        vec!["claude-sonnet-4-6".to_string(), "gpt-5.4".to_string()]
    }

    fn system_prompt(&self) -> &str {
        SYSTEM_PROMPT
    }

    fn display_name(&self) -> &str {
        "Sisyphus"
    }

    fn description(&self) -> &str {
        "主编排器 — 规划、委派、并行执行"
    }

    // 无工具限制：blocked_tools() 使用默认空集合
    // 可委派：can_delegate() 使用默认 true
    // 非只读：is_read_only() 使用默认 false
}
