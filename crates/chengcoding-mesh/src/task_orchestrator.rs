//! 任务编排器模块
//!
//! 提供基于 DAG（有向无环图）的任务依赖管理和调度功能。
//! 支持任务的添加、依赖关系定义、拓扑排序和状态追踪。

use std::collections::{HashMap, VecDeque};
use std::fmt;

use chengcoding_core::AgentId;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// 任务标识符
// ---------------------------------------------------------------------------

/// 任务标识符 - 唯一标识一个任务
///
/// 使用字符串作为底层类型，支持人类可读的任务命名。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    /// 从字符串创建任务 ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 获取任务 ID 的字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// ---------------------------------------------------------------------------
// 任务状态
// ---------------------------------------------------------------------------

/// 任务状态 - 描述任务在生命周期中的当前阶段
///
/// 状态转换规则：
/// - `Pending` → `Ready`（所有依赖完成时，由编排器自动更新）
/// - `Ready` → `Running`（任务被分配给代理执行时）
/// - `Running` → `Completed`（任务执行成功时）
/// - `Running` → `Failed`（任务执行失败时）
/// - 任意非终止状态 → `Cancelled`（任务被取消时）
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// 待处理 - 任务已创建但还有未完成的依赖
    Pending,
    /// 就绪 - 所有依赖已完成，等待分配执行
    Ready,
    /// 运行中 - 任务正在被某个代理执行
    Running,
    /// 已完成 - 任务执行成功
    Completed,
    /// 失败 - 任务执行失败
    Failed,
    /// 已取消 - 任务被取消
    Cancelled,
}

impl TaskStatus {
    /// 判断任务是否处于终止状态（已完成、失败或已取消）
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// 判断任务是否处于活跃状态（运行中）
    pub fn is_active(&self) -> bool {
        matches!(self, TaskStatus::Running)
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            TaskStatus::Pending => "待处理",
            TaskStatus::Ready => "就绪",
            TaskStatus::Running => "运行中",
            TaskStatus::Completed => "已完成",
            TaskStatus::Failed => "失败",
            TaskStatus::Cancelled => "已取消",
        };
        write!(f, "{label}")
    }
}

// ---------------------------------------------------------------------------
// 任务定义
// ---------------------------------------------------------------------------

/// 任务 - 描述一个可调度执行的工作单元
///
/// 每个任务有唯一的 ID、名称、描述、分配的代理、状态、
/// 依赖列表和执行结果等信息。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    /// 任务的唯一标识符
    pub id: TaskId,
    /// 任务名称
    pub name: String,
    /// 任务的详细描述
    pub description: String,
    /// 分配执行此任务的代理 ID（`None` 表示未分配）
    pub assigned_agent: Option<AgentId>,
    /// 任务当前的状态
    pub status: TaskStatus,
    /// 此任务依赖的其他任务 ID 列表
    pub dependencies: Vec<TaskId>,
    /// 任务执行结果（JSON 格式，`None` 表示尚未产生结果）
    pub result: Option<Value>,
}

impl Task {
    /// 创建一个新的任务（初始状态为待处理）
    pub fn new(
        id: impl Into<TaskId>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            assigned_agent: None,
            status: TaskStatus::Pending,
            dependencies: Vec::new(),
            result: None,
        }
    }

    /// 创建一个带依赖关系的任务
    pub fn with_dependencies(
        id: impl Into<TaskId>,
        name: impl Into<String>,
        description: impl Into<String>,
        dependencies: Vec<TaskId>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            assigned_agent: None,
            status: TaskStatus::Pending,
            dependencies,
            result: None,
        }
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "任务[{}] {} (状态: {})", self.id, self.name, self.status)
    }
}

// ---------------------------------------------------------------------------
// 编排器错误
// ---------------------------------------------------------------------------

/// 编排器错误类型
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    /// 任务未找到
    #[error("任务未找到: {0}")]
    TaskNotFound(TaskId),

    /// 检测到循环依赖
    #[error("检测到循环依赖")]
    CyclicDependency,

    /// 依赖的任务不存在
    #[error("依赖的任务不存在: {0}")]
    DependencyNotFound(TaskId),

    /// 任务 ID 重复
    #[error("任务 ID 已存在: {0}")]
    DuplicateTask(TaskId),

    /// 无效的状态转换
    #[error("无效的状态转换: 任务 {task_id} 从 {from} 到 {to}")]
    InvalidTransition {
        /// 任务 ID
        task_id: TaskId,
        /// 当前状态
        from: TaskStatus,
        /// 目标状态
        to: TaskStatus,
    },
}

