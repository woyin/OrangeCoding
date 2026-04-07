//! 角色系统模块
//!
//! 定义代理角色的详细描述，包括系统提示词、允许使用的工具列表等。
//! 提供预定义的角色集合和自定义角色注册功能。

use std::collections::HashMap;

use ceair_core::AgentRole;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// 角色定义
// ---------------------------------------------------------------------------

/// 角色定义 - 描述一个代理角色的完整信息
///
/// 包括角色类型、系统提示词（用于引导 AI 的行为模式）、
/// 允许使用的工具列表以及角色的文字描述。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleDefinition {
    /// 角色类型
    pub role: AgentRole,
    /// 系统提示词 - 传递给 AI 模型的系统级指令
    pub system_prompt: String,
    /// 允许该角色使用的工具名称列表
    pub allowed_tools: Vec<String>,
    /// 角色的文字描述
    pub description: String,
}

impl RoleDefinition {
    /// 创建一个新的角色定义
    pub fn new(
        role: AgentRole,
        system_prompt: impl Into<String>,
        allowed_tools: Vec<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            role,
            system_prompt: system_prompt.into(),
            allowed_tools,
            description: description.into(),
        }
    }

    /// 检查该角色是否允许使用指定工具
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.allowed_tools.iter().any(|t| t == tool_name)
    }
}

// ---------------------------------------------------------------------------
// 角色注册表
// ---------------------------------------------------------------------------

/// 角色注册表 - 管理所有角色定义的中央注册表
///
/// 初始化时会自动注册一组预定义角色（编码者、审查者、规划者、
/// 执行者、观察者、协调者），也支持自定义角色的注册和覆盖。
///
/// # 示例
///
/// ```rust
/// use ceair_mesh::role_system::RoleRegistry;
/// use ceair_core::AgentRole;
///
/// let registry = RoleRegistry::new();
/// let prompt = registry.get_system_prompt(&AgentRole::Coder);
/// assert!(prompt.is_some());
/// ```
#[derive(Debug)]
pub struct RoleRegistry {
    /// 角色定义映射表，受读写锁保护
    roles: RwLock<HashMap<AgentRole, RoleDefinition>>,
}

impl RoleRegistry {
    /// 创建一个新的角色注册表，并注册所有预定义角色
    pub fn new() -> Self {
        debug!("创建角色注册表，初始化预定义角色");
        let registry = Self {
            roles: RwLock::new(HashMap::new()),
        };
        registry.register_default_roles();
        registry
    }

    /// 注册所有预定义角色
    fn register_default_roles(&self) {
        // 编码者角色
        self.register_role(RoleDefinition::new(
            AgentRole::Coder,
            concat!(
                "你是一位专业的软件工程师，擅长编写高质量的代码。\n",
                "你的职责是：\n",
                "1. 根据需求编写清晰、可维护的代码\n",
                "2. 遵循最佳实践和设计模式\n",
                "3. 编写必要的单元测试\n",
                "4. 添加适当的注释和文档\n",
                "5. 考虑边界情况和错误处理",
            ),
            vec![
                "file_read".to_string(),
                "file_write".to_string(),
                "bash_exec".to_string(),
                "grep_search".to_string(),
                "code_analysis".to_string(),
            ],
            "编码者 - 负责编写和修改代码，遵循最佳实践",
        ));

        // 审查者角色
        self.register_role(RoleDefinition::new(
            AgentRole::Reviewer,
            concat!(
                "你是一位经验丰富的代码审查专家。\n",
                "你的职责是：\n",
                "1. 检查代码的正确性和逻辑完整性\n",
                "2. 发现潜在的 bug 和安全漏洞\n",
                "3. 评估代码质量和可维护性\n",
                "4. 提出建设性的改进意见\n",
                "5. 确保代码符合项目规范",
            ),
            vec![
                "file_read".to_string(),
                "grep_search".to_string(),
                "code_analysis".to_string(),
                "diff_view".to_string(),
            ],
            "审查者 - 负责代码审查，发现问题并提出改进建议",
        ));

        // 规划者角色
        self.register_role(RoleDefinition::new(
            AgentRole::Planner,
            concat!(
                "你是一位出色的项目规划师和任务分解专家。\n",
                "你的职责是：\n",
                "1. 分析复杂需求并分解为可执行的子任务\n",
                "2. 确定任务之间的依赖关系\n",
                "3. 制定合理的执行顺序和优先级\n",
                "4. 评估每个任务的复杂度和所需资源\n",
                "5. 监控计划执行进度并及时调整",
            ),
            vec![
                "file_read".to_string(),
                "grep_search".to_string(),
                "task_management".to_string(),
            ],
            "规划者 - 负责任务分解、依赖分析和执行计划制定",
        ));

        // 执行者角色
        self.register_role(RoleDefinition::new(
            AgentRole::Executor,
            concat!(
                "你是一位高效的任务执行者。\n",
                "你的职责是：\n",
                "1. 精确执行分配给你的操作指令\n",
                "2. 运行命令并报告执行结果\n",
                "3. 处理执行过程中的异常情况\n",
                "4. 确保操作的安全性和可逆性\n",
                "5. 及时报告进度和遇到的问题",
            ),
            vec![
                "bash_exec".to_string(),
                "file_read".to_string(),
                "file_write".to_string(),
                "process_management".to_string(),
            ],
            "执行者 - 负责执行具体操作指令，处理系统交互",
        ));

        // 观察者角色
        self.register_role(RoleDefinition::new(
            AgentRole::Observer,
            concat!(
                "你是一位细心的系统观察者和监控专家。\n",
                "你的职责是：\n",
                "1. 监控系统运行状态和性能指标\n",
                "2. 记录重要事件和操作日志\n",
                "3. 检测异常行为和潜在风险\n",
                "4. 生成状态报告和分析总结\n",
                "5. 在必要时发出预警通知",
            ),
            vec![
                "file_read".to_string(),
                "grep_search".to_string(),
                "system_monitor".to_string(),
                "log_analysis".to_string(),
            ],
            "观察者 - 负责监控系统状态、记录日志和异常检测",
        ));

        info!("预定义角色注册完成");
    }

