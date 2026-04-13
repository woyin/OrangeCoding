//! # 专业 Agent 模块
//!
//! 本模块定义了 11 个专业化 AI 代理，每个代理具有不同的专长、模型偏好和工具权限。
//! 采用 Clean-room 方式基于公开功能描述重新实现。
//!
//! ## Agent 列表
//!
//! | Agent | 角色 | 默认模型 |
//! |-------|------|---------|
//! | Sisyphus | 主编排器 | claude-opus-4-6 |
//! | Hephaestus | 深度工作者 | gpt-5.4 |
//! | Prometheus | 战略规划 | claude-opus-4-6 |
//! | Atlas | 任务编排 | claude-sonnet-4-6 |
//! | Oracle | 架构顾问 | gpt-5.4 |
//! | Librarian | 文档搜索 | minimax-m2.7 |
//! | Explore | 代码搜索 | grok-code-fast-1 |
//! | Metis | 计划顾问 | claude-opus-4-6 |
//! | Momus | 计划审核 | gpt-5.4 |
//! | Junior | 任务执行 | (由 Category 决定) |
//! | Multimodal | 视觉分析 | gpt-5.4 |

pub mod atlas;
pub mod explore;
pub mod hephaestus;
pub mod junior;
pub mod librarian;
pub mod metis;
pub mod momus;
pub mod multimodal;
pub mod oracle;
pub mod prometheus;
pub mod sisyphus;

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

// ============================================================
// AgentKind 枚举 — 所有内置 Agent 的类型标识
// ============================================================

/// Agent 类型枚举，标识系统中的每一种内置 Agent。
///
/// Tab 循环按 `tab_order()` 返回的顺序排列，前 4 个核心 Agent 有固定位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentKind {
    /// 主编排器 — 规划、委派、并行执行
    Sisyphus,
    /// 深度工作者 — 自主探索、深度推理
    Hephaestus,
    /// 战略规划器 — 访谈式需求分析、计划生成
    Prometheus,
    /// 任务编排器 — 执行已验证计划、管理 todo
    Atlas,
    /// 架构顾问 — 只读分析、代码审查
    Oracle,
    /// 文档搜索 — 多仓库分析、OSS 示例查找
    Librarian,
    /// 代码搜索 — 快速上下文 grep
    Explore,
    /// 计划顾问 — 预规划分析、缺口检测
    Metis,
    /// 计划审核 — 严格验证计划质量
    Momus,
    /// 任务执行者 — 由 Category 决定模型的执行 Agent
    Junior,
    /// 多模态分析 — PDF/图片/图表分析
    Multimodal,
}

impl AgentKind {
    /// 返回该 Agent 在 Tab 循环中的排序值。
    ///
    /// 核心 Agent 有固定排序：Sisyphus(1), Hephaestus(2), Prometheus(3), Atlas(4)。
    /// 其余按字母序排在后面。
    pub fn tab_order(&self) -> u8 {
        match self {
            Self::Sisyphus => 1,
            Self::Hephaestus => 2,
            Self::Prometheus => 3,
            Self::Atlas => 4,
            Self::Oracle => 5,
            Self::Librarian => 6,
            Self::Explore => 7,
            Self::Metis => 8,
            Self::Momus => 9,
            Self::Junior => 10,
            Self::Multimodal => 11,
        }
    }

    /// 返回 Agent 的标识名称（小写、用于序列化和配置引用）
    pub fn name(&self) -> &'static str {
        match self {
            Self::Sisyphus => "sisyphus",
            Self::Hephaestus => "hephaestus",
            Self::Prometheus => "prometheus",
            Self::Atlas => "atlas",
            Self::Oracle => "oracle",
            Self::Librarian => "librarian",
            Self::Explore => "explore",
            Self::Metis => "metis",
            Self::Momus => "momus",
            Self::Junior => "sisyphus-junior",
            Self::Multimodal => "multimodal-looker",
        }
    }

    /// 返回所有内置 Agent 类型列表（按 Tab 顺序）
    pub fn all() -> Vec<Self> {
        let mut kinds = vec![
            Self::Sisyphus,
            Self::Hephaestus,
            Self::Prometheus,
            Self::Atlas,
            Self::Oracle,
            Self::Librarian,
            Self::Explore,
            Self::Metis,
            Self::Momus,
            Self::Junior,
            Self::Multimodal,
        ];
        kinds.sort_by_key(|k| k.tab_order());
        kinds
    }

    /// 该 Agent 是否是核心 Agent（出现在 Tab 循环的前 4 个位置）
    pub fn is_core(&self) -> bool {
        self.tab_order() <= 4
    }
}