// ---------------------------------------------------------------------------
// 任务编排器
// ---------------------------------------------------------------------------

/// 任务编排器 - 基于 DAG 的任务依赖管理和调度引擎
///
/// 管理任务的完整生命周期，包括创建、依赖定义、就绪检测、
/// 分配执行和状态追踪。自动处理依赖关系并提供拓扑排序。
///
/// # 示例
///
/// ```rust
/// use chengcoding_mesh::task_orchestrator::{TaskOrchestrator, Task, TaskId};
///
/// let orchestrator = TaskOrchestrator::new();
///
/// // 添加任务
/// orchestrator.add_task(Task::new("design", "设计", "设计系统架构")).unwrap();
/// orchestrator.add_task(Task::new("implement", "实现", "编写代码")).unwrap();
///
/// // 添加依赖：实现依赖于设计
/// orchestrator.add_dependency(
///     &TaskId::new("implement"),
///     &TaskId::new("design"),
/// ).unwrap();
///
/// // 获取就绪任务（设计任务没有依赖，立即就绪）
/// let ready = orchestrator.get_ready_tasks();
/// assert_eq!(ready.len(), 1);
/// assert_eq!(ready[0].id, TaskId::new("design"));
/// ```
#[derive(Debug)]
pub struct TaskOrchestrator {
    /// 任务映射表，键为任务 ID
    tasks: RwLock<HashMap<TaskId, Task>>,
}

impl TaskOrchestrator {
    /// 创建一个空的任务编排器
    pub fn new() -> Self {
        debug!("创建新的任务编排器");
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// 添加一个新任务
    ///
    /// 任务的初始状态为 `Pending`。如果任务没有依赖，
    /// 可以通过 `get_ready_tasks()` 获取。
    pub fn add_task(&self, task: Task) -> Result<(), OrchestratorError> {
        let mut tasks = self.tasks.write();

        // 检查任务 ID 是否重复
        if tasks.contains_key(&task.id) {
            return Err(OrchestratorError::DuplicateTask(task.id.clone()));
        }

        info!(task_id = %task.id, task_name = %task.name, "添加任务");
        tasks.insert(task.id.clone(), task);
        Ok(())
    }

    /// 为任务添加依赖关系
    ///
    /// `task_id` 依赖于 `depends_on`，即 `depends_on` 必须先完成，
    /// `task_id` 才能变为就绪状态。
    pub fn add_dependency(
        &self,
        task_id: &TaskId,
        depends_on: &TaskId,
    ) -> Result<(), OrchestratorError> {
        let mut tasks = self.tasks.write();

        // 验证两个任务都存在
        if !tasks.contains_key(task_id) {
            return Err(OrchestratorError::TaskNotFound(task_id.clone()));
        }
        if !tasks.contains_key(depends_on) {
            return Err(OrchestratorError::DependencyNotFound(depends_on.clone()));
        }

        debug!(task = %task_id, depends_on = %depends_on, "添加任务依赖");

        // 添加依赖关系
        let task = tasks.get_mut(task_id).unwrap();
        if !task.dependencies.contains(depends_on) {
            task.dependencies.push(depends_on.clone());
        }

        // 检查是否产生了循环依赖
        if Self::has_cycle_internal(&tasks) {
            // 回滚依赖关系
            let task = tasks.get_mut(task_id).unwrap();
            task.dependencies.retain(|d| d != depends_on);
            return Err(OrchestratorError::CyclicDependency);
        }

        Ok(())
    }

    /// 获取所有就绪任务（所有依赖已完成且自身状态为 Pending 的任务）
    ///
    /// 返回就绪任务的副本列表。
    pub fn get_ready_tasks(&self) -> Vec<Task> {
        let tasks = self.tasks.read();

        tasks
            .values()
            .filter(|task| {
                // 只考虑 Pending 状态的任务
                task.status == TaskStatus::Pending
                    && task.dependencies.iter().all(|dep_id| {
                        // 所有依赖都必须已完成
                        tasks
                            .get(dep_id)
                            .map(|dep| dep.status == TaskStatus::Completed)
                            .unwrap_or(false)
                    })
            })
            .cloned()
            .collect()
    }

    /// 分配任务给指定代理
    ///
    /// 将任务状态从 Pending/Ready 更新为 Running，并记录分配的代理。
    pub fn assign_task(
        &self,
        task_id: &TaskId,
        agent_id: AgentId,
    ) -> Result<(), OrchestratorError> {
        let mut tasks = self.tasks.write();

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| OrchestratorError::TaskNotFound(task_id.clone()))?;

        // 只有 Pending 或 Ready 状态的任务可以被分配
        if task.status != TaskStatus::Pending && task.status != TaskStatus::Ready {
            return Err(OrchestratorError::InvalidTransition {
                task_id: task_id.clone(),
                from: task.status.clone(),
                to: TaskStatus::Running,
            });
        }

        info!(task_id = %task_id, agent_id = %agent_id, "分配任务给代理");
        task.assigned_agent = Some(agent_id);
        task.status = TaskStatus::Running;
        Ok(())
    }

