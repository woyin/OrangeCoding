//! 核心类型定义模块
//!
//! 本模块定义了 CEAIR 系统中所有基础类型，包括各种标识符、枚举和数据结构。
//! 这些类型在整个系统中被广泛使用，是各个 crate 之间通信的基础。

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// 标识符类型
// ---------------------------------------------------------------------------

/// 代理标识符 - 每个 AI 代理的唯一 ID
///
/// 内部使用 UUID v4 保证全局唯一性。实现了常用的比较、哈希等 trait，
/// 可以安全地用作 HashMap 的键。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(Uuid);

impl AgentId {
    /// 创建一个新的随机代理 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 从已有的 UUID 创建代理 ID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 获取内部的 UUID 引用
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "agent-{}", self.0)
    }
}

impl From<Uuid> for AgentId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// 会话标识符 - 每个对话会话的唯一 ID
///
/// 一个会话可以包含多轮对话、多次工具调用等。
/// 同样基于 UUID v4 实现全局唯一性。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    /// 创建一个新的随机会话 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 从已有的 UUID 创建会话 ID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 获取内部的 UUID 引用
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

impl From<Uuid> for SessionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// 工具名称 - 标识系统中可用的工具
///
/// 使用字符串包装类型而非裸字符串，提供更强的类型安全性，
/// 防止与其他字符串类型混淆。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolName(String);

impl ToolName {
    /// 创建一个新的工具名称
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// 获取工具名称的字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ToolName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ToolName {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// 枚举类型
// ---------------------------------------------------------------------------

/// 代理角色 - 描述 AI 代理在系统中承担的职责
///
/// 不同角色的代理拥有不同的能力和行为模式：
/// - `Coder`：负责编写和修改代码
/// - `Reviewer`：负责审查代码质量和正确性
/// - `Planner`：负责分解任务并制定执行计划
/// - `Executor`：负责执行具体的操作命令
/// - `Observer`：负责监控和记录系统运行状态
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// 编码者 - 负责编写和修改代码
    Coder,
    /// 审查者 - 负责代码审查
    Reviewer,
    /// 规划者 - 负责任务分解和计划制定
    Planner,
    /// 执行者 - 负责执行具体操作
    Executor,
    /// 观察者 - 负责系统监控和状态记录
    Observer,
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            AgentRole::Coder => "编码者",
            AgentRole::Reviewer => "审查者",
            AgentRole::Planner => "规划者",
            AgentRole::Executor => "执行者",
            AgentRole::Observer => "观察者",
        };
        write!(f, "{label}")
    }
}

/// 代理状态 - 描述 AI 代理当前的运行状态
///
/// 状态机转换规则：
/// - `Idle` → `Running`（收到任务时）
/// - `Running` → `Waiting`（等待外部响应时）
/// - `Running` → `Completed`（任务成功完成时）
/// - `Running` → `Failed`（任务执行失败时）
/// - `Waiting` → `Running`（收到外部响应后）
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// 空闲 - 代理未执行任何任务
    Idle,
    /// 运行中 - 代理正在执行任务
    Running,
    /// 等待中 - 代理正在等待外部响应（如工具调用结果）
    Waiting,
    /// 已完成 - 代理已成功完成任务
    Completed,
    /// 失败 - 代理执行任务时遇到错误
    Failed,
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            AgentStatus::Idle => "空闲",
            AgentStatus::Running => "运行中",
            AgentStatus::Waiting => "等待中",
            AgentStatus::Completed => "已完成",
            AgentStatus::Failed => "失败",
        };
        write!(f, "{label}")
    }
}

impl AgentStatus {
    /// 判断代理是否处于终止状态（已完成或失败）
    pub fn is_terminal(&self) -> bool {
        matches!(self, AgentStatus::Completed | AgentStatus::Failed)
    }