impl FromStr for AgentKind {
    type Err = AgentError;

    /// 从字符串解析 Agent 类型。
    ///
    /// 支持多种别名，如 "sisyphus-junior" 和 "junior" 都映射到 Junior。
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sisyphus" => Ok(Self::Sisyphus),
            "hephaestus" => Ok(Self::Hephaestus),
            "prometheus" => Ok(Self::Prometheus),
            "atlas" => Ok(Self::Atlas),
            "oracle" => Ok(Self::Oracle),
            "librarian" => Ok(Self::Librarian),
            "explore" => Ok(Self::Explore),
            "metis" => Ok(Self::Metis),
            "momus" => Ok(Self::Momus),
            "junior" | "sisyphus-junior" => Ok(Self::Junior),
            "multimodal" | "multimodal-looker" => Ok(Self::Multimodal),
            _ => Err(AgentError::UnknownAgent(s.to_string())),
        }
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================
// AgentError — Agent 模块专用错误类型
// ============================================================

/// Agent 模块的错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    /// 未知的 Agent 名称
    UnknownAgent(String),
    /// Agent 工具权限不足
    ToolBlocked {
        /// 被阻止的工具名称
        tool: String,
        /// 试图使用该工具的 Agent
        agent: AgentKind,
    },
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAgent(name) => write!(f, "未知的 Agent 类型: '{}'", name),
            Self::ToolBlocked { tool, agent } => {
                write!(f, "Agent '{}' 无权使用工具 '{}'", agent, tool)
            }
        }
    }
}

impl std::error::Error for AgentError {}

// ============================================================
// AgentDefinition trait — Agent 的行为定义接口
// ============================================================

/// Agent 行为定义 trait。
///
/// 每个具名 Agent 实现此 trait 来定义自己的：
/// - 默认模型和回退链
/// - 工具限制（被阻止的工具集合）
/// - 系统提示词
/// - 显示名称和描述
pub trait AgentDefinition: Send + Sync {
    /// Agent 的类型标识
    fn kind(&self) -> AgentKind;

    /// 默认使用的模型标识符（如 "anthropic/claude-opus-4-6"）
    fn default_model(&self) -> &str;

    /// 模型变体（如 "max", "high", "medium"）
    fn default_variant(&self) -> Option<&str> {
        None
    }

    /// 模型回退链——当主模型不可用时依次尝试的模型列表
    fn fallback_models(&self) -> Vec<String> {
        vec![]
    }

    /// 该 Agent 被阻止使用的工具名称集合。
    ///
    /// 返回空集合表示无限制。
    fn blocked_tools(&self) -> HashSet<String> {
        HashSet::new()
    }

    /// 该 Agent 仅可使用的工具白名单。
    ///
    /// 返回 None 表示不使用白名单模式（使用 blocked_tools 黑名单模式）。
    /// 返回 Some(set) 表示仅允许集合中的工具。
    fn allowed_tools_only(&self) -> Option<HashSet<String>> {
        None
    }

    /// Agent 的系统提示词
    fn system_prompt(&self) -> &str;

    /// 人类可读的显示名称
    fn display_name(&self) -> &str;

    /// Agent 的简短描述
    fn description(&self) -> &str;

    /// 该 Agent 是否只读（不能写文件、编辑代码）
    fn is_read_only(&self) -> bool {
        false
    }

    /// 该 Agent 是否可以委派任务给子 Agent
    fn can_delegate(&self) -> bool {
        true
    }

    /// 默认温度参数
    fn default_temperature(&self) -> f32 {
        0.1
    }