    /// 将任务标记为已完成
    ///
    /// 可以附带执行结果（JSON 值）。
    pub fn complete_task(
        &self,
        task_id: &TaskId,
        result: Option<Value>,
    ) -> Result<(), OrchestratorError> {
        let mut tasks = self.tasks.write();

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| OrchestratorError::TaskNotFound(task_id.clone()))?;

        if task.status != TaskStatus::Running {
            return Err(OrchestratorError::InvalidTransition {
                task_id: task_id.clone(),
                from: task.status.clone(),
                to: TaskStatus::Completed,
            });
        }

        info!(task_id = %task_id, "任务完成");
        task.status = TaskStatus::Completed;
        task.result = result;
        Ok(())
    }

    /// 将任务标记为失败
    ///
    /// 可以附带错误信息（JSON 值）。
    pub fn fail_task(
        &self,
        task_id: &TaskId,
        error: Option<Value>,
    ) -> Result<(), OrchestratorError> {
        let mut tasks = self.tasks.write();

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| OrchestratorError::TaskNotFound(task_id.clone()))?;

        if task.status != TaskStatus::Running {
            return Err(OrchestratorError::InvalidTransition {
                task_id: task_id.clone(),
                from: task.status.clone(),
                to: TaskStatus::Failed,
            });
        }

        warn!(task_id = %task_id, "任务失败");
        task.status = TaskStatus::Failed;
        task.result = error;
        Ok(())
    }

    /// 取消任务
    pub fn cancel_task(&self, task_id: &TaskId) -> Result<(), OrchestratorError> {
        let mut tasks = self.tasks.write();

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| OrchestratorError::TaskNotFound(task_id.clone()))?;

        if task.status.is_terminal() {
            return Err(OrchestratorError::InvalidTransition {
                task_id: task_id.clone(),
                from: task.status.clone(),
                to: TaskStatus::Cancelled,
            });
        }

        info!(task_id = %task_id, "取消任务");
        task.status = TaskStatus::Cancelled;
        Ok(())
    }

    /// 获取指定任务的状态
    pub fn get_status(&self, task_id: &TaskId) -> Option<TaskStatus> {
        self.tasks
            .read()
            .get(task_id)
            .map(|task| task.status.clone())
    }

    /// 获取指定任务的完整信息
    pub fn get_task(&self, task_id: &TaskId) -> Option<Task> {
        self.tasks.read().get(task_id).cloned()
    }

    /// 获取所有任务列表
    pub fn list_tasks(&self) -> Vec<Task> {
        self.tasks.read().values().cloned().collect()
    }

    /// 获取任务的拓扑排序执行顺序
    ///
    /// 使用 Kahn 算法进行拓扑排序。
    /// 如果存在循环依赖，返回 `CyclicDependency` 错误。
    pub fn get_execution_order(&self) -> Result<Vec<TaskId>, OrchestratorError> {
        let tasks = self.tasks.read();
        Self::topological_sort_internal(&tasks)
    }

    /// 获取任务数量
    pub fn task_count(&self) -> usize {
        self.tasks.read().len()
    }

    /// 检查编排器是否为空
    pub fn is_empty(&self) -> bool {
        self.tasks.read().is_empty()
    }

    // -----------------------------------------------------------------------
    // 内部辅助方法
    // -----------------------------------------------------------------------

    /// 内部循环检测方法（使用 DFS 染色法）
    ///
    /// 使用三色标记：
    /// - 白色（未访问）：初始状态
    /// - 灰色（访问中）：当前 DFS 路径上的节点
    /// - 黑色（已完成）：所有后继节点都已访问
    fn has_cycle_internal(tasks: &HashMap<TaskId, Task>) -> bool {
        // 0 = 白色，1 = 灰色，2 = 黑色
        let mut color: HashMap<&TaskId, u8> = HashMap::new();
        for id in tasks.keys() {
            color.insert(id, 0);
        }

        for id in tasks.keys() {
            if color[id] == 0 && Self::dfs_has_cycle(id, tasks, &mut color) {
                return true;
            }
        }

        false
    }

    /// DFS 循环检测的递归辅助函数
    fn dfs_has_cycle(
        node: &TaskId,
        tasks: &HashMap<TaskId, Task>,
        color: &mut HashMap<&TaskId, u8>,
    ) -> bool {
        // 标记为灰色（正在访问）
        if let Some(c) = color.get_mut(node) {
            *c = 1;
        }

        if let Some(task) = tasks.get(node) {
            for dep in &task.dependencies {
                match color.get(dep) {
                    Some(1) => return true, // 发现灰色节点，存在环
                    Some(0) => {
                        // 白色节点，继续深度搜索
                        if Self::dfs_has_cycle(dep, tasks, color) {
                            return true;
                        }
                    }
                    _ => {} // 黑色节点，已完成，跳过
                }
            }
        }

        // 标记为黑色（已完成）
        if let Some(c) = color.get_mut(node) {
            *c = 2;
        }

        false
    }

    /// 内部拓扑排序（Kahn 算法）
    fn topological_sort_internal(
        tasks: &HashMap<TaskId, Task>,
    ) -> Result<Vec<TaskId>, OrchestratorError> {
        // 计算每个任务的入度（被依赖的次数不是入度；入度是该任务依赖了多少个其他任务）
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        // 反向邻接表：dependency -> 依赖它的任务列表
        let mut reverse_adj: HashMap<TaskId, Vec<TaskId>> = HashMap::new();

        // 初始化
        for (id, task) in tasks.iter() {
            in_degree.entry(id.clone()).or_insert(0);
            for dep in &task.dependencies {
                *in_degree.entry(id.clone()).or_insert(0) += 1;
                reverse_adj.entry(dep.clone()).or_default().push(id.clone());
            }
        }

        // 将入度为 0 的任务加入队列
        let mut queue: VecDeque<TaskId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(current) = queue.pop_front() {
            result.push(current.clone());

            // 对所有依赖当前任务的任务，减少入度
            if let Some(dependents) = reverse_adj.get(&current) {
                for dependent in dependents {
                    if let Some(deg) = in_degree.get_mut(dependent) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        // 如果排序结果数量少于任务总数，说明存在环
        if result.len() != tasks.len() {
            return Err(OrchestratorError::CyclicDependency);
        }

        Ok(result)
    }
}

impl Default for TaskOrchestrator {
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
    use serde_json::json;

    #[test]
    fn 测试任务ID的创建和显示() {
        let id = TaskId::new("task-1");
        assert_eq!(id.as_str(), "task-1");
        assert_eq!(format!("{id}"), "task-1");

        // 测试 From 转换
        let id2: TaskId = "task-2".into();
        assert_eq!(id2.as_str(), "task-2");

        let id3: TaskId = String::from("task-3").into();
        assert_eq!(id3.as_str(), "task-3");
    }

    #[test]
    fn 测试任务状态的判断方法() {
        // 终止状态
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Ready.is_terminal());
        assert!(!TaskStatus::Running.is_terminal());

        // 活跃状态
        assert!(TaskStatus::Running.is_active());
        assert!(!TaskStatus::Pending.is_active());
    }

    #[test]
    fn 测试任务状态的显示() {
        assert_eq!(format!("{}", TaskStatus::Pending), "待处理");
        assert_eq!(format!("{}", TaskStatus::Ready), "就绪");
        assert_eq!(format!("{}", TaskStatus::Running), "运行中");
        assert_eq!(format!("{}", TaskStatus::Completed), "已完成");
        assert_eq!(format!("{}", TaskStatus::Failed), "失败");
        assert_eq!(format!("{}", TaskStatus::Cancelled), "已取消");
    }

    #[test]
    fn 测试创建任务() {
        let task = Task::new("t1", "设计", "设计系统架构");
        assert_eq!(task.id, TaskId::new("t1"));
        assert_eq!(task.name, "设计");
        assert_eq!(task.description, "设计系统架构");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.assigned_agent.is_none());
        assert!(task.dependencies.is_empty());
        assert!(task.result.is_none());
    }

    #[test]
    fn 测试创建带依赖的任务() {
        let deps = vec![TaskId::new("dep1"), TaskId::new("dep2")];
        let task = Task::with_dependencies("t2", "实现", "编写代码", deps.clone());
        assert_eq!(task.dependencies, deps);
    }

    #[test]
    fn 测试任务的显示() {
        let task = Task::new("my-task", "我的任务", "描述");
        let display = format!("{task}");
        assert!(display.contains("my-task"));
        assert!(display.contains("我的任务"));
        assert!(display.contains("待处理"));
    }

    #[test]
    fn 测试添加任务() {
        let orch = TaskOrchestrator::new();
        assert!(orch.add_task(Task::new("t1", "任务1", "描述1")).is_ok());
        assert_eq!(orch.task_count(), 1);
    }

    #[test]
    fn 测试添加重复任务() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "描述1")).unwrap();

        let result = orch.add_task(Task::new("t1", "重复", "重复任务"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrchestratorError::DuplicateTask(_)
        ));
    }

    #[test]
    fn 测试添加依赖关系() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("design", "设计", "")).unwrap();
        orch.add_task(Task::new("implement", "实现", "")).unwrap();

        let result = orch.add_dependency(&TaskId::new("implement"), &TaskId::new("design"));
        assert!(result.is_ok());

        let task = orch.get_task(&TaskId::new("implement")).unwrap();
        assert!(task.dependencies.contains(&TaskId::new("design")));
    }

    #[test]
    fn 测试添加不存在的依赖() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();

        let result = orch.add_dependency(&TaskId::new("t1"), &TaskId::new("not_exist"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrchestratorError::DependencyNotFound(_)
        ));
    }

    #[test]
    fn 测试循环依赖检测() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("a", "A", "")).unwrap();
        orch.add_task(Task::new("b", "B", "")).unwrap();
        orch.add_task(Task::new("c", "C", "")).unwrap();

        // a -> b -> c -> a（循环）
        orch.add_dependency(&TaskId::new("a"), &TaskId::new("b"))
            .unwrap();
        orch.add_dependency(&TaskId::new("b"), &TaskId::new("c"))
            .unwrap();

        let result = orch.add_dependency(&TaskId::new("c"), &TaskId::new("a"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrchestratorError::CyclicDependency
        ));
    }

    #[test]
    fn 测试获取就绪任务() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("design", "设计", "")).unwrap();
        orch.add_task(Task::new("implement", "实现", "")).unwrap();
        orch.add_dependency(&TaskId::new("implement"), &TaskId::new("design"))
            .unwrap();

        // 只有设计任务应该就绪（无依赖）
        let ready = orch.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, TaskId::new("design"));
    }

    #[test]
    fn 测试依赖完成后任务变为就绪() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("design", "设计", "")).unwrap();
        orch.add_task(Task::new("implement", "实现", "")).unwrap();
        orch.add_dependency(&TaskId::new("implement"), &TaskId::new("design"))
            .unwrap();

        // 分配并完成设计任务
        orch.assign_task(&TaskId::new("design"), AgentId::new())
            .unwrap();
        orch.complete_task(&TaskId::new("design"), None).unwrap();

        // 现在实现任务应该就绪
        let ready = orch.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, TaskId::new("implement"));
    }

    #[test]
    fn 测试分配任务() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();

        let agent = AgentId::new();
        orch.assign_task(&TaskId::new("t1"), agent.clone()).unwrap();

        let task = orch.get_task(&TaskId::new("t1")).unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.assigned_agent, Some(agent));
    }

    #[test]
    fn 测试完成任务() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();
        orch.assign_task(&TaskId::new("t1"), AgentId::new())
            .unwrap();

        orch.complete_task(&TaskId::new("t1"), Some(json!({"output": "成功"})))
            .unwrap();

        let task = orch.get_task(&TaskId::new("t1")).unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.result, Some(json!({"output": "成功"})));
    }

    #[test]
    fn 测试任务失败() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();
        orch.assign_task(&TaskId::new("t1"), AgentId::new())
            .unwrap();

        orch.fail_task(&TaskId::new("t1"), Some(json!({"error": "超时"})))
            .unwrap();

        let task = orch.get_task(&TaskId::new("t1")).unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
    }

    #[test]
    fn 测试取消任务() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();

        orch.cancel_task(&TaskId::new("t1")).unwrap();

        let task = orch.get_task(&TaskId::new("t1")).unwrap();
        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    #[test]
    fn 测试无效的状态转换() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();

        // 尝试在 Pending 状态完成任务（应该失败）
        let result = orch.complete_task(&TaskId::new("t1"), None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrchestratorError::InvalidTransition { .. }
        ));
    }

    #[test]
    fn 测试已完成的任务不能取消() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();
        orch.assign_task(&TaskId::new("t1"), AgentId::new())
            .unwrap();
        orch.complete_task(&TaskId::new("t1"), None).unwrap();

        let result = orch.cancel_task(&TaskId::new("t1"));
        assert!(result.is_err());
    }

    #[test]
    fn 测试获取任务状态() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();

        assert_eq!(
            orch.get_status(&TaskId::new("t1")),
            Some(TaskStatus::Pending)
        );
        assert_eq!(orch.get_status(&TaskId::new("not_exist")), None);
    }

    #[test]
    fn 测试拓扑排序_简单链() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("a", "A", "")).unwrap();
        orch.add_task(Task::new("b", "B", "")).unwrap();
        orch.add_task(Task::new("c", "C", "")).unwrap();

        // c -> b -> a（c 依赖 b，b 依赖 a）
        orch.add_dependency(&TaskId::new("b"), &TaskId::new("a"))
            .unwrap();
        orch.add_dependency(&TaskId::new("c"), &TaskId::new("b"))
            .unwrap();

        let order = orch.get_execution_order().unwrap();
        assert_eq!(order.len(), 3);

        // a 应该在 b 之前，b 应该在 c 之前
        let pos_a = order.iter().position(|id| id == &TaskId::new("a")).unwrap();
        let pos_b = order.iter().position(|id| id == &TaskId::new("b")).unwrap();
        let pos_c = order.iter().position(|id| id == &TaskId::new("c")).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn 测试拓扑排序_钻石依赖() {
        let orch = TaskOrchestrator::new();
        //     a
        //    / \
        //   b   c
        //    \ /
        //     d
        orch.add_task(Task::new("a", "A", "")).unwrap();
        orch.add_task(Task::new("b", "B", "")).unwrap();
        orch.add_task(Task::new("c", "C", "")).unwrap();
        orch.add_task(Task::new("d", "D", "")).unwrap();

        orch.add_dependency(&TaskId::new("b"), &TaskId::new("a"))
            .unwrap();
        orch.add_dependency(&TaskId::new("c"), &TaskId::new("a"))
            .unwrap();
        orch.add_dependency(&TaskId::new("d"), &TaskId::new("b"))
            .unwrap();
        orch.add_dependency(&TaskId::new("d"), &TaskId::new("c"))
            .unwrap();

        let order = orch.get_execution_order().unwrap();
        assert_eq!(order.len(), 4);

        // a 应该最先
        let pos_a = order.iter().position(|id| id == &TaskId::new("a")).unwrap();
        let pos_b = order.iter().position(|id| id == &TaskId::new("b")).unwrap();
        let pos_c = order.iter().position(|id| id == &TaskId::new("c")).unwrap();
        let pos_d = order.iter().position(|id| id == &TaskId::new("d")).unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_a < pos_c);
        assert!(pos_b < pos_d);
        assert!(pos_c < pos_d);
    }

    #[test]
    fn 测试拓扑排序_无依赖() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("a", "A", "")).unwrap();
        orch.add_task(Task::new("b", "B", "")).unwrap();

        // 没有依赖关系，任何顺序都可以
        let order = orch.get_execution_order().unwrap();
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn 测试拓扑排序_空编排器() {
        let orch = TaskOrchestrator::new();
        let order = orch.get_execution_order().unwrap();
        assert!(order.is_empty());
    }

    #[test]
    fn 测试列出所有任务() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("t1", "任务1", "")).unwrap();
        orch.add_task(Task::new("t2", "任务2", "")).unwrap();

        let tasks = orch.list_tasks();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn 测试空编排器() {
        let orch = TaskOrchestrator::new();
        assert!(orch.is_empty());
        assert_eq!(orch.task_count(), 0);
    }

    #[test]
    fn 测试默认构造() {
        let orch = TaskOrchestrator::default();
        assert!(orch.is_empty());
    }

    #[test]
    fn 测试任务状态的序列化() {
        let json = serde_json::to_string(&TaskStatus::Running).unwrap();
        assert_eq!(json, "\"running\"");

        let deserialized: TaskStatus = serde_json::from_str("\"completed\"").unwrap();
        assert_eq!(deserialized, TaskStatus::Completed);
    }

    #[test]
    fn 测试任务的序列化和反序列化() {
        let task = Task::new("ser-test", "序列化测试", "测试任务的序列化");
        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, TaskId::new("ser-test"));
        assert_eq!(deserialized.name, "序列化测试");
        assert_eq!(deserialized.status, TaskStatus::Pending);
    }

    #[test]
    fn 测试重复添加相同依赖不会重复() {
        let orch = TaskOrchestrator::new();
        orch.add_task(Task::new("a", "A", "")).unwrap();
        orch.add_task(Task::new("b", "B", "")).unwrap();

        orch.add_dependency(&TaskId::new("b"), &TaskId::new("a"))
            .unwrap();
        orch.add_dependency(&TaskId::new("b"), &TaskId::new("a"))
            .unwrap();

        let task = orch.get_task(&TaskId::new("b")).unwrap();
        assert_eq!(task.dependencies.len(), 1);
    }

    #[test]
    fn 测试完整的任务生命周期() {
        let orch = TaskOrchestrator::new();

        // 创建任务
        orch.add_task(Task::new("plan", "规划", "制定计划"))
            .unwrap();
        orch.add_task(Task::new("code", "编码", "编写代码"))
            .unwrap();
        orch.add_task(Task::new("test", "测试", "运行测试"))
            .unwrap();

        // 设置依赖
        orch.add_dependency(&TaskId::new("code"), &TaskId::new("plan"))
            .unwrap();
        orch.add_dependency(&TaskId::new("test"), &TaskId::new("code"))
            .unwrap();

        // 第一轮：只有 plan 就绪
        let ready = orch.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, TaskId::new("plan"));

        // 执行 plan
        let agent = AgentId::new();
        orch.assign_task(&TaskId::new("plan"), agent.clone())
            .unwrap();
        orch.complete_task(&TaskId::new("plan"), Some(json!("计划已完成")))
            .unwrap();

        // 第二轮：code 变为就绪
        let ready = orch.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, TaskId::new("code"));

        // 执行 code
        orch.assign_task(&TaskId::new("code"), agent.clone())
            .unwrap();
        orch.complete_task(&TaskId::new("code"), None).unwrap();

        // 第三轮：test 变为就绪
        let ready = orch.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, TaskId::new("test"));

        // 执行 test
        orch.assign_task(&TaskId::new("test"), agent).unwrap();
        orch.complete_task(&TaskId::new("test"), Some(json!({"passed": true})))
            .unwrap();

        // 所有任务完成，没有更多就绪任务
        let ready = orch.get_ready_tasks();
        assert!(ready.is_empty());
    }
}