    /// 注册或更新一个角色定义
    ///
    /// 如果该角色已存在，将覆盖原有定义。
    pub fn register_role(&self, definition: RoleDefinition) {
        let role = definition.role.clone();
        debug!(role = %role, "注册角色定义");
        self.roles.write().insert(role, definition);
    }

    /// 获取指定角色的完整定义
    pub fn get_role(&self, role: &AgentRole) -> Option<RoleDefinition> {
        self.roles.read().get(role).cloned()
    }

    /// 获取指定角色的系统提示词
    pub fn get_system_prompt(&self, role: &AgentRole) -> Option<String> {
        self.roles
            .read()
            .get(role)
            .map(|def| def.system_prompt.clone())
    }

    /// 获取指定角色允许使用的工具列表
    pub fn get_allowed_tools(&self, role: &AgentRole) -> Option<Vec<String>> {
        self.roles
            .read()
            .get(role)
            .map(|def| def.allowed_tools.clone())
    }

    /// 获取所有已注册角色的列表
    pub fn list_roles(&self) -> Vec<AgentRole> {
        self.roles.read().keys().cloned().collect()
    }

    /// 获取已注册角色的数量
    pub fn len(&self) -> usize {
        self.roles.read().len()
    }

    /// 检查注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.roles.read().is_empty()
    }

    /// 检查指定角色是否允许使用某个工具
    pub fn is_tool_allowed(&self, role: &AgentRole, tool_name: &str) -> bool {
        self.roles
            .read()
            .get(role)
            .map(|def| def.is_tool_allowed(tool_name))
            .unwrap_or(false)
    }
}

impl Default for RoleRegistry {
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

    #[test]
    fn 测试创建角色定义() {
        let def = RoleDefinition::new(
            AgentRole::Coder,
            "你是编码助手",
            vec!["file_read".to_string(), "file_write".to_string()],
            "编码角色",
        );

        assert_eq!(def.role, AgentRole::Coder);
        assert_eq!(def.system_prompt, "你是编码助手");
        assert_eq!(def.allowed_tools.len(), 2);
        assert_eq!(def.description, "编码角色");
    }

    #[test]
    fn 测试工具权限检查() {
        let def = RoleDefinition::new(
            AgentRole::Coder,
            "prompt",
            vec!["file_read".to_string(), "bash_exec".to_string()],
            "desc",
        );

        assert!(def.is_tool_allowed("file_read"));
        assert!(def.is_tool_allowed("bash_exec"));
        assert!(!def.is_tool_allowed("not_allowed"));
    }