    /// 判断代理是否处于活跃状态（运行中或等待中）
    pub fn is_active(&self) -> bool {
        matches!(self, AgentStatus::Running | AgentStatus::Waiting)
    }
}

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// 代理能力描述 - 描述一个 AI 代理所具备的能力
///
/// 包含能力名称、描述以及该能力支持使用的工具列表。
/// 用于在多代理协作场景中进行能力匹配和任务分配。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapability {
    /// 能力名称，例如 "代码生成"、"代码审查"
    pub name: String,
    /// 能力的详细描述
    pub description: String,
    /// 该能力支持使用的工具列表
    pub supported_tools: Vec<ToolName>,
}

impl AgentCapability {
    /// 创建一个新的代理能力描述
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        supported_tools: Vec<ToolName>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            supported_tools,
        }
    }

    /// 检查该能力是否支持指定的工具
    pub fn supports_tool(&self, tool: &ToolName) -> bool {
        self.supported_tools.contains(tool)
    }
}

impl fmt::Display for AgentCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (工具数: {})",
            self.name,
            self.supported_tools.len()
        )
    }
}

/// Token 使用量统计 - 追踪 AI 模型调用的 token 消耗
///
/// 记录每次 AI 调用的 prompt token 数、补全 token 数以及总量。
/// 用于成本核算、配额管理和使用量监控。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 提示词消耗的 token 数
    pub prompt_tokens: u64,
    /// AI 补全生成的 token 数
    pub completion_tokens: u64,
    /// 总共消耗的 token 数（prompt + completion）
    pub total_tokens: u64,
}

impl TokenUsage {
    /// 创建一个新的 token 使用量记录
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }

    /// 将另一个 TokenUsage 的数据累加到当前记录中
    ///
    /// 常用于在多轮对话中累计 token 消耗。
    pub fn accumulate(&mut self, other: &TokenUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
    }

    /// 判断是否有任何 token 被消耗
    pub fn is_empty(&self) -> bool {
        self.total_tokens == 0
    }
}

impl fmt::Display for TokenUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Token 用量: 提示词={}, 补全={}, 总计={}",
            self.prompt_tokens, self.completion_tokens, self.total_tokens
        )
    }
}

impl std::ops::Add for TokenUsage {
    type Output = Self;

    /// 两个 TokenUsage 相加，返回累加后的结果
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            prompt_tokens: self.prompt_tokens + rhs.prompt_tokens,
            completion_tokens: self.completion_tokens + rhs.completion_tokens,
            total_tokens: self.total_tokens + rhs.total_tokens,
        }
    }
}

impl std::ops::AddAssign for TokenUsage {
    /// 将另一个 TokenUsage 累加到自身
    fn add_assign(&mut self, rhs: Self) {
        self.prompt_tokens += rhs.prompt_tokens;
        self.completion_tokens += rhs.completion_tokens;
        self.total_tokens += rhs.total_tokens;
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试代理ID的创建和显示() {
        let id = AgentId::new();
        let display = format!("{id}");
        // 验证显示格式以 "agent-" 前缀开头
        assert!(display.starts_with("agent-"));
    }

    #[test]
    fn 测试会话ID的创建和显示() {
        let id = SessionId::new();
        let display = format!("{id}");
        assert!(display.starts_with("session-"));
    }

    #[test]
    fn 测试代理ID的唯一性() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        // 两个随机生成的 ID 应该不同
        assert_ne!(id1, id2);
    }

    #[test]
    fn 测试从UUID创建代理ID() {
        let uuid = Uuid::new_v4();
        let id = AgentId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn 测试工具名称的创建和转换() {
        let tool = ToolName::new("file_read");
        assert_eq!(tool.as_str(), "file_read");
        assert_eq!(format!("{tool}"), "file_read");

        // 测试 From 转换
        let tool2: ToolName = "bash_exec".into();
        assert_eq!(tool2.as_str(), "bash_exec");

        let tool3: ToolName = String::from("grep_search").into();
        assert_eq!(tool3.as_str(), "grep_search");
    }

    #[test]
    fn 测试代理角色的序列化() {
        let role = AgentRole::Coder;
        let json = serde_json::to_string(&role).unwrap();
        // 验证 snake_case 序列化
        assert_eq!(json, "\"coder\"");

        // 验证反序列化
        let deserialized: AgentRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, AgentRole::Coder);
    }

