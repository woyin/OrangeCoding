//! 代理注册表模块
//!
//! 提供代理的注册、发现和状态管理功能。
//! 基于 `DashMap` 实现线程安全的并发访问。

use std::fmt;

use chengcoding_core::{AgentCapability, AgentId, AgentRole, AgentStatus};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// 代理信息
// ---------------------------------------------------------------------------

/// 代理信息 - 描述一个已注册代理的元数据
///
/// 包含代理的标识、名称、角色、状态、能力列表和创建时间等信息。
/// 这些信息用于代理发现、任务分配和状态监控。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentInfo {
    /// 代理的唯一标识符
    pub id: AgentId,
    /// 代理的人类可读名称
    pub name: String,
    /// 代理在系统中承担的角色
    pub role: AgentRole,
    /// 代理当前的运行状态
    pub status: AgentStatus,
    /// 代理所具备的能力列表
    pub capabilities: Vec<AgentCapability>,
    /// 代理的创建（注册）时间
    pub created_at: DateTime<Utc>,
}

impl AgentInfo {
    /// 创建一个新的代理信息（初始状态为空闲）
    pub fn new(id: AgentId, name: impl Into<String>, role: AgentRole) -> Self {
        Self {
            id,
            name: name.into(),
            role,
            status: AgentStatus::Idle,
            capabilities: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// 创建代理信息并指定初始能力列表
    pub fn with_capabilities(
        id: AgentId,
        name: impl Into<String>,
        role: AgentRole,
        capabilities: Vec<AgentCapability>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            role,
            status: AgentStatus::Idle,
            capabilities,
            created_at: Utc::now(),
        }
    }

    /// 检查代理是否具有指定名称的能力
    pub fn has_capability(&self, capability_name: &str) -> bool {
        self.capabilities.iter().any(|c| c.name == capability_name)
    }
}

impl fmt::Display for AgentInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "代理[{}] {} (角色: {}, 状态: {})",
            self.id, self.name, self.role, self.status
        )
    }
}

// ---------------------------------------------------------------------------
// 代理注册表
// ---------------------------------------------------------------------------

/// 代理注册表 - 管理所有已注册代理的中央注册表
///
/// 提供线程安全的代理注册、注销、查询和状态更新功能。
/// 底层使用 `DashMap` 实现无锁并发访问。
///
/// # 示例
///
/// ```rust
/// use chengcoding_mesh::agent_registry::{AgentRegistry, AgentInfo};
/// use chengcoding_core::{AgentId, AgentRole};
///
/// let registry = AgentRegistry::new();
/// let id = AgentId::new();
/// let info = AgentInfo::new(id.clone(), "编码助手", AgentRole::Coder);
/// registry.register(info);
///
/// let agent = registry.get(&id).unwrap();
/// assert_eq!(agent.name, "编码助手");
/// ```
#[derive(Debug)]
pub struct AgentRegistry {
    /// 内部代理映射表，键为代理 ID
    agents: DashMap<AgentId, AgentInfo>,
}

impl AgentRegistry {
    /// 创建一个空的代理注册表
    pub fn new() -> Self {
        debug!("创建新的代理注册表");
        Self {
            agents: DashMap::new(),
        }
    }

    /// 注册一个新代理
    ///
    /// 如果该 ID 已存在，将覆盖原有信息并记录警告日志。
    pub fn register(&self, info: AgentInfo) {
        let id = info.id.clone();
        if self.agents.contains_key(&id) {
            warn!(agent_id = %id, "代理已存在，将覆盖注册信息");
        }
        info!(agent_id = %id, name = %info.name, role = %info.role, "注册代理");
        self.agents.insert(id, info);
    }

    /// 注销一个代理
    ///
    /// 返回被注销的代理信息，如果 ID 不存在则返回 `None`。
    pub fn unregister(&self, id: &AgentId) -> Option<AgentInfo> {
        info!(agent_id = %id, "注销代理");
        self.agents.remove(id).map(|(_, info)| info)
    }

    /// 获取指定代理的信息副本
    pub fn get(&self, id: &AgentId) -> Option<AgentInfo> {
        self.agents.get(id).map(|entry| entry.value().clone())
    }