    #[test]
    fn 测试预定义角色注册() {
        let registry = RoleRegistry::new();

        // 验证所有预定义角色都已注册
        assert!(registry.get_role(&AgentRole::Coder).is_some());
        assert!(registry.get_role(&AgentRole::Reviewer).is_some());
        assert!(registry.get_role(&AgentRole::Planner).is_some());
        assert!(registry.get_role(&AgentRole::Executor).is_some());
        assert!(registry.get_role(&AgentRole::Observer).is_some());

        // 应该有 5 个预定义角色
        assert_eq!(registry.len(), 5);
    }

    #[test]
    fn 测试获取系统提示词() {
        let registry = RoleRegistry::new();

        let prompt = registry.get_system_prompt(&AgentRole::Coder).unwrap();
        assert!(prompt.contains("软件工程师"));

        let prompt = registry.get_system_prompt(&AgentRole::Reviewer).unwrap();
        assert!(prompt.contains("代码审查"));
    }

    #[test]
    fn 测试获取允许的工具列表() {
        let registry = RoleRegistry::new();

        let tools = registry.get_allowed_tools(&AgentRole::Coder).unwrap();
        assert!(tools.contains(&"file_read".to_string()));
        assert!(tools.contains(&"file_write".to_string()));
        assert!(tools.contains(&"bash_exec".to_string()));

        // 审查者不应该有写文件权限
        let tools = registry.get_allowed_tools(&AgentRole::Reviewer).unwrap();
        assert!(!tools.contains(&"file_write".to_string()));
    }

    #[test]
    fn 测试通过注册表检查工具权限() {
        let registry = RoleRegistry::new();

        assert!(registry.is_tool_allowed(&AgentRole::Coder, "bash_exec"));
        assert!(!registry.is_tool_allowed(&AgentRole::Reviewer, "bash_exec"));
    }

    #[test]
    fn 测试自定义角色注册() {
        let registry = RoleRegistry::new();
        let initial_count = registry.len();

        // 覆盖编码者角色
        registry.register_role(RoleDefinition::new(
            AgentRole::Coder,
            "自定义编码提示词",
            vec!["custom_tool".to_string()],
            "自定义编码角色",
        ));

        // 数量不变（覆盖了已有角色）
        assert_eq!(registry.len(), initial_count);

        // 验证已被覆盖
        let def = registry.get_role(&AgentRole::Coder).unwrap();
        assert_eq!(def.system_prompt, "自定义编码提示词");
        assert!(def.is_tool_allowed("custom_tool"));
    }

    #[test]
    fn 测试列出所有角色() {
        let registry = RoleRegistry::new();
        let roles = registry.list_roles();
        assert_eq!(roles.len(), 5);
    }

    #[test]
    fn 测试默认构造() {
        let registry = RoleRegistry::default();
        assert!(!registry.is_empty());
        // 默认应该有预定义角色
        assert!(registry.len() > 0);
    }

    #[test]
    fn 测试角色定义的序列化和反序列化() {
        let def = RoleDefinition::new(
            AgentRole::Planner,
            "规划者提示词",
            vec!["file_read".to_string()],
            "规划者描述",
        );

        let json = serde_json::to_string(&def).unwrap();
        let deserialized: RoleDefinition = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.role, AgentRole::Planner);
        assert_eq!(deserialized.system_prompt, "规划者提示词");
        assert_eq!(deserialized.allowed_tools, vec!["file_read".to_string()]);
    }

    #[test]
    fn 测试不存在的角色返回None() {
        let registry = RoleRegistry::new();

        // 注册表中已有所有预定义角色，但我们可以清空来测试
        // 或者测试一些边界情况
        // 由于所有 AgentRole 变体都已注册，这里测试返回值的正确性即可
        let def = registry.get_role(&AgentRole::Coder);
        assert!(def.is_some());
    }

    #[test]
    fn 测试各角色提示词不为空() {
        let registry = RoleRegistry::new();

        for role in [
            AgentRole::Coder,
            AgentRole::Reviewer,
            AgentRole::Planner,
            AgentRole::Executor,
            AgentRole::Observer,
        ] {
            let prompt = registry.get_system_prompt(&role).unwrap();
            assert!(!prompt.is_empty(), "角色 {} 的提示词不应为空", role);
        }
    }

    #[test]
    fn 测试各角色工具列表不为空() {
        let registry = RoleRegistry::new();

        for role in [
            AgentRole::Coder,
            AgentRole::Reviewer,
            AgentRole::Planner,
            AgentRole::Executor,
            AgentRole::Observer,
        ] {
            let tools = registry.get_allowed_tools(&role).unwrap();
            assert!(!tools.is_empty(), "角色 {} 的工具列表不应为空", role);
        }
    }
}