    /// 检查该 Agent 是否有权使用指定工具
    fn can_use_tool(&self, tool_name: &str) -> bool {
        // 白名单模式优先
        if let Some(allowed) = self.allowed_tools_only() {
            return allowed.contains(tool_name);
        }
        // 黑名单模式
        !self.blocked_tools().contains(tool_name)
    }
}

// ============================================================
// AgentRegistry — Agent 实例注册表
// ============================================================

/// Agent 注册表，管理所有可用 Agent 实例。
///
/// 在系统启动时创建并注册所有内置 Agent，运行时通过名称查找。
pub struct AgentRegistry {
    /// 按 AgentKind 索引的 Agent 定义
    agents: std::collections::HashMap<AgentKind, Box<dyn AgentDefinition>>,
}

impl AgentRegistry {
    /// 创建空的 Agent 注册表
    pub fn new() -> Self {
        Self {
            agents: std::collections::HashMap::new(),
        }
    }

    /// 创建包含所有内置 Agent 的默认注册表
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(sisyphus::SisyphusAgent::new()));
        registry.register(Box::new(hephaestus::HephaestusAgent::new()));
        registry.register(Box::new(prometheus::PrometheusAgent::new()));
        registry.register(Box::new(atlas::AtlasAgent::new()));
        registry.register(Box::new(oracle::OracleAgent::new()));
        registry.register(Box::new(librarian::LibrarianAgent::new()));
        registry.register(Box::new(explore::ExploreAgent::new()));
        registry.register(Box::new(metis::MetisAgent::new()));
        registry.register(Box::new(momus::MomusAgent::new()));
        registry.register(Box::new(junior::JuniorAgent::new()));
        registry.register(Box::new(multimodal::MultimodalAgent::new()));
        registry
    }

    /// 注册一个 Agent 定义
    pub fn register(&mut self, agent: Box<dyn AgentDefinition>) {
        self.agents.insert(agent.kind(), agent);
    }

    /// 通过类型获取 Agent 定义
    pub fn get(&self, kind: AgentKind) -> Option<&dyn AgentDefinition> {
        self.agents.get(&kind).map(|a| a.as_ref())
    }

    /// 通过名称字符串获取 Agent 定义
    pub fn get_by_name(&self, name: &str) -> Option<&dyn AgentDefinition> {
        let kind = AgentKind::from_str(name).ok()?;
        self.get(kind)
    }

    /// 返回所有已注册的 Agent 数量
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// 注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// 按 Tab 顺序返回所有已注册的 Agent 类型
    pub fn sorted_kinds(&self) -> Vec<AgentKind> {
        let mut kinds: Vec<_> = self.agents.keys().copied().collect();
        kinds.sort_by_key(|k| k.tab_order());
        kinds
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 AgentKind 从字符串解析
    #[test]
    fn test_agent_kind_from_str() {
        assert_eq!(
            AgentKind::from_str("sisyphus").unwrap(),
            AgentKind::Sisyphus
        );
        assert_eq!(
            AgentKind::from_str("Hephaestus").unwrap(),
            AgentKind::Hephaestus
        );
        assert_eq!(
            AgentKind::from_str("PROMETHEUS").unwrap(),
            AgentKind::Prometheus
        );
        assert_eq!(AgentKind::from_str("atlas").unwrap(), AgentKind::Atlas);
        assert_eq!(AgentKind::from_str("oracle").unwrap(), AgentKind::Oracle);
        assert_eq!(
            AgentKind::from_str("librarian").unwrap(),
            AgentKind::Librarian
        );
        assert_eq!(AgentKind::from_str("explore").unwrap(), AgentKind::Explore);
        assert_eq!(AgentKind::from_str("metis").unwrap(), AgentKind::Metis);
        assert_eq!(AgentKind::from_str("momus").unwrap(), AgentKind::Momus);
        assert_eq!(AgentKind::from_str("junior").unwrap(), AgentKind::Junior);
        assert_eq!(
            AgentKind::from_str("sisyphus-junior").unwrap(),
            AgentKind::Junior
        );
        assert_eq!(
            AgentKind::from_str("multimodal").unwrap(),
            AgentKind::Multimodal
        );
        assert_eq!(
            AgentKind::from_str("multimodal-looker").unwrap(),
            AgentKind::Multimodal
        );
    }

    /// 测试未知 Agent 名称返回错误
    #[test]
    fn test_agent_kind_unknown() {
        assert!(AgentKind::from_str("unknown").is_err());
        assert!(AgentKind::from_str("").is_err());
        assert!(AgentKind::from_str("zeus").is_err());
    }

    /// 测试 Tab 循环顺序（核心 Agent 排在前面）
    #[test]
    fn test_agent_tab_order() {
        assert_eq!(AgentKind::Sisyphus.tab_order(), 1);
        assert_eq!(AgentKind::Hephaestus.tab_order(), 2);
        assert_eq!(AgentKind::Prometheus.tab_order(), 3);
        assert_eq!(AgentKind::Atlas.tab_order(), 4);
        // 非核心 Agent 顺序 >= 5
        assert!(AgentKind::Oracle.tab_order() >= 5);
    }

    /// 测试核心 Agent 判断
    #[test]
    fn test_agent_is_core() {
        assert!(AgentKind::Sisyphus.is_core());
        assert!(AgentKind::Hephaestus.is_core());
        assert!(AgentKind::Prometheus.is_core());
        assert!(AgentKind::Atlas.is_core());
        assert!(!AgentKind::Oracle.is_core());
        assert!(!AgentKind::Librarian.is_core());
    }

    /// 测试 all() 返回 11 个 Agent 且按 Tab 顺序排列
    #[test]
    fn test_agent_kind_all() {
        let all = AgentKind::all();
        assert_eq!(all.len(), 11);
        // 验证按 Tab 顺序排列
        for window in all.windows(2) {
            assert!(window[0].tab_order() <= window[1].tab_order());
        }
    }

    /// 测试 AgentKind 的 Display 实现
    #[test]
    fn test_agent_kind_display() {
        assert_eq!(format!("{}", AgentKind::Sisyphus), "sisyphus");
        assert_eq!(format!("{}", AgentKind::Junior), "sisyphus-junior");
        assert_eq!(format!("{}", AgentKind::Multimodal), "multimodal-looker");
    }

    /// 测试 AgentError 的显示格式
    #[test]
    fn test_agent_error_display() {
        let err = AgentError::UnknownAgent("zeus".into());
        assert!(err.to_string().contains("zeus"));

        let err = AgentError::ToolBlocked {
            tool: "write".into(),
            agent: AgentKind::Oracle,
        };
        assert!(err.to_string().contains("oracle"));
        assert!(err.to_string().contains("write"));
    }

    /// 测试 Sisyphus Agent 无工具限制
    #[test]
    fn test_sisyphus_no_tool_restrictions() {
        let agent = sisyphus::SisyphusAgent::new();
        assert!(agent.blocked_tools().is_empty());
        assert!(agent.can_delegate());
        assert!(!agent.is_read_only());
    }

    /// 测试 Oracle 是只读 Agent
    #[test]
    fn test_oracle_is_readonly() {
        let agent = oracle::OracleAgent::new();
        let blocked = agent.blocked_tools();
        assert!(blocked.contains("write"));
        assert!(blocked.contains("edit"));
        assert!(blocked.contains("task"));
        assert!(blocked.contains("call_omo_agent"));
        assert!(agent.is_read_only());
        assert!(!agent.can_delegate());
    }

    /// 测试 Librarian 不可写入和委派
    #[test]
    fn test_librarian_restrictions() {
        let agent = librarian::LibrarianAgent::new();
        assert!(agent.is_read_only());
        assert!(!agent.can_delegate());
        let blocked = agent.blocked_tools();
        assert!(blocked.contains("write"));
        assert!(blocked.contains("edit"));
    }

    /// 测试 Explore 不可写入和委派
    #[test]
    fn test_explore_restrictions() {
        let agent = explore::ExploreAgent::new();
        assert!(agent.is_read_only());
        assert!(!agent.can_delegate());
    }

    /// 测试 Multimodal 仅允许 read 工具
    #[test]
    fn test_multimodal_allowlist() {
        let agent = multimodal::MultimodalAgent::new();
        let allowed = agent.allowed_tools_only();
        assert!(allowed.is_some());
        let allowed = allowed.unwrap();
        assert!(allowed.contains("read"));
        // 白名单模式：未列出的工具应被拒绝
        assert!(!agent.can_use_tool("write"));
        assert!(agent.can_use_tool("read"));
    }

    /// 测试 Atlas 不可 re-delegate
    #[test]
    fn test_atlas_no_redelegate() {
        let agent = atlas::AtlasAgent::new();
        let blocked = agent.blocked_tools();
        assert!(blocked.contains("task"));
        assert!(blocked.contains("call_omo_agent"));
        assert!(!agent.can_delegate());
    }

    /// 测试 Junior 不可 delegate
    #[test]
    fn test_junior_no_delegate() {
        let agent = junior::JuniorAgent::new();
        let blocked = agent.blocked_tools();
        assert!(blocked.contains("task"));
        assert!(blocked.contains("call_omo_agent"));
        assert!(!agent.can_delegate());
    }

    /// 测试 Momus 不可写入和委派
    #[test]
    fn test_momus_restrictions() {
        let agent = momus::MomusAgent::new();
        let blocked = agent.blocked_tools();
        assert!(blocked.contains("write"));
        assert!(blocked.contains("edit"));
        assert!(blocked.contains("task"));
    }

    /// 测试 Prometheus 的只读和计划生成能力
    #[test]
    fn test_prometheus_readonly_planner() {
        let agent = prometheus::PrometheusAgent::new();
        assert!(agent.is_read_only());
        assert!(agent.can_delegate()); // Prometheus 可调用 Metis/Momus
    }

    /// 测试 can_use_tool 黑名单检查
    #[test]
    fn test_can_use_tool_blacklist() {
        let agent = oracle::OracleAgent::new();
        assert!(!agent.can_use_tool("write"));
        assert!(!agent.can_use_tool("edit"));
        assert!(agent.can_use_tool("read"));
        assert!(agent.can_use_tool("grep"));
    }

    /// 测试默认 AgentRegistry 包含 11 个 Agent
    #[test]
    fn test_default_registry_has_all_agents() {
        let registry = AgentRegistry::with_defaults();
        assert_eq!(registry.len(), 11);
    }

    /// 测试通过名称查找 Agent
    #[test]
    fn test_registry_get_by_name() {
        let registry = AgentRegistry::with_defaults();
        let sisyphus = registry.get_by_name("sisyphus");
        assert!(sisyphus.is_some());
        assert_eq!(sisyphus.unwrap().kind(), AgentKind::Sisyphus);

        let unknown = registry.get_by_name("zeus");
        assert!(unknown.is_none());
    }

    /// 测试 sorted_kinds 按 Tab 顺序返回
    #[test]
    fn test_registry_sorted_kinds() {
        let registry = AgentRegistry::with_defaults();
        let sorted = registry.sorted_kinds();
        assert_eq!(sorted.len(), 11);
        assert_eq!(sorted[0], AgentKind::Sisyphus);
        assert_eq!(sorted[1], AgentKind::Hephaestus);
        assert_eq!(sorted[2], AgentKind::Prometheus);
        assert_eq!(sorted[3], AgentKind::Atlas);
    }

    /// 测试每个 Agent 都有有效的默认模型
    #[test]
    fn test_all_agents_have_valid_models() {
        let registry = AgentRegistry::with_defaults();
        for kind in AgentKind::all() {
            let agent = registry
                .get(kind)
                .expect(&format!("Agent {:?} 应存在", kind));
            let model = agent.default_model();
            assert!(!model.is_empty(), "Agent {:?} 的默认模型不应为空", kind);
        }
    }

    /// 测试每个 Agent 都有系统提示词
    #[test]
    fn test_all_agents_have_prompts() {
        let registry = AgentRegistry::with_defaults();
        for kind in AgentKind::all() {
            let agent = registry.get(kind).unwrap();
            assert!(
                !agent.system_prompt().is_empty(),
                "Agent {:?} 的系统提示词不应为空",
                kind
            );
        }
    }
}
