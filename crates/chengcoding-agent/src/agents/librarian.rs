//! # Librarian Agent — 文档搜索
//!
//! Librarian 是系统的文档搜索 Agent，专注于多仓库文档分析、
//! 开源项目示例查找和技术文档检索。它是只读 Agent，
//! 使用 MiniMax-M2.7 模型以高效处理大量文本数据。

use super::{AgentDefinition, AgentKind};
use std::collections::HashSet;

/// 系统提示词常量 — 描述 Librarian 的角色和行为准则
const SYSTEM_PROMPT: &str = "\
你是 Librarian，系统的文档搜索 Agent。你的核心职责是：

1. **文档检索**：在项目文档、README、Wiki 和技术规范中搜索相关信息，
   快速定位用户或其他 Agent 所需的知识。
2. **多仓库分析**：跨多个代码仓库搜索和分析文档，
   发现不同项目间的共通模式和最佳实践。
3. **示例查找**：在开源项目中查找特定技术、API 或模式的使用示例，
   为实现提供参考。
4. **API 文档解析**：阅读和理解 API 文档、SDK 文档和库文档，
   提取关键信息和使用指南。
5. **知识汇总**：将分散在多个文档中的信息整合为结构化的知识摘要，
   方便其他 Agent 快速消化。

**搜索策略**：
- 优先搜索项目内部文档（README.md、docs/ 目录、注释）
- 然后搜索依赖库的文档和示例
- 最后搜索更广泛的开源生态
- 对搜索结果进行相关性排序和摘要

**约束**：
- 你是严格只读的 Agent，不能写入或编辑任何文件
- 你不能委派任务给其他 Agent
- write、edit、task、call_omo_agent 工具均被阻止
- 输出以摘要和引用为主，附带原始来源链接

**输出格式**：
- 每个搜索结果附带来源和置信度
- 提供相关代码片段的引用
- 对结果进行分类和优先排序";

/// Librarian Agent — 文档搜索实例
///
/// 只读文档搜索 Agent，使用 MiniMax-M2.7。不可写入、编辑或委派。
pub struct LibrarianAgent;

impl LibrarianAgent {
    /// 创建新的 Librarian Agent 实例
    pub fn new() -> Self {
        Self
    }
}

impl AgentDefinition for LibrarianAgent {
    fn kind(&self) -> AgentKind {
        AgentKind::Librarian
    }

    /// 默认使用 MiniMax-M2.7 — 高效处理大量文本数据
    fn default_model(&self) -> &str {
        "minimax-m2.7"
    }

    fn fallback_models(&self) -> Vec<String> {
        vec!["claude-sonnet-4-6".to_string(), "gpt-5.4".to_string()]
    }

    /// 阻止写入、编辑和委派相关工具
    fn blocked_tools(&self) -> HashSet<String> {
        let mut blocked = HashSet::new();
        blocked.insert("write".to_string());
        blocked.insert("edit".to_string());
        blocked.insert("task".to_string());
        blocked.insert("call_omo_agent".to_string());
        blocked
    }

    fn system_prompt(&self) -> &str {
        SYSTEM_PROMPT
    }

    fn display_name(&self) -> &str {
        "Librarian"
    }

    fn description(&self) -> &str {
        "文档搜索 — 多仓库分析、OSS 示例查找"
    }

    /// Librarian 是只读 Agent
    fn is_read_only(&self) -> bool {
        true
    }

    /// Librarian 不可委派任务
    fn can_delegate(&self) -> bool {
        false
    }
}
