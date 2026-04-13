//! # Explore Agent — 代码搜索
//!
//! Explore 是系统的代码搜索 Agent，专注于快速上下文 grep、
//! 文件定位和代码模式搜索。它使用 Grok-Code-Fast-1 模型
//! 以获得最快的搜索响应速度。只读 Agent，不可委派。

use super::{AgentDefinition, AgentKind};
use std::collections::HashSet;

/// 系统提示词常量 — 描述 Explore 的角色和行为准则
const SYSTEM_PROMPT: &str = "\
你是 Explore，系统的代码搜索 Agent。你的核心职责是：

1. **快速搜索**：使用 grep、glob 等工具在代码库中快速搜索，
   定位特定的函数、类型、变量或模式。
2. **上下文提取**：找到目标代码后，提取足够的上下文信息，
   包括周围的注释、类型定义和相关实现。
3. **依赖追踪**：追踪函数调用链、类型引用和模块依赖，
   帮助理解代码的影响范围。
4. **模式识别**：识别代码库中的重复模式、命名约定和结构规律，
   为其他 Agent 提供代码库的「地图」。
5. **文件定位**：根据功能描述或关键词，快速定位相关源文件和目录。

**搜索策略**：
- 优先使用精确匹配，减少噪音
- 使用 glob 模式缩小文件范围
- 对搜索结果进行去重和排序
- 提供文件路径和行号以便精确定位

**约束**：
- 你是严格只读的 Agent，不能写入或编辑任何文件
- 你不能委派任务给其他 Agent
- write、edit、task、call_omo_agent 工具均被阻止
- 专注于搜索和读取，不要尝试修改代码

**输出要求**：
- 搜索结果附带完整的文件路径和行号
- 提供足够的代码上下文（前后各 3-5 行）
- 对多个匹配结果进行相关性排序
- 标注最可能是目标的结果";

/// Explore Agent — 代码搜索实例
///
/// 只读搜索 Agent，使用 Grok-Code-Fast-1。不可写入、编辑或委派。
pub struct ExploreAgent;

impl ExploreAgent {
    /// 创建新的 Explore Agent 实例
    pub fn new() -> Self {
        Self
    }
}

impl AgentDefinition for ExploreAgent {
    fn kind(&self) -> AgentKind {
        AgentKind::Explore
    }

    /// 默认使用 Grok-Code-Fast-1 — 优化代码搜索速度
    fn default_model(&self) -> &str {
        "grok-code-fast-1"
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
        "Explore"
    }

    fn description(&self) -> &str {
        "代码搜索 — 快速上下文 grep"
    }

    /// Explore 是只读 Agent
    fn is_read_only(&self) -> bool {
        true
    }

    /// Explore 不可委派任务
    fn can_delegate(&self) -> bool {
        false
    }
}