    /// 获取所有已注册代理的信息列表
    pub fn list(&self) -> Vec<AgentInfo> {
        self.agents
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 按角色查找代理
    ///
    /// 返回所有具有指定角色的代理信息列表。
    pub fn find_by_role(&self, role: &AgentRole) -> Vec<AgentInfo> {
        self.agents
            .iter()
            .filter(|entry| &entry.value().role == role)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 按能力查找代理
    ///
    /// 返回所有具有指定能力名称的代理信息列表。
    pub fn find_by_capability(&self, capability_name: &str) -> Vec<AgentInfo> {
        self.agents
            .iter()
            .filter(|entry| entry.value().has_capability(capability_name))
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 更新代理状态
    ///
    /// 成功更新返回 `true`，代理不存在返回 `false`。
    pub fn update_status(&self, id: &AgentId, status: AgentStatus) -> bool {
        match self.agents.get_mut(id) {
            Some(mut entry) => {
                debug!(agent_id = %id, old_status = %entry.status, new_status = %status, "更新代理状态");
                entry.status = status;
                true
            }
            None => {
                warn!(agent_id = %id, "尝试更新不存在的代理状态");
                false
            }
        }
    }

    /// 获取已注册代理的数量
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// 检查注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chengcoding_core::ToolName;

    #[test]
    fn 测试创建代理信息() {
        let id = AgentId::new();
        let info = AgentInfo::new(id.clone(), "测试代理", AgentRole::Coder);

        assert_eq!(info.id, id);
        assert_eq!(info.name, "测试代理");
        assert_eq!(info.role, AgentRole::Coder);
        assert_eq!(info.status, AgentStatus::Idle);
        assert!(info.capabilities.is_empty());
    }

    #[test]
    fn 测试创建带能力的代理信息() {
        let id = AgentId::new();
        let caps = vec![AgentCapability::new(
            "代码生成",
            "自动生成代码",
            vec![ToolName::new("file_write")],
        )];
        let info = AgentInfo::with_capabilities(id, "高级代理", AgentRole::Coder, caps);

        assert_eq!(info.capabilities.len(), 1);
        assert!(info.has_capability("代码生成"));
        assert!(!info.has_capability("不存在的能力"));
    }

    #[test]
    fn 测试代理信息的显示() {
        let info = AgentInfo::new(AgentId::new(), "显示测试", AgentRole::Reviewer);
        let display = format!("{info}");
        assert!(display.contains("显示测试"));
        assert!(display.contains("审查者"));
    }

    #[test]
    fn 测试注册和获取代理() {
        let registry = AgentRegistry::new();
        let id = AgentId::new();
        let info = AgentInfo::new(id.clone(), "代理A", AgentRole::Coder);

        registry.register(info);

        let retrieved = registry.get(&id).unwrap();
        assert_eq!(retrieved.name, "代理A");
        assert_eq!(retrieved.role, AgentRole::Coder);
    }

    #[test]
    fn 测试获取不存在的代理() {
        let registry = AgentRegistry::new();
        let id = AgentId::new();
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn 测试注销代理() {
        let registry = AgentRegistry::new();
        let id = AgentId::new();
        registry.register(AgentInfo::new(id.clone(), "待注销", AgentRole::Executor));

        let removed = registry.unregister(&id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "待注销");
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn 测试注销不存在的代理() {
        let registry = AgentRegistry::new();
        assert!(registry.unregister(&AgentId::new()).is_none());
    }

    #[test]
    fn 测试列出所有代理() {
        let registry = AgentRegistry::new();
        registry.register(AgentInfo::new(AgentId::new(), "代理1", AgentRole::Coder));
        registry.register(AgentInfo::new(AgentId::new(), "代理2", AgentRole::Reviewer));

        let agents = registry.list();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn 测试按角色查找代理() {
        let registry = AgentRegistry::new();
        registry.register(AgentInfo::new(AgentId::new(), "编码者1", AgentRole::Coder));
        registry.register(AgentInfo::new(AgentId::new(), "编码者2", AgentRole::Coder));
        registry.register(AgentInfo::new(
            AgentId::new(),
            "审查者1",
            AgentRole::Reviewer,
        ));

        let coders = registry.find_by_role(&AgentRole::Coder);
        assert_eq!(coders.len(), 2);

        let reviewers = registry.find_by_role(&AgentRole::Reviewer);
        assert_eq!(reviewers.len(), 1);

        let planners = registry.find_by_role(&AgentRole::Planner);
        assert!(planners.is_empty());
    }

    #[test]
    fn 测试按能力查找代理() {
        let registry = AgentRegistry::new();

        let caps = vec![AgentCapability::new("代码生成", "自动生成代码", vec![])];
        registry.register(AgentInfo::with_capabilities(
            AgentId::new(),
            "生成器",
            AgentRole::Coder,
            caps,
        ));

        registry.register(AgentInfo::new(
            AgentId::new(),
            "普通代理",
            AgentRole::Executor,
        ));

        let generators = registry.find_by_capability("代码生成");
        assert_eq!(generators.len(), 1);
        assert_eq!(generators[0].name, "生成器");

        let empty = registry.find_by_capability("不存在的能力");
        assert!(empty.is_empty());
    }

    #[test]
    fn 测试更新代理状态() {
        let registry = AgentRegistry::new();
        let id = AgentId::new();
        registry.register(AgentInfo::new(id.clone(), "状态测试", AgentRole::Coder));

        // 更新为运行中状态
        assert!(registry.update_status(&id, AgentStatus::Running));
        let info = registry.get(&id).unwrap();
        assert_eq!(info.status, AgentStatus::Running);

        // 更新不存在的代理应返回 false
        assert!(!registry.update_status(&AgentId::new(), AgentStatus::Failed));
    }

    #[test]
    fn 测试注册表数量统计() {
        let registry = AgentRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(AgentInfo::new(AgentId::new(), "代理", AgentRole::Coder));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn 测试覆盖注册() {
        let registry = AgentRegistry::new();
        let id = AgentId::new();

        registry.register(AgentInfo::new(id.clone(), "原始名称", AgentRole::Coder));
        registry.register(AgentInfo::new(id.clone(), "新名称", AgentRole::Reviewer));

        let info = registry.get(&id).unwrap();
        // 覆盖注册后应该使用新的信息
        assert_eq!(info.name, "新名称");
        assert_eq!(info.role, AgentRole::Reviewer);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn 测试默认构造() {
        let registry = AgentRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn 测试代理信息的序列化和反序列化() {
        let info = AgentInfo::new(AgentId::new(), "序列化测试", AgentRole::Planner);
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: AgentInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "序列化测试");
        assert_eq!(deserialized.role, AgentRole::Planner);
    }
}