    #[test]
    fn 测试代理角色的显示() {
        assert_eq!(format!("{}", AgentRole::Coder), "编码者");
        assert_eq!(format!("{}", AgentRole::Reviewer), "审查者");
        assert_eq!(format!("{}", AgentRole::Planner), "规划者");
        assert_eq!(format!("{}", AgentRole::Executor), "执行者");
        assert_eq!(format!("{}", AgentRole::Observer), "观察者");
    }

    #[test]
    fn 测试代理状态的判断方法() {
        // 终止状态
        assert!(AgentStatus::Completed.is_terminal());
        assert!(AgentStatus::Failed.is_terminal());
        assert!(!AgentStatus::Idle.is_terminal());
        assert!(!AgentStatus::Running.is_terminal());
        assert!(!AgentStatus::Waiting.is_terminal());

        // 活跃状态
        assert!(AgentStatus::Running.is_active());
        assert!(AgentStatus::Waiting.is_active());
        assert!(!AgentStatus::Idle.is_active());
        assert!(!AgentStatus::Completed.is_active());
        assert!(!AgentStatus::Failed.is_active());
    }

    #[test]
    fn 测试代理状态的序列化和反序列化() {
        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let deserialized: AgentStatus = serde_json::from_str("\"waiting\"").unwrap();
        assert_eq!(deserialized, AgentStatus::Waiting);
    }

    #[test]
    fn 测试代理能力的创建和工具检查() {
        let capability = AgentCapability::new(
            "代码生成",
            "根据需求自动生成代码",
            vec![ToolName::new("file_write"), ToolName::new("bash_exec")],
        );

        assert_eq!(capability.name, "代码生成");
        assert!(capability.supports_tool(&ToolName::new("file_write")));
        assert!(!capability.supports_tool(&ToolName::new("unknown_tool")));
    }

    #[test]
    fn 测试token使用量的计算() {
        let usage1 = TokenUsage::new(100, 50);
        assert_eq!(usage1.prompt_tokens, 100);
        assert_eq!(usage1.completion_tokens, 50);
        assert_eq!(usage1.total_tokens, 150);
        assert!(!usage1.is_empty());

        let usage2 = TokenUsage::new(200, 100);

        // 测试加法运算符
        let total = usage1 + usage2;
        assert_eq!(total.prompt_tokens, 300);
        assert_eq!(total.completion_tokens, 150);
        assert_eq!(total.total_tokens, 450);
    }

    #[test]
    fn 测试token使用量的累加() {
        let mut usage = TokenUsage::new(100, 50);
        let other = TokenUsage::new(200, 100);
        usage.accumulate(&other);

        assert_eq!(usage.prompt_tokens, 300);
        assert_eq!(usage.completion_tokens, 150);
        assert_eq!(usage.total_tokens, 450);
    }

    #[test]
    fn 测试token使用量的加法赋值() {
        let mut usage = TokenUsage::new(100, 50);
        usage += TokenUsage::new(200, 100);

        assert_eq!(usage.prompt_tokens, 300);
        assert_eq!(usage.completion_tokens, 150);
        assert_eq!(usage.total_tokens, 450);
    }

    #[test]
    fn 测试空token使用量() {
        let usage = TokenUsage::default();
        assert!(usage.is_empty());
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn 测试token使用量的显示() {
        let usage = TokenUsage::new(100, 50);
        let display = format!("{usage}");
        assert!(display.contains("100"));
        assert!(display.contains("50"));
        assert!(display.contains("150"));
    }

    #[test]
    fn 测试代理能力的显示() {
        let cap = AgentCapability::new("代码审查", "审查代码质量", vec![ToolName::new("diff")]);
        let display = format!("{cap}");
        assert!(display.contains("代码审查"));
        assert!(display.contains("1"));
    }

    #[test]
    fn 测试代理ID的JSON序列化和反序列化() {
        let id = AgentId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn 测试会话ID的JSON序列化和反序列化() {
        let id = SessionId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }
}
